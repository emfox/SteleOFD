//! Rasterization off the draw path.
//! Rendering can take long enough to drop a frame, and the reader's budget is
//! that the interface never blocks, so the worker rasterizes on its own thread.

use std::sync::mpsc::{Receiver, Sender};

use crate::viewer::{self, RenderedPage};

/// One unit of work: rasterize `page` at `dpi`, as generation `generation`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderRequest {
    /// Monotonic stamp; a newer stamp supersedes every older one.
    pub generation: u64,
    /// Page index to rasterize.
    pub page: usize,
    /// Resolution to rasterize at, before the texture limit is applied.
    pub dpi: f32,
}

/// A finished raster, carrying the generation it was requested under.
pub struct RenderResult {
    /// The generation of the request that produced this.
    pub generation: u64,
    /// Page index, so the caller can key its cache.
    pub page: usize,
    /// Resolution it was rendered at.
    pub dpi: f32,
    /// The decoded page, or the error that stopped it.
    pub outcome: Result<RenderedPage, String>,
}

/// The reader's half of the channel pair.
pub struct RenderQueue {
    to_worker: Sender<RenderRequest>,
    from_worker: Receiver<RenderResult>,
    /// The newest generation handed out.
    newest: u64,
    /// The newest generation handed out for each page, so a stale raster for one
    /// page can be told apart from a wanted raster for another.
    newest_for: std::collections::HashMap<usize, u64>,
    /// What has been asked for and not yet answered, so an identical repeat
    /// request can be recognised instead of restamped.
    in_flight: std::collections::HashMap<usize, f32>,
    /// The newest generation the worker has answered, if any.
    answered: Option<u64>,
}

impl RenderQueue {
    /// Start a worker over a copy of `bytes`, where the document came from.
    ///
    /// The thread ends when the queue is dropped and the channel closes.
    pub fn spawn(bytes: Vec<u8>) -> Self {
        let (to_worker, work) = std::sync::mpsc::channel::<RenderRequest>();
        let (finish, from_worker) = std::sync::mpsc::channel::<RenderResult>();
        // The worker keeps its own sender so it can put back the requests it
        // looked at but did not consume; without this a peek would either eat
        // them or need a second channel.
        let own_to_worker = to_worker.clone();
        std::thread::Builder::new()
            .name("steleofd-render".to_string())
            .spawn(move || run_worker(bytes, &work, &own_to_worker, &finish))
            .expect("the render worker thread starts");
        Self {
            to_worker,
            from_worker,
            newest: 0,
            newest_for: std::collections::HashMap::new(),
            in_flight: std::collections::HashMap::new(),
            answered: None,
        }
    }

    /// The generation currently being asked for.
    pub fn generation(&self) -> u64 {
        self.newest
    }

    /// Whether the newest request is still outstanding.
    pub fn is_busy(&self) -> bool {
        self.answered != Some(self.newest)
    }

    /// Forget what is in flight, because the resolution changed.
    /// A raster of the old resolution that lands now is stale and discarded;
    /// `newest_for` is left alone so that stays true.
    pub fn rezoom(&mut self) {
        self.in_flight.clear();
        self.newest += 1;
        self.answered = Some(self.newest);
    }

    /// Ask for a page. Returns the generation the request was stamped with.
    /// Asking twice for the same page at the same resolution is not new
    /// information, and honouring every repeat would starve the worker.
    pub fn request(&mut self, page: usize, dpi: f32) -> u64 {
        if self.in_flight.get(&page).is_some_and(|asked| *asked == dpi) {
            return self.newest;
        }
        self.newest += 1;
        self.newest_for.insert(page, self.newest);
        self.in_flight.insert(page, dpi);
        let request = RenderRequest {
            generation: self.newest,
            page,
            dpi,
        };
        // A closed worker means the thread is gone; the caller then simply
        // never sees a result, which reads as "still pending" and is safe.
        let _ = self.to_worker.send(request);
        self.newest
    }

    /// Everything the worker has finished since the last call.
    /// A result is dropped only when a newer request exists for the same page,
    /// so the other pages the reader asked for still arrive.
    pub fn drain(&mut self) -> Vec<RenderResult> {
        let mut done = Vec::new();
        while let Ok(result) = self.from_worker.try_recv() {
            self.answered = Some(match self.answered {
                Some(seen) => seen.max(result.generation),
                None => result.generation,
            });
            let superseded = self
                .newest_for
                .get(&result.page)
                .is_some_and(|newest| result.generation < *newest);
            // Whatever came back, that page is no longer outstanding, so the
            // next identical request is a real one again.
            self.in_flight.remove(&result.page);
            if !superseded {
                done.push(result);
            }
        }
        done
    }
}

/// The worker loop: recv a request and render it, skipping only work a newer
/// request for the same page has already made pointless. A request is never
/// skipped when it is the last word on its page, or nothing would answer it.
fn run_worker(
    bytes: Vec<u8>,
    work: &Receiver<RenderRequest>,
    own_to_worker: &Sender<RenderRequest>,
    finish: &Sender<RenderResult>,
) {
    let mut document = None;
    let mut pending: Option<RenderRequest> = None;
    loop {
        // Hold the request we are about to render, and look at whatever has
        // already been queued behind it. Peeking does not consume: `pending`
        // carries forward, so nothing is ever dropped on the floor.
        let request = match pending.take() {
            Some(request) => request,
            None => match work.recv() {
                Ok(request) => request,
                Err(_) => return,
            },
        };
        let queued = drain_queue(work);
        let superseded = queued
            .iter()
            .any(|later| later.page == request.page && later.generation > request.generation);
        if superseded {
            // The newest queued request for this page becomes the next one
            // rendered; the rest of the queue is put back untouched.
            let newest_for_page = queued
                .iter()
                .filter(|later| later.page == request.page)
                .max_by_key(|later| later.generation)
                .copied()
                .expect("a newer request for this page was just found");
            let rest = queued
                .iter()
                .copied()
                .filter(|later| *later != newest_for_page);
            pending = Some(newest_for_page);
            for later in rest.rev() {
                let _ = own_to_worker.send(later);
            }
            continue;
        }
        // Everything queued behind this request has to be rendered too, so put
        // it back in the order it arrived: a channel is a FIFO, so pushing in
        // reverse restores the original sequence.
        for later in queued.iter().rev() {
            let _ = own_to_worker.send(*later);
        }
        if document.is_none() {
            match steleofd_core::Document::open_bytes(&bytes) {
                Ok(doc) => document = Some(doc),
                Err(err) => {
                    let _ = finish.send(RenderResult {
                        generation: request.generation,
                        page: request.page,
                        dpi: request.dpi,
                        outcome: Err(err.to_string()),
                    });
                    continue;
                }
            }
        }
        let doc = document.as_ref().expect("opened just above");
        let outcome =
            viewer::render_page(doc, request.page, request.dpi).map_err(|e| e.to_string());
        let _ = finish.send(RenderResult {
            generation: request.generation,
            page: request.page,
            dpi: request.dpi,
            outcome,
        });
    }
}

/// Take everything currently queued, stopping at the first empty read.
fn drain_queue(work: &Receiver<RenderRequest>) -> Vec<RenderRequest> {
    let mut queued = Vec::new();
    while let Ok(request) = work.try_recv() {
        queued.push(request);
    }
    queued
}

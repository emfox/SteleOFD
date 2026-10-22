//! Painting text runs and images.

use super::*;

/// What a text run contributed to the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TextOutcome {
    /// Every glyph the run named was drawn.
    Drawn,
    /// The run drew, but some of its glyphs had no outline in any face. The
    /// count is carried rather than a flag: one missing ideograph in a heading
    /// and a run of them are the same boolean and not the same problem.
    ShortGlyphs {
        /// Glyph positions that came out empty.
        glyphs: usize,
    },
    /// The run's face could not be loaded, so the run was skipped.
    NoFace,
}

pub(crate) fn draw_text(
    pixmap: &mut Pixmap,
    run: &GlyphRun,
    state: &State,
    view: Rect,
    fonts: &FontManager,
    options: &RenderOptions,
) -> Result<TextOutcome> {
    if run.glyphs.is_empty() {
        return Ok(TextOutcome::Drawn);
    }
    // A face that will not load costs its run, not the page, and is counted in
    // `skipped_font_runs`. The chain gets one more chance, told what the run
    // is about to draw so the substitute can cover those characters.
    let wanted = run_wanted(run);
    let Some(face) = steleofd_display::run_face_for(fonts, run, &wanted) else {
        return Ok(TextOutcome::NoFace);
    };
    draw_text_inner(pixmap, run, state, view, fonts, face, options)
}

/// The code points a run will draw, or nothing when its codes are glyph
/// indices. `text` is no help there: it serves search and copy, not drawing.
pub(crate) fn run_wanted(run: &GlyphRun) -> Vec<u32> {
    if run.codes_are_glyphs {
        return Vec::new();
    }
    run.glyphs.iter().map(|&c| u32::from(c)).collect()
}

pub(crate) fn draw_text_inner(
    pixmap: &mut Pixmap,
    run: &GlyphRun,
    state: &State,
    view: Rect,
    fonts: &FontManager,
    face: Arc<steleofd_font::FontFace>,
    options: &RenderOptions,
) -> Result<TextOutcome> {
    let mut offset = 0.0f64;
    let mut missing = 0usize;
    let wanted = run_wanted(run);
    for (i, &gid) in run.glyphs.iter().enumerate() {
        if i > 0 {
            offset += run.advances.get(i - 1).copied().unwrap_or(0.0);
        }
        offset += run.letter_spacing;
        let Some((outline, _source)) =
            fonts.outline_for_code_in(&face, gid, run.codes_are_glyphs, &wanted)
        else {
            if fonts.is_missing_code(&face, gid, run.codes_are_glyphs) {
                missing += 1;
                tracing::debug!(
                    code = gid,
                    glyphs = run.codes_are_glyphs,
                    "a run code had no outline in its face or any fallback"
                );
            }
            continue;
        };
        // Outlines are normalized to 1/1000 em with y pointing up, and the
        // run's own matrix scales, turns and places them from there.
        let glyph = run.glyph_matrix(offset);
        let total = total_transform(state, &view, options.dpi);
        let ts_path = to_ts_path(&outline, total.pre_concat(to_ts(glyph)))?;
        if let Some(paint) = &run.fill {
            let p = solid_paint(paint);
            pixmap.fill_path(&ts_path, &p, FillRule::Winding, Transform::identity(), None);
        }
    }
    Ok(if missing > 0 {
        TextOutcome::ShortGlyphs { glyphs: missing }
    } else {
        TextOutcome::Drawn
    })
}

/// What became of one placed image.
pub(crate) enum ImageOutcome {
    /// The image was decoded and painted.
    Drawn,
    /// The image was skipped: its bytes would not decode or its size was
    /// rejected. The page keeps going, so a single bad resource cannot take
    /// the rest of the page with it.
    Failed,
}

pub(crate) fn draw_image(
    pixmap: &mut Pixmap,
    image: &ImageRef,
    dst: Rect,
    transform: Affine,
    state: &State,
    view: Rect,
    options: &RenderOptions,
) -> Result<ImageOutcome> {
    if image.data.is_empty() {
        // A missing resource must not abort the page; leave the area empty.
        return Ok(ImageOutcome::Failed);
    }
    let (pixels, iw, ih) = match decode_resource(image) {
        Ok(decoded) => decoded,
        Err(e) => {
            tracing::warn!(id = %image.id, error = %e, "skipping an image that would not decode");
            return Ok(ImageOutcome::Failed);
        }
    };
    let Some(size) = IntSize::from_wh(iw, ih) else {
        tracing::warn!(id = %image.id, w = iw, h = ih, "skipping an image with a rejected size");
        return Ok(ImageOutcome::Failed);
    };
    let Some(img) = Pixmap::from_vec(pixels, size) else {
        tracing::warn!(id = %image.id, w = iw, h = ih, "skipping an image whose pixels were rejected");
        return Ok(ImageOutcome::Failed);
    };

    // The pixels fill the destination rect and nothing else: the placement
    // CTM is already in the graphics state, so applying `transform` here
    // would scale the placement a second time.
    let pixel_rect = Affine::new(dst.w / iw as f64, 0.0, 0.0, dst.h / ih as f64, dst.x, dst.y);
    // `post_concat` multiplies on the right, which applies the rect before the
    // millimetre-to-pixel step: that is the order the offsets need, since
    // `pre_concat` would scale the rect's own origin by the page scale.
    let _ = transform;
    let total = to_ts(pixel_rect).post_concat(total_transform(state, &view, options.dpi));
    pixmap.draw_pixmap(
        0,
        0,
        img.as_ref(),
        &PixmapPaint::default(),
        total,
        state.clip_ref(),
    );
    Ok(ImageOutcome::Drawn)
}

/// Decode a placed resource into RGBA, whichever encoding it uses.
///
/// JBIG2 is not one `image` reads, so it takes its own path.
pub(crate) fn decode_resource(image: &ImageRef) -> Result<(Vec<u8>, u32, u32)> {
    let fail = |reason: String| {
        OfdError::Render(RenderError::ImageDecode {
            page: 0,
            reason: format!("resource {}: {reason}", image.id),
        })
    };
    if image.codec == steleofd_display::ImageCodec::Jbig2 {
        let gray = steleofd_io::jbig2::decode_to_gray(&image.data).map_err(fail)?;
        let (w, h) = (image.pixel_size.0, image.pixel_size.1);
        if gray.len() != (w * h) as usize {
            return Err(fail(format!(
                "JBIG2 declares {w}x{h} but decoded {} bytes",
                gray.len()
            )));
        }
        if w == 0 || h == 0 {
            return Err(fail(format!("JBIG2 size {w}x{h} rejected")));
        }
        let rgba: Vec<u8> = gray.iter().flat_map(|&v| [v, v, v, u8::MAX]).collect();
        return Ok((rgba, w, h));
    }
    let decoded = image::load_from_memory(&image.data).map_err(|e| fail(e.to_string()))?;
    let rgba = decoded.to_rgba8();
    let (w, h) = (rgba.width(), rgba.height());
    let premul: Vec<u8> = rgba.pixels().flat_map(|p| premultiply(p.0)).collect();
    Ok((premul, w, h))
}

pub(crate) fn premultiply(px: [u8; 4]) -> [u8; 4] {
    let a = px[3] as u32;
    if a == 255 {
        return px;
    }
    let m = |c: u8| ((c as u32 * a + 127) / 255) as u8;
    [m(px[0]), m(px[1]), m(px[2]), px[3]]
}

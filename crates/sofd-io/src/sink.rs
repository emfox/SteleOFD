//! Byte sink abstraction.

use std::io::Write;
use std::path::{Path, PathBuf};

use sofd_error::Result;

/// A destination for bytes.
pub trait ByteSink: Send {
    /// Open a write handle.
    fn open_writer(&mut self) -> Result<Box<dyn Write + Send + '_>>;

    /// Write everything at once; most implementations optimize this.
    fn write_all_bytes(&mut self, data: &[u8]) -> Result<()> {
        self.open_writer()?.write_all(data)?;
        Ok(())
    }

    /// Human-readable destination, used in errors.
    fn describe(&self) -> String;
}

/// A sink writing into memory.
#[derive(Default)]
pub struct VecSink {
    buf: Vec<u8>,
}

impl VecSink {
    /// Create an empty sink.
    pub fn new() -> Self {
        Self::default()
    }

    /// Take the accumulated bytes.
    pub fn into_vec(self) -> Vec<u8> {
        self.buf
    }

    /// Borrow the accumulated bytes.
    pub fn as_slice(&self) -> &[u8] {
        &self.buf
    }
}

impl ByteSink for VecSink {
    fn open_writer(&mut self) -> Result<Box<dyn Write + Send + '_>> {
        Ok(Box::new(&mut self.buf))
    }

    fn write_all_bytes(&mut self, data: &[u8]) -> Result<()> {
        self.buf.extend_from_slice(data);
        Ok(())
    }

    fn describe(&self) -> String {
        "<memory buffer>".to_string()
    }
}

/// A sink writing to a file, atomically by default.
pub struct FileSink {
    path: PathBuf,
    /// Writing goes to a temporary file that is renamed over the target only on
    /// success, so a mid-write failure cannot destroy an existing output file.
    temp: Option<PathBuf>,
}

impl FileSink {
    /// Create a sink that replaces the target atomically.
    pub fn new(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref().to_path_buf();
        let temp = path.with_extension("ofd.tmp");
        Self {
            path,
            temp: Some(temp),
        }
    }

    /// Write directly, without the temporary file.
    pub fn direct(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            temp: None,
        }
    }

    /// Atomically move the temporary file into place.
    pub fn commit(mut self) -> Result<()> {
        if let Some(temp) = self.temp.take() {
            std::fs::rename(&temp, &self.path)?;
        }
        Ok(())
    }
}

impl ByteSink for FileSink {
    fn open_writer(&mut self) -> Result<Box<dyn Write + Send + '_>> {
        let target = self.temp.as_ref().unwrap_or(&self.path);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(Box::new(std::fs::File::create(target)?))
    }

    fn describe(&self) -> String {
        format!("{}", self.path.display())
    }
}

/// A sink writing into a directory, for batch exports such as page images.
pub struct DirSink {
    dir: PathBuf,
}

impl DirSink {
    /// Create the directory if needed.
    pub fn new(dir: impl AsRef<Path>) -> Result<Self> {
        let dir = dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    /// A sink for one file inside the directory.
    pub fn child(&self, name: &str) -> FileSink {
        FileSink::direct(self.dir.join(name))
    }

    /// The directory path.
    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

impl ByteSink for DirSink {
    fn open_writer(&mut self) -> Result<Box<dyn Write + Send + '_>> {
        Err(std::io::Error::other("DirSink requires child() to pick a file name").into())
    }

    fn describe(&self) -> String {
        format!("{}", self.dir.display())
    }
}

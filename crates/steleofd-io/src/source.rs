//! Byte source abstraction.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use steleofd_error::Result;

/// A source of bytes.
/// Each call to [`ByteSource::open_reader`] must yield an independent reader from
/// offset zero, as parsing walks the data twice: central directory, then entries.
pub trait ByteSource: Send + Sync {
    /// Total length in bytes.
    fn len(&self) -> u64;

    /// Open a reader positioned at the start.
    fn open_reader(&self) -> Result<Box<dyn Read + Send + '_>>;

    /// Read the whole payload at once, convenient for small inputs.
    fn read_all(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::with_capacity(self.len() as usize);
        self.open_reader()?.read_to_end(&mut buf)?;
        Ok(buf)
    }

    /// Whether the source is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Human-readable origin, used in errors and logs.
    fn describe(&self) -> String;
}

/// A source backed by memory, used by WASM, FFI and uploads.
#[derive(Clone)]
pub struct MemorySource {
    data: Arc<Vec<u8>>,
}

impl MemorySource {
    /// Take ownership of a byte buffer.
    pub fn new(data: Vec<u8>) -> Self {
        Self {
            data: Arc::new(data),
        }
    }

    /// Share an existing buffer without copying.
    pub fn from_arc(data: Arc<Vec<u8>>) -> Self {
        Self { data }
    }

    /// Borrow the shared buffer.
    pub fn as_arc(&self) -> &Arc<Vec<u8>> {
        &self.data
    }
}

impl ByteSource for MemorySource {
    fn len(&self) -> u64 {
        self.data.len() as u64
    }

    fn open_reader(&self) -> Result<Box<dyn Read + Send + '_>> {
        Ok(Box::new(std::io::Cursor::new(self.data.as_slice())))
    }

    fn read_all(&self) -> Result<Vec<u8>> {
        Ok(self.data.as_ref().clone())
    }

    fn describe(&self) -> String {
        format!("<memory {} bytes>", self.data.len())
    }
}

/// A zero-copy source borrowing an external buffer.
pub struct SliceSource<'a> {
    data: &'a [u8],
    name: String,
}

impl<'a> SliceSource<'a> {
    /// Borrow a slice and give it a name for diagnostics.
    pub fn new(data: &'a [u8], name: impl Into<String>) -> Self {
        Self {
            data,
            name: name.into(),
        }
    }
}

impl ByteSource for SliceSource<'_> {
    fn len(&self) -> u64 {
        self.data.len() as u64
    }

    fn open_reader(&self) -> Result<Box<dyn Read + Send + '_>> {
        Ok(Box::new(std::io::Cursor::new(self.data)))
    }

    fn read_all(&self) -> Result<Vec<u8>> {
        Ok(self.data.to_vec())
    }

    fn describe(&self) -> String {
        format!("<slice {}>", self.name)
    }
}

/// A source backed by a file.
/// Files above [`crate::DEFAULT_MMAP_THRESHOLD`] are better mapped than read
/// whole, which avoids a memory spike on large documents.
pub struct FileSource {
    path: PathBuf,
    len: u64,
}

impl FileSource {
    /// Open a path as a source.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let len = std::fs::metadata(&path)?.len();
        Ok(Self { path, len })
    }

    /// The underlying path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Whether mapping is recommended for this file.
    pub fn should_mmap(&self) -> bool {
        self.len > crate::DEFAULT_MMAP_THRESHOLD
    }
}

impl ByteSource for FileSource {
    fn len(&self) -> u64 {
        self.len
    }

    fn open_reader(&self) -> Result<Box<dyn Read + Send + '_>> {
        Ok(Box::new(std::fs::File::open(&self.path)?))
    }

    fn describe(&self) -> String {
        format!("{}", self.path.display())
    }
}

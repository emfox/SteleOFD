//! OFD container layer.
//! An OFD file is a ZIP archive with `OFD.xml` as its entry point. This crate
//! opens that archive and hands out entries as bytes; it never interprets XML.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::collections::HashSet;
use std::io::Read;
use std::path::{Path, PathBuf};

use steleofd_error::{OfdError, PackageError, Result};
use steleofd_io::{ByteSource, DEFAULT_MAX_ENTRY_COUNT, DEFAULT_MAX_ENTRY_SIZE};

pub mod entry;

pub use entry::{EntryInfo, EntryKind};

/// Archive path of the document entry point, mandated by GB/T 33190-2016.
pub const ENTRY_POINT: &str = "OFD.xml";

/// Directory holding per-document subtrees.
pub const DOCS_PREFIX: &str = "Doc_";

/// Limits applied while opening an archive.
/// OFD files are routinely attacker-supplied, so every unbounded quantity gets a
/// ceiling rather than trusting the archive header.
#[derive(Debug, Clone, Copy)]
pub struct PackageLimits {
    /// Largest decompressed size accepted for one entry.
    pub max_entry_size: u64,
    /// Largest number of entries accepted.
    pub max_entry_count: usize,
    /// Largest total decompressed size accepted across all entries.
    pub max_total_size: u64,
}

impl Default for PackageLimits {
    fn default() -> Self {
        Self {
            max_entry_size: DEFAULT_MAX_ENTRY_SIZE,
            max_entry_count: DEFAULT_MAX_ENTRY_COUNT,
            max_total_size: 2 * 1024 * 1024 * 1024,
        }
    }
}

impl PackageLimits {
    /// Limits suitable for documents from an untrusted source.
    pub fn strict() -> Self {
        Self {
            max_entry_size: 64 * 1024 * 1024,
            max_entry_count: 4096,
            max_total_size: 256 * 1024 * 1024,
        }
    }

    /// Limits effectively off, for fuzzing and tests over known-good inputs.
    pub fn unbounded() -> Self {
        Self {
            max_entry_size: u64::MAX,
            max_entry_count: usize::MAX,
            max_total_size: u64::MAX,
        }
    }
}

/// An opened OFD container.
/// The reader stays alive for the container's lifetime because `zip` resolves
/// entries lazily against the central directory.
pub struct Package {
    archive: zip::ZipArchive<std::io::Cursor<Vec<u8>>>,
    limits: PackageLimits,
    /// Entry paths in archive order, captured once so callers need not re-walk.
    paths: Vec<String>,
}

impl std::fmt::Debug for Package {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Package")
            .field("entries", &self.paths.len())
            .field("limits", &self.limits)
            .finish()
    }
}

impl Package {
    /// Open a container from any byte source.
    pub fn open(source: &dyn ByteSource) -> Result<Self> {
        Self::open_with_limits(source, PackageLimits::default())
    }

    /// Open a container with explicit limits.
    pub fn open_with_limits(source: &dyn ByteSource, limits: PackageLimits) -> Result<Self> {
        // The archive is buffered rather than streamed: every OFD read touches the
        // central directory first, and owning the bytes keeps `Package` free of
        // the source's lifetime so callers can drop it immediately.
        let data = source.read_all()?;
        let cursor = std::io::Cursor::new(data);
        let archive = zip::ZipArchive::new(cursor).map_err(map_zip_error)?;
        Self::from_archive(archive, limits)
    }

    /// Open a container from a byte slice, the common case for tests and uploads.
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        let source = steleofd_io::SliceSource::new(data, "package");
        Self::open(&source)
    }

    fn from_archive(
        mut archive: zip::ZipArchive<std::io::Cursor<Vec<u8>>>,
        limits: PackageLimits,
    ) -> Result<Self> {
        let count = archive.len();
        if count > limits.max_entry_count {
            return Err(PackageError::EntryCountLimit {
                actual: count,
                limit: limits.max_entry_count,
            }
            .into());
        }

        let mut paths = Vec::with_capacity(count);
        let mut seen = HashSet::with_capacity(count);

        for i in 0..count {
            let file = archive.by_index(i).map_err(map_zip_error)?;

            // Reject traversal rather than sanitising: a container that tries it
            // is malformed or hostile, and rewriting the path would hide that
            // from every layer above.
            let owned = match file.enclosed_name() {
                Some(p) => p,
                None => {
                    return Err(PackageError::IllegalPath {
                        path: file.name().to_string(),
                    }
                    .into());
                }
            };
            let path = normalize(&owned);

            // Duplicate names are legal in ZIP but ambiguous for OFD lookups, so
            // the first occurrence wins and the collision is reported.
            if seen.insert(path.clone()) {
                paths.push(path);
            } else {
                tracing::warn!(path, "duplicate archive entry, keeping the first");
            }
        }

        Ok(Self {
            archive,
            limits,
            paths,
        })
    }

    /// Entry paths in archive order, directories excluded.
    pub fn entries(&self) -> &[String] {
        &self.paths
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.paths.len()
    }

    /// Whether the archive holds no entries.
    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    /// Limits in force for this container.
    pub fn limits(&self) -> PackageLimits {
        self.limits
    }

    /// Whether a path exists in the archive.
    pub fn contains(&self, path: &str) -> bool {
        self.paths.iter().any(|p| p == path)
    }

    /// Read one entry in full.
    pub fn read(&mut self, path: &str) -> Result<Vec<u8>> {
        let mut file = self.archive.by_name(path).map_err(|e| match e {
            zip::result::ZipError::FileNotFound => PackageError::MissingEntry {
                entry: path.to_string(),
            }
            .into(),
            other => map_zip_error(other),
        })?;

        // Read behind a hard ceiling rather than trusting the declared size: a
        // crafted header can understate it and inflate past the limit.
        let limit = self.limits.max_entry_size;
        let declared = file.size();
        if declared > limit {
            return Err(PackageError::DecompressionLimit {
                actual: declared,
                limit,
            }
            .into());
        }

        let mut buf = Vec::with_capacity(declared as usize);
        file.by_ref().take(limit + 1).read_to_end(&mut buf)?;
        if buf.len() as u64 > limit {
            return Err(PackageError::DecompressionLimit {
                actual: buf.len() as u64,
                limit,
            }
            .into());
        }
        Ok(buf)
    }

    /// Read one entry as text, rejecting invalid UTF-8.
    /// OFD XML is UTF-8 by specification, so a failure here means corruption.
    pub fn read_string(&mut self, path: &str) -> Result<String> {
        let bytes = self.read(path)?;
        String::from_utf8(bytes).map_err(|e| {
            PackageError::Corrupted {
                reason: format!("{path} is not valid UTF-8: {e}"),
            }
            .into()
        })
    }

    /// Read an entry, or `None` when it is absent.
    pub fn read_optional(&mut self, path: &str) -> Result<Option<Vec<u8>>> {
        if !self.contains(path) {
            return Ok(None);
        }
        self.read(path).map(Some)
    }

    /// Whether the archive has the mandated entry point.
    pub fn has_entry_point(&self) -> bool {
        self.contains(ENTRY_POINT)
    }

    /// Read the entry point, failing when it is absent.
    pub fn entry_point(&mut self) -> Result<String> {
        if !self.has_entry_point() {
            return Err(OfdError::Format(steleofd_error::FormatError::NotOfd));
        }
        self.read_string(ENTRY_POINT)
    }

    /// Document directory names, sorted, such as `Doc_0`.
    pub fn document_dirs(&self) -> Vec<String> {
        let mut dirs: Vec<String> = self
            .paths
            .iter()
            .filter_map(|p| p.split_once('/'))
            .map(|(head, _)| head)
            .filter(|h| h.starts_with(DOCS_PREFIX))
            .map(str::to_string)
            .collect();
        dirs.sort_unstable();
        dirs.dedup();
        dirs
    }

    /// Entries under one document directory.
    pub fn entries_under(&self, prefix: &str) -> Vec<&str> {
        let needle = format!("{prefix}/");
        self.paths
            .iter()
            .filter(|p| p.starts_with(&needle))
            .map(String::as_str)
            .collect()
    }

    /// Classify an entry by its path.
    pub fn kind_of(&self, path: &str) -> Option<EntryKind> {
        EntryKind::from_path(path)
    }
}

/// Strip a leading `./` and collapse backslashes so lookups are predictable.
/// `enclosed_name` already guarantees the path stays inside the root, so this
/// only removes cosmetic differences between writers.
fn normalize(path: &Path) -> String {
    let s = path.to_string_lossy().replace('\\', "/");
    s.strip_prefix("./").unwrap_or(&s).to_string()
}

/// Map a `zip` failure onto the container taxonomy.
fn map_zip_error(e: zip::result::ZipError) -> OfdError {
    let reason = match &e {
        zip::result::ZipError::Io(io) => io.to_string(),
        other => other.to_string(),
    };
    PackageError::Corrupted { reason }.into()
}

/// Build a container in memory, used by tests and by the writer path.
/// Kept here so fixtures share one definition of a well-formed archive instead
/// of every test re-deriving the ZIP layout.
pub fn build_archive(entries: &[(&str, &[u8])]) -> Result<Vec<u8>> {
    use std::io::Write;

    let mut cursor = std::io::Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut cursor);
        let options: zip::write::FileOptions<'_, ()> =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (name, data) in entries {
            writer
                .start_file(*name, options)
                .map_err(|e| PackageError::Corrupted {
                    reason: format!("failed to start entry {name}: {e}"),
                })?;
            writer.write_all(data)?;
        }
        writer.finish().map_err(|e| PackageError::Corrupted {
            reason: format!("failed to finish archive: {e}"),
        })?;
    }
    Ok(cursor.into_inner())
}

/// Whether a path is under the `Doc_*` prefix.
pub fn is_document_path(path: &str) -> bool {
    path.starts_with(DOCS_PREFIX)
}

/// Join an archive directory and a file name.
pub fn join(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        name.to_string()
    } else {
        format!("{}/{}", dir.trim_end_matches('/'), name)
    }
}

/// Parent directory of an archive path, or an empty string at the root.
pub fn parent_of(path: &str) -> String {
    match path.rsplit_once('/') {
        Some((head, _)) => head.to_string(),
        None => String::new(),
    }
}

/// The last path segment.
pub fn file_name_of(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Resolve a possibly relative reference against a base directory.
/// XML references inside a document are relative to the file containing them, so
/// parsing must resolve them the way the writer intended.
pub fn resolve(base: &str, reference: &str) -> String {
    if reference.starts_with('/') {
        return reference.trim_start_matches('/').to_string();
    }
    if reference.contains(':') {
        // Absolute ORIs and URLs are kept verbatim; the caller decides.
        return reference.to_string();
    }

    let mut parts: Vec<&str> = if base.is_empty() {
        Vec::new()
    } else {
        base.split('/').filter(|s| !s.is_empty()).collect()
    };

    for seg in reference.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }

    parts.join("/")
}

/// Whether a path looks like a directory entry.
pub fn looks_like_dir(path: &str) -> bool {
    path.ends_with('/')
}

/// Convert an archive path to a `PathBuf` for interop with I/O code.
pub fn to_path_buf(path: &str) -> PathBuf {
    PathBuf::from(path)
}

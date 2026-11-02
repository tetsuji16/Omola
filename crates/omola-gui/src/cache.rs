//! On-disk cache for the library class index.
//!
//! Loading a library's full class index (`getClassNames(..., recursive=true)`) makes OMC
//! parse the whole package, which takes seconds for the Modelica Standard Library. The
//! index only changes when the installed library changes, so it is cached per
//! `(name, requested version)` and validated against the source file's size and
//! modification time before it is reused.
//!
//! The cache is advisory: any I/O or parse failure silently falls back to querying OMC.

use crate::library::{ClassEntry, LibrarySpec, LoadedLibrary};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

/// Bump when the serialized layout changes; older files are then ignored.
const SCHEMA_VERSION: u32 = 1;

/// Serialized form of [`LoadedLibrary`], kept separate so the on-disk format is explicit
/// and does not silently change when the in-memory types evolve.
#[derive(Debug, Serialize, Deserialize)]
struct CachedLibrary {
    schema_version: u32,
    name: String,
    requested_version: Option<String>,
    version: String,
    source_file: PathBuf,
    /// Size of `source_file` when the index was produced.
    source_size: u64,
    /// Modification time of `source_file` (seconds since the Unix epoch).
    source_mtime: u64,
    classes: Vec<CachedClass>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CachedClass {
    name: String,
    restriction: String,
    comment: String,
}

/// Root directory for Omola's GUI caches.
///
/// `OMOLA_CACHE_DIR` overrides the platform location (used by tests and packaging).
pub fn cache_root() -> PathBuf {
    if let Some(dir) = std::env::var_os("OMOLA_CACHE_DIR") {
        if !dir.is_empty() {
            return PathBuf::from(dir);
        }
    }
    platform_cache_dir().unwrap_or_else(|| std::env::temp_dir().join("omola-cache"))
}

#[cfg(windows)]
fn platform_cache_dir() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .filter(|v| !v.is_empty())
        .map(|v| PathBuf::from(v).join("omola").join("cache"))
}

#[cfg(target_os = "macos")]
fn platform_cache_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").filter(|v| !v.is_empty()).map(|v| {
        PathBuf::from(v)
            .join("Library")
            .join("Caches")
            .join("omola")
    })
}

#[cfg(not(any(windows, target_os = "macos")))]
fn platform_cache_dir() -> Option<PathBuf> {
    std::env::var_os("XDG_CACHE_HOME")
        .filter(|v| !v.is_empty())
        .map(|v| PathBuf::from(v).join("omola"))
        .or_else(|| {
            std::env::var_os("HOME")
                .filter(|v| !v.is_empty())
                .map(|v| PathBuf::from(v).join(".cache").join("omola"))
        })
}

/// File name for a library index. Unsafe characters are replaced so a library name can
/// never escape the cache directory.
pub fn library_cache_path(root: &Path, spec: &LibrarySpec) -> PathBuf {
    let safe = |s: &str| -> String {
        s.chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.') {
                    c
                } else {
                    '_'
                }
            })
            .collect()
    };
    let version = spec.version.as_deref().unwrap_or("any");
    root.join("libraries")
        .join(format!("{}-{}.json", safe(&spec.name), safe(version)))
}

/// Size and modification time of `path`, used to detect a changed installation.
fn file_stamp(path: &Path) -> Option<(u64, u64)> {
    let meta = std::fs::metadata(path).ok()?;
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())?;
    Some((meta.len(), mtime))
}

/// Read a cached index if it is present, well-formed, and still matches the installed
/// library. Returns `None` on any mismatch so the caller falls back to OMC.
pub fn load(root: &Path, spec: &LibrarySpec) -> Option<LoadedLibrary> {
    let path = library_cache_path(root, spec);
    let text = std::fs::read_to_string(&path).ok()?;
    let cached: CachedLibrary = serde_json::from_str(&text).ok()?;
    if cached.schema_version != SCHEMA_VERSION
        || cached.name != spec.name
        || cached.requested_version != spec.version
    {
        return None;
    }
    // The index is only valid while the installed source file is unchanged.
    let (size, mtime) = file_stamp(&cached.source_file)?;
    if size != cached.source_size || mtime != cached.source_mtime {
        return None;
    }
    if cached.classes.is_empty() {
        return None;
    }
    Some(LoadedLibrary {
        spec: spec.clone(),
        version: cached.version,
        source_file: cached.source_file,
        classes: cached
            .classes
            .into_iter()
            .map(|c| ClassEntry {
                qualified_name: c.name,
                restriction: c.restriction,
                comment: c.comment,
            })
            .collect(),
    })
}

/// Write `lib` to the cache. Failures are reported to the caller but are never fatal:
/// a missing cache only costs time on the next start.
pub fn store(root: &Path, lib: &LoadedLibrary) -> std::io::Result<()> {
    let (source_size, source_mtime) = file_stamp(&lib.source_file).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("{} is not readable", lib.source_file.display()),
        )
    })?;
    let cached = CachedLibrary {
        schema_version: SCHEMA_VERSION,
        name: lib.spec.name.clone(),
        requested_version: lib.spec.version.clone(),
        version: lib.version.clone(),
        source_file: lib.source_file.clone(),
        source_size,
        source_mtime,
        classes: lib
            .classes
            .iter()
            .map(|c| CachedClass {
                name: c.qualified_name.clone(),
                restriction: c.restriction.clone(),
                comment: c.comment.clone(),
            })
            .collect(),
    };
    let path = library_cache_path(root, &lib.spec);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string(&cached)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    // Write beside the target and rename so a crash never leaves a half-written index.
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, &path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static NEXT: AtomicU32 = AtomicU32::new(0);

    /// A unique scratch directory per test (no `tempfile` dependency in the GUI crate).
    fn scratch(tag: &str) -> PathBuf {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("omola-cache-test-{}-{tag}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn spec() -> LibrarySpec {
        LibrarySpec::new("Modelica", Some("4.1.0"))
    }

    fn library(source: PathBuf) -> LoadedLibrary {
        LoadedLibrary {
            spec: spec(),
            version: "4.1.0".into(),
            source_file: source,
            classes: vec![
                ClassEntry {
                    qualified_name: "Modelica".into(),
                    restriction: "package".into(),
                    comment: "Modelica Standard Library".into(),
                },
                ClassEntry {
                    qualified_name: "Modelica.Blocks".into(),
                    restriction: "package".into(),
                    comment: String::new(),
                },
            ],
        }
    }

    #[test]
    fn round_trips_through_the_cache() {
        let dir = scratch("roundtrip");
        let source = dir.join("package.mo");
        std::fs::write(&source, "package Modelica end Modelica;").unwrap();
        let root = dir.join("cache");

        store(&root, &library(source.clone())).unwrap();
        let loaded = load(&root, &spec()).expect("cache hit");

        assert_eq!(loaded.version, "4.1.0");
        assert_eq!(loaded.source_file, source);
        assert_eq!(loaded.classes.len(), 2);
        assert_eq!(loaded.classes[0].comment, "Modelica Standard Library");
        assert_eq!(loaded.classes[1].qualified_name, "Modelica.Blocks");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_cache_when_the_library_changed() {
        let dir = scratch("stale");
        let source = dir.join("package.mo");
        std::fs::write(&source, "package Modelica end Modelica;").unwrap();
        let root = dir.join("cache");
        store(&root, &library(source.clone())).unwrap();
        assert!(load(&root, &spec()).is_some());

        // A different file size means a different installation.
        std::fs::write(&source, "package Modelica // reinstalled end Modelica;").unwrap();
        assert!(load(&root, &spec()).is_none(), "stale index was reused");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_or_corrupt_cache_is_ignored() {
        let dir = scratch("corrupt");
        let root = dir.join("cache");
        assert!(load(&root, &spec()).is_none());

        let path = library_cache_path(&root, &spec());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "{ not json").unwrap();
        assert!(load(&root, &spec()).is_none());

        // Valid JSON but the wrong schema version.
        std::fs::write(&path, r#"{"schema_version":999}"#).unwrap();
        assert!(load(&root, &spec()).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cache_path_is_safe_and_stable() {
        let root = Path::new("/cache");
        assert_eq!(
            library_cache_path(root, &spec()),
            Path::new("/cache/libraries/Modelica-4.1.0.json")
        );
        assert_eq!(
            library_cache_path(root, &LibrarySpec::new("Modelica", None)),
            Path::new("/cache/libraries/Modelica-any.json")
        );
        // Path separators cannot appear in the file name, so a name can never escape the
        // cache directory (the ".." inside a file name is harmless without separators).
        let escaped = library_cache_path(root, &LibrarySpec::new("A/B\\C", Some("../x")));
        assert_eq!(escaped.parent().unwrap(), Path::new("/cache/libraries"));
        let file_name = escaped.file_name().unwrap().to_string_lossy().into_owned();
        assert!(!file_name.contains('/'), "{file_name}");
        assert!(!file_name.contains('\\'), "{file_name}");
    }

    #[test]
    fn unreadable_source_is_not_cached() {
        let dir = scratch("missing-source");
        let lib = library(dir.join("does-not-exist.mo"));
        assert!(store(&dir.join("cache"), &lib).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

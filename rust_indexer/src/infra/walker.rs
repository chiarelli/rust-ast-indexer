use std::ffi::OsStr;
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use blake3::Hasher;
use globset::{Glob, GlobSet, GlobSetBuilder};
use walkdir::WalkDir;

use crate::application::protocol::Event;
use crate::domain::types::FileRecord;
use crate::infra::jsonl;
use serde_json::json;

#[derive(Debug, Clone)]
pub struct ScanOptions {
    pub path: PathBuf,
    pub include_patterns: Vec<String>,
    pub ignore_patterns: Vec<String>,
    pub follow_links: bool,
    pub load_ignores: bool,
}

impl ScanOptions {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            include_patterns: Vec::new(),
            ignore_patterns: Vec::new(),
            follow_links: false,
            load_ignores: false,
        }
    }

    pub fn with_include(mut self, pattern: impl Into<String>) -> Self {
        self.include_patterns.push(pattern.into());
        self
    }

    pub fn with_ignore(mut self, pattern: impl Into<String>) -> Self {
        self.ignore_patterns.push(pattern.into());
        self
    }

    pub fn follow_links(mut self, follow: bool) -> Self {
        self.follow_links = follow;
        self
    }

    pub fn with_load_ignores(mut self, load: bool) -> Self {
        self.load_ignores = load;
        self
    }
}

#[derive(Debug)]
pub enum WalkerError {
    Glob(globset::Error),
}

impl From<globset::Error> for WalkerError {
    fn from(value: globset::Error) -> Self {
        WalkerError::Glob(value)
    }
}

impl WalkerError {
    pub fn is_glob_error(&self) -> bool {
        matches!(self, WalkerError::Glob(_))
    }
}

pub fn walk_path(opts: &ScanOptions) -> Result<Vec<FileRecord>, WalkerError> {
    let mut files = Vec::new();
    walk_with_callback(opts, |record| files.push(record))?;
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}

pub fn emit_file_listed_events(
    opts: &ScanOptions,
    job_id: Option<String>,
) -> Result<(), WalkerError> {
    walk_with_callback(opts, |record| {
        let ev = Event {
            protocol_version: "1.0.0".into(),
            r#type: "event".into(),
            event: "file_listed".into(),
            job_id: job_id.clone(),
            payload: Some(json!({
                "file": record
            })),
        };
        jsonl::write_event(&ev);
    })
}

pub fn emit_file_listed_from_records(records: &[FileRecord], job_id: Option<String>) {
    for record in records {
        let ev = Event {
            protocol_version: "1.0.0".into(),
            r#type: "event".into(),
            event: "file_listed".into(),
            job_id: job_id.clone(),
            payload: Some(json!({
                "file": record
            })),
        };
        jsonl::write_event(&ev);
    }
}

fn walk_with_callback<F>(opts: &ScanOptions, mut handler: F) -> Result<(), WalkerError>
where
    F: FnMut(FileRecord),
{
    let include_set = build_globset(&opts.include_patterns)?;
    let mut ignore_patterns = opts.ignore_patterns.clone();
    if opts.load_ignores {
        let loaded = load_ignore_patterns(&opts.path);
        ignore_patterns.extend(loaded);
        // Always ignore repository ignore files themselves
        ignore_patterns.push(".gitignore".to_string());
        ignore_patterns.push(".crushignore".to_string());
    }
    let ignore_set = build_globset(&ignore_patterns)?;
    let walker = WalkDir::new(&opts.path).follow_links(opts.follow_links);

    for entry in walker.into_iter().flatten() {
        if !entry.file_type().is_file() {
            continue;
        }

        let relative_path = normalize_relative_path(entry.path(), &opts.path);
        if matches_glob(&ignore_set, &relative_path) {
            continue;
        }

        if let Some(include) = &include_set {
            if !include.is_match(&relative_path) {
                continue;
            }
        }

        let metadata = match entry.metadata() {
            Ok(value) => value,
            Err(_) => continue,
        };

        let hash = match hash_file(entry.path()) {
            Ok(value) => value,
            Err(_) => continue,
        };

        let mtime = metadata
            .modified()
            .ok()
            .and_then(|timestamp| timestamp.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs())
            .unwrap_or(0);

        handler(FileRecord {
            path: relative_path,
            size: metadata.len(),
            mtime,
            hash,
            language: detect_language(entry.path()),
        });
    }

    Ok(())
}

fn load_ignore_patterns(root: &Path) -> Vec<String> {
    let mut patterns = Vec::new();
    for fname in &[".gitignore", ".crushignore"] {
        let path = root.join(fname);
        if let Ok(content) = std::fs::read_to_string(&path) {
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                patterns.push(line.to_string());
            }
        }
    }
    patterns
}

fn build_globset(patterns: &[String]) -> Result<Option<GlobSet>, globset::Error> {
    if patterns.is_empty() {
        return Ok(None);
    }

    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        builder.add(Glob::new(pattern)?);
    }

    Ok(Some(builder.build()?))
}

fn matches_glob(set: &Option<GlobSet>, value: &str) -> bool {
    set.as_ref().is_some_and(|set| set.is_match(value))
}

fn normalize_relative_path(path: &Path, base: &Path) -> String {
    path.strip_prefix(base)
        .unwrap_or(path)
        .iter()
        .map(|component| component.to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn detect_language(path: &Path) -> Option<String> {
    path.extension()
        .and_then(OsStr::to_str)
        .and_then(|ext| match ext.to_lowercase().as_str() {
            "rs" => Some("rust"),
            "go" => Some("go"),
            "py" => Some("python"),
            "ts" => Some("typescript"),
            "tsx" => Some("typescript"),
            "js" => Some("javascript"),
            "java" => Some("java"),
            "c" => Some("c"),
            "cpp" => Some("cpp"),
            "h" => Some("c"),
            "cs" => Some("csharp"),
            "swift" => Some("swift"),
            "kt" => Some("kotlin"),
            "kts" => Some("kotlin"),
            "php" => Some("php"),
            "rb" => Some("ruby"),
            _ => None,
        })
        .map(String::from)
}

fn hash_file(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Hasher::new();
    let mut buffer = [0u8; 8192];

    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(hasher.finalize().to_hex().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn collects_records_with_metadata() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("lib.rs");
        let contents = b"fn main() {}\n";
        std::fs::write(&file_path, contents).unwrap();

        let opts = ScanOptions::new(dir.path());
        let records = walk_path(&opts).unwrap();

        assert_eq!(records.len(), 1);
        let record = &records[0];
        assert_eq!(record.path, "lib.rs");
        assert_eq!(record.language.as_deref(), Some("rust"));
        assert_eq!(record.size, contents.len() as u64);
        assert_eq!(record.hash, blake3::hash(contents).to_hex().to_string());
        assert!(record.mtime > 0);
    }

    #[test]
    fn respects_include_patterns() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("main.rs"), b"fn main() {}").unwrap();
        std::fs::write(dir.path().join("README.md"), b"# hi").unwrap();

        let opts = ScanOptions::new(dir.path()).with_include("**/*.rs");
        let records = walk_path(&opts).unwrap();

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].path, "main.rs");
    }

    #[test]
    fn ignores_patterns() {
        let dir = tempdir().unwrap();
        let keep_dir = dir.path().join("src");
        let ignored_dir = dir.path().join("target");
        std::fs::create_dir_all(&keep_dir).unwrap();
        std::fs::create_dir_all(&ignored_dir).unwrap();
        std::fs::write(keep_dir.join("lib.rs"), b"fn main() { println!(\"hi\"); }").unwrap();
        std::fs::write(
            ignored_dir.join("secret.rs"),
            b"pub const SECRET: i32 = 123;",
        )
        .unwrap();

        let opts = ScanOptions::new(dir.path()).with_ignore("target/**");
        let records = walk_path(&opts).unwrap();

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].path, "src/lib.rs");
    }

    #[test]
    fn invalid_include_pattern_returns_error() {
        let dir = tempdir().unwrap();
        let opts = ScanOptions::new(dir.path()).with_include("[");
        assert!(matches!(walk_path(&opts), Err(WalkerError::Glob(_))));
    }

    #[test]
    fn loads_gitignore_patterns_when_enabled() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join(".gitignore"), "secret.txt\n").unwrap();
        std::fs::write(dir.path().join("secret.txt"), b"topsecret").unwrap();
        std::fs::write(dir.path().join("visible.txt"), b"ok").unwrap();

        let opts = ScanOptions::new(dir.path()).with_load_ignores(true);
        let records = walk_path(&opts).unwrap();

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].path, "visible.txt");
    }
}

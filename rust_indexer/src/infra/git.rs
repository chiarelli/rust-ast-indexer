use std::io;
use std::process::Command;

#[derive(Debug)]
pub enum GitError {
    GitNotFound,
    NotARepository,
    Io(io::Error),
    CommandFailed(String),
    Other(String),
}

pub type Result<T> = std::result::Result<T, GitError>;

fn run_git_command(dir: &str, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .map_err(GitError::Io)?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        if stderr.to_lowercase().contains("not a git repository")
            || stderr.contains("Repository not found")
        {
            return Err(GitError::NotARepository);
        }
        return Err(GitError::CommandFailed(stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    Ok(stdout)
}

/// Returns tracked files relative to repository root
pub fn emit_git_tracked_files(path: &str) -> Result<Vec<String>> {
    let out = run_git_command(path, &["ls-files", "--exclude-standard"])?;
    let files = out
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    Ok(files)
}

/// Returns tracked files plus untracked-but-not-ignored files, honoring the
/// full git ignore stack (nested `.gitignore`, `.git/info/exclude`,
/// `core.excludesFile`). Paths are relative to `path` (the caller's scan root),
/// so reusing them mirrors `walk_path`'s relative-path contract.
///
/// Used as the file source when `load_ignores` is enabled, so the indexer sees
/// the same working-set the repository actually tracks.
pub fn emit_git_visible_files(path: &str) -> Result<Vec<String>> {
    let out = run_git_command(path, &["ls-files", "--cached", "--others", "--exclude-standard"])?;
    let files = out
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    Ok(files)
}

/// Returns files changed between two refs (names only)
pub fn get_git_diff_files(path: &str, from: &str, to: &str) -> Result<Vec<String>> {
    let args = ["diff", "--name-only", from, to];
    let out = run_git_command(path, &args)?;
    let files = out
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use std::process::Command;
    use tempfile::tempdir;

    fn init_repo(path: &std::path::Path) {
        let p = path.to_str().unwrap();
        Command::new("git")
            .arg("-C")
            .arg(p)
            .arg("init")
            .output()
            .expect("git init failed");
        Command::new("git")
            .arg("-C")
            .arg(p)
            .args(["config", "user.email", "you@example.com"])
            .output()
            .unwrap();
        Command::new("git")
            .arg("-C")
            .arg(p)
            .args(["config", "user.name", "Tester"])
            .output()
            .unwrap();
    }

    #[test]
    fn tracked_files_empty_repo() {
        let td = tempdir().unwrap();
        init_repo(td.path());
        let files = emit_git_tracked_files(td.path().to_str().unwrap()).unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn tracked_files_after_add_commit() {
        let td = tempdir().unwrap();
        init_repo(td.path());
        let file_path = td.path().join("foo.txt");
        let mut f = File::create(&file_path).unwrap();
        writeln!(f, "hello").unwrap();
        Command::new("git")
            .arg("-C")
            .arg(td.path())
            .arg("add")
            .arg(".")
            .output()
            .unwrap();
        Command::new("git")
            .arg("-C")
            .arg(td.path())
            .arg("commit")
            .arg("-m")
            .arg("init")
            .output()
            .unwrap();

        let files = emit_git_tracked_files(td.path().to_str().unwrap()).unwrap();
        assert_eq!(files, vec!["foo.txt".to_string()]);
    }

    #[test]
    fn diff_files_between_commits() {
        let td = tempdir().unwrap();
        init_repo(td.path());
        let file_path = td.path().join("a.txt");
        File::create(&file_path).unwrap();
        Command::new("git")
            .arg("-C")
            .arg(td.path())
            .arg("add")
            .arg(".")
            .output()
            .unwrap();
        Command::new("git")
            .arg("-C")
            .arg(td.path())
            .arg("commit")
            .arg("-m")
            .arg("first")
            .output()
            .unwrap();
        Command::new("git")
            .arg("-C")
            .arg(td.path())
            .arg("tag")
            .arg("v1")
            .output()
            .unwrap();

        let mut f = File::create(&file_path).unwrap();
        writeln!(f, "change").unwrap();
        Command::new("git")
            .arg("-C")
            .arg(td.path())
            .arg("add")
            .arg(".")
            .output()
            .unwrap();
        Command::new("git")
            .arg("-C")
            .arg(td.path())
            .arg("commit")
            .arg("-m")
            .arg("second")
            .output()
            .unwrap();

        let files = get_git_diff_files(td.path().to_str().unwrap(), "v1", "HEAD").unwrap();
        assert!(files.contains(&"a.txt".to_string()));
    }

    #[test]
    fn not_a_repo_returns_error() {
        let td = tempdir().unwrap();
        let res = emit_git_tracked_files(td.path().to_str().unwrap());
        match res {
            Err(GitError::NotARepository) => {}
            other => panic!("expected NotARepository, got {:?}", other),
        }
    }

    #[test]
    fn visible_files_include_tracked_and_untracked_but_exclude_ignored() {
        let td = tempdir().unwrap();
        init_repo(td.path());

        // tracked file
        std::fs::write(td.path().join("tracked.rs"), b"fn a() {}").unwrap();
        // committed .gitignore excluding target/
        std::fs::write(td.path().join(".gitignore"), b"target/\n").unwrap();
        Command::new("git")
            .arg("-C")
            .arg(td.path())
            .args(["add", "."])
            .output()
            .unwrap();
        Command::new("git")
            .arg("-C")
            .arg(td.path())
            .args(["commit", "-m", "init"])
            .output()
            .unwrap();

        // untracked but not ignored
        std::fs::write(td.path().join("untracked.rs"), b"fn b() {}").unwrap();
        // ignored directory/file
        std::fs::create_dir_all(td.path().join("target")).unwrap();
        std::fs::write(td.path().join("target/ignored.rs"), b"fn c() {}").unwrap();

        let files = emit_git_visible_files(td.path().to_str().unwrap()).unwrap();

        assert!(files.contains(&"tracked.rs".to_string()), "missing tracked: {files:?}");
        assert!(
            files.contains(&"untracked.rs".to_string()),
            "missing untracked-non-ignored: {files:?}"
        );
        assert!(
            !files.iter().any(|f| f.starts_with("target/")),
            "ignored file leaked: {files:?}"
        );
    }

    #[test]
    fn visible_files_from_subdir_are_relative_to_subdir() {
        let td = tempdir().unwrap();
        init_repo(td.path());
        std::fs::create_dir_all(td.path().join("pkg")).unwrap();
        std::fs::write(td.path().join("pkg/lib.rs"), b"fn a() {}").unwrap();
        Command::new("git")
            .arg("-C")
            .arg(td.path())
            .args(["add", "."])
            .output()
            .unwrap();
        Command::new("git")
            .arg("-C")
            .arg(td.path())
            .args(["commit", "-m", "init"])
            .output()
            .unwrap();

        let sub = td.path().join("pkg");
        let files = emit_git_visible_files(sub.to_str().unwrap()).unwrap();
        assert_eq!(files, vec!["lib.rs".to_string()]);
    }

    #[test]
    fn visible_files_no_repo_returns_error() {
        let td = tempdir().unwrap();
        match emit_git_visible_files(td.path().to_str().unwrap()) {
            Err(GitError::NotARepository) => {}
            other => panic!("expected NotARepository, got {:?}", other),
        }
    }
}

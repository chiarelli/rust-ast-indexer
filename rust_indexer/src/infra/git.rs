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

/// Run a git command and return its raw stdout BYTES.
///
/// Bytes (not `String`) porque os paths do `ls-files` podem conter qualquer
/// byte não-NUL; converter para UTF-8 com `from_utf8_lossy` corromperia nomes
/// não-UTF8 (U+FFFD) e produziria paths inexistentes.
fn run_git_command_bytes(dir: &std::path::Path, args: &[&str]) -> Result<Vec<u8>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .map_err(|e| {
            if e.kind() == io::ErrorKind::NotFound {
                GitError::GitNotFound
            } else {
                GitError::Io(e)
            }
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        if stderr.to_lowercase().contains("not a git repository")
            || stderr.contains("Repository not found")
        {
            return Err(GitError::NotARepository);
        }
        return Err(GitError::CommandFailed(stderr));
    }

    Ok(output.stdout)
}

/// Split a NUL-separated `git ... -z` output into paths.
///
/// NÃO usa `lines()` nem `trim()`: ambos corrompem paths legítimos (byte NUL é
/// o separador real do `-z`; espaços nas pontas são significativos). Um path
/// não-UTF8 é preservado via `OsString`/`PathBuf`, evitando perda silenciosa.
///
/// Rejeita paths absolutos ou que escapem da raiz (`..`): o stdout do git é
/// entrada externa e um wrapper/alias malicioso não pode fazer o scan sair do
/// escopo. `git ls-files` legítimo nunca emite esses valores.
fn split_nul_paths(bytes: &[u8]) -> Result<Vec<std::path::PathBuf>> {
    let mut out = Vec::new();
    for chunk in bytes.split(|b| *b == 0) {
        if chunk.is_empty() {
            continue;
        }
        let path = {
            #[cfg(unix)]
            {
                use std::os::unix::ffi::OsStrExt;
                std::path::PathBuf::from(std::ffi::OsStr::from_bytes(chunk))
            }
            #[cfg(not(unix))]
            {
                use std::os::windows::ffi::OsStringExt;
                let wide: Vec<u16> = String::from_utf8_lossy(chunk)
                    .encode_utf16()
                    .collect();
                std::path::PathBuf::from(std::ffi::OsString::from_wide(&wide))
            }
        };
        if path.is_absolute()
            || path
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err(GitError::Other(format!(
                "git retornou path fora da raiz: {:?}",
                path
            )));
        }
        out.push(path);
    }
    Ok(out)
}

/// Returns tracked files relative to repository root.
///
/// Retorna `PathBuf` (não `String`) para preservar paths não-UTF8: os bytes
/// originais do git são mantidos e resolvem no FS. Consumidores que exigem
/// `String` devem checar `to_str()` explicitamente — não usar `to_string_lossy`,
/// que corromperia o path para `U+FFFD`.
pub fn emit_git_tracked_files(path: &std::path::Path) -> Result<Vec<std::path::PathBuf>> {
    let out = run_git_command_bytes(path, &["ls-files", "-z", "--exclude-standard"])?;
    split_nul_paths(&out)
}

/// Returns tracked files plus untracked-but-not-ignored files, honoring the
/// full git ignore stack (nested `.gitignore`, `.git/info/exclude`,
/// `core.excludesFile`). Paths are relative to `path` (the caller's scan root),
/// so reusing them mirrors `walk_path`'s relative-path contract.
///
/// Used as the file source when `load_ignores` is enabled, so the indexer sees
/// the same working-set the repository actually tracks.
pub fn emit_git_visible_files(path: &std::path::Path) -> Result<Vec<std::path::PathBuf>> {
    let out = run_git_command_bytes(
        path,
        &["ls-files", "-z", "--cached", "--others", "--exclude-standard"],
    )?;
    split_nul_paths(&out)
}

/// Returns files changed between two refs (names only)
pub fn get_git_diff_files(
    path: &std::path::Path,
    from: &str,
    to: &str,
) -> Result<Vec<std::path::PathBuf>> {
    // `from`/`to` vêm do payload (entrada não confiável). Um valor começando com
    // `-` seria interpretado como OPÇÃO do git (ex.: `--output=<arquivo>`),
    // permitindo escrita fora do escopo. Rejeita vazio/option-like — sem `--`,
    // que em `git diff` transformaria as refs em pathspecs.
    if from.is_empty() || to.is_empty() || from.starts_with('-') || to.starts_with('-') {
        return Err(GitError::Other(format!(
            "git_range inválido (ref vazia ou option-like): from={from:?} to={to:?}"
        )));
    }
    let args = ["diff", "-z", "--name-only", from, to];
    let out = run_git_command_bytes(path, &args)?;
    split_nul_paths(&out)
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
        let files = emit_git_tracked_files(td.path()).unwrap();
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

        let files = emit_git_tracked_files(td.path()).unwrap();
        assert_eq!(files, vec![std::path::PathBuf::from("foo.txt")]);
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

        let files = get_git_diff_files(td.path(), "v1", "HEAD").unwrap();
        assert!(files.contains(&std::path::PathBuf::from("a.txt")));
    }

    /// `git_range` option-like (começando com `-`) é rejeitado: um valor como
    /// `--output=<arquivo>` seria interpretado como opção do git.
    #[test]
    fn diff_files_rejects_option_like_refs() {
        let td = tempdir().unwrap();
        init_repo(td.path());
        assert!(get_git_diff_files(td.path(), "--output=/tmp/evil", "HEAD").is_err());
        assert!(get_git_diff_files(td.path(), "HEAD", "-x").is_err());
        assert!(get_git_diff_files(td.path(), "", "HEAD").is_err());
    }

    #[test]
    fn not_a_repo_returns_error() {
        let td = tempdir().unwrap();
        let res = emit_git_tracked_files(td.path());
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

        let files = emit_git_visible_files(td.path()).unwrap();

        assert!(
            files.contains(&std::path::PathBuf::from("tracked.rs")),
            "missing tracked: {files:?}"
        );
        assert!(
            files.contains(&std::path::PathBuf::from("untracked.rs")),
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
        let files = emit_git_visible_files(&sub).unwrap();
        assert_eq!(files, vec![std::path::PathBuf::from("lib.rs")]);
    }

    #[test]
    fn visible_files_no_repo_returns_error() {
        let td = tempdir().unwrap();
        match emit_git_visible_files(td.path()) {
            Err(GitError::NotARepository) => {}
            other => panic!("expected NotARepository, got {:?}", other),
        }
    }

    /// Regressão: paths com espaço nas pontas, não-ASCII e newline NÃO podem ser
    /// corrompidos (quoting/escape do git) nem sofrer `trim()`. Sem `-z` +
    /// split-NUL, `café.rs` virava `"caf\303\251.rs"` e `trailing.rs ` perdia o
    /// espaço — ambos apontando para arquivos inexistentes.
    #[test]
    fn visible_files_preserve_special_paths() {
        let td = tempdir().unwrap();
        init_repo(td.path());
        for name in [
            "café.rs",
            " leading.rs",
            "trailing.rs ",
            "tab\tname.rs",
            "nl\nname.rs",
        ] {
            std::fs::write(td.path().join(name), b"fn x() {}").unwrap();
        }
        Command::new("git")
            .arg("-C")
            .arg(td.path())
            .args(["add", "-A"])
            .output()
            .unwrap();

        let files = emit_git_visible_files(td.path()).unwrap();
        for name in [
            "café.rs",
            " leading.rs",
            "trailing.rs ",
            "tab\tname.rs",
            "nl\nname.rs",
        ] {
            let expected = std::path::PathBuf::from(name);
            assert!(
                files.contains(&expected),
                "path corrompido/perdido: {name:?}; obtido {files:?}"
            );
            assert!(
                td.path().join(name).exists(),
                "fixture ausente: {name:?}"
            );
        }
        // Cada path retornado deve existir de fato no FS (sem quote/escape).
        for f in &files {
            assert!(
                td.path().join(f).exists(),
                "path retornado não resolve: {f:?}"
            );
        }
    }

    /// Regressão: um path NÃO-UTF8 deve sobreviver como `PathBuf` (bytes
    /// originais) e resolver no FS — sem virar `U+FFFD` via `from_utf8_lossy`.
    #[cfg(unix)]
    #[test]
    fn visible_files_preserve_non_utf8_path_bytes() {
        use std::os::unix::ffi::OsStrExt;
        let td = tempdir().unwrap();
        init_repo(td.path());

        let name = std::ffi::OsStr::from_bytes(b"bad\xFF.rs");
        std::fs::write(td.path().join(name), b"fn x() {}").unwrap();
        Command::new("git")
            .arg("-C")
            .arg(td.path())
            .args(["add", "-A"])
            .output()
            .unwrap();

        let files = emit_git_visible_files(td.path()).unwrap();
        let expected = std::path::PathBuf::from(name);
        assert!(
            files.contains(&expected),
            "path não-UTF8 corrompido/perdido; obtido {files:?}"
        );
        assert!(td.path().join(&expected).exists(), "path não resolve");
    }

    /// `split_nul_paths` aceita paths relativos normais (inclusive nomes que
    /// contêm `..` como texto, ex. `..foo.rs`) e rejeita absolutos/`..` reais.
    #[test]
    fn split_nul_paths_accepts_relative_and_rejects_escapes() {
        let ok = split_nul_paths(b"a/b.rs\x00..foo.rs\x00foo..bar.rs\x00trailing.rs \x00").unwrap();
        assert_eq!(
            ok,
            vec![
                std::path::PathBuf::from("a/b.rs"),
                std::path::PathBuf::from("..foo.rs"),
                std::path::PathBuf::from("foo..bar.rs"),
                std::path::PathBuf::from("trailing.rs "),
            ]
        );

        assert!(split_nul_paths(b"/etc/passwd\x00").is_err());
        assert!(split_nul_paths(b"../evil.rs\x00").is_err());
        assert!(split_nul_paths(b"a/../../evil.rs\x00").is_err());
    }
}

use std::process::Command;

pub fn git_diff_files(path: &str, from: &str, to: &str) -> Vec<String> {
    // invoke shell git: git diff --name-only <from> <to> -- <path>
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .arg("diff")
        .arg("--name-only")
        .arg(from)
        .arg(to)
        .output();

    match output {
        Ok(o) if o.status.success() => {
            let s = String::from_utf8_lossy(&o.stdout);
            s.lines().map(|l| l.to_string()).collect()
        }
        _ => Vec::new(),
    }
}

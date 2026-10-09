//! Resolução de alvo de import: módulo → arquivo.
//!
//! O indexer **não guarda estado entre execuções** — cada invocação é
//! independente e pode receber um subconjunto do repositório (`explicit_files`,
//! ex.: o diff do git). Isso NÃO impede resolver o alvo do import, porque numa
//! mesma execução a árvore é varrida (`walk_path`) **antes** do parse
//! (`application/indexer.rs`): o indexer conhece todos os arquivos do escopo sem
//! precisar parsear nenhum deles.
//!
//! ## Fronteira de responsabilidade (decidida em 2026-10-09)
//!
//! | campo | quem resolve | por quê |
//! |---|---|---|
//! | `to_file` | **indexer** (aqui) | só precisa da lista de arquivos (walk, ~ms); sem estado |
//! | `to_symbol_id` / `callee_symbol_id` | **consumidor** | exige a tabela símbolo→arquivo, que o consumidor já tem (todo chunk indexado) |
//!
//! Por isso `resolved` passa a significar **"arquivo-alvo identificado"**
//! (`to_file.is_some()`), e não mais "o texto parece local".

use std::collections::HashSet;

use crate::domain::types::ImportEdge;

/// Resolve `to_module` para um arquivo presente em `known_files`.
///
/// Retorna o caminho relativo do arquivo-alvo, ou `None` quando não há
/// correspondência **inequívoca** na árvore varrida.
pub fn resolve_import_target(
    from_file: &str,
    language: &str,
    to_module: &str,
    imported_symbol: Option<&str>,
    known_files: &HashSet<String>,
) -> Option<String> {
    match language {
        "rust" => resolve_rust(from_file, to_module, imported_symbol, known_files),
        "typescript" | "javascript" => resolve_ts(from_file, to_module, known_files),
        "python" => resolve_python(from_file, to_module, known_files),
        "java" => resolve_java(to_module, known_files),
        "go" => resolve_go(to_module, known_files),
        _ => None,
    }
}

/// Aplica a resolução a uma aresta: preenche `to_file` e reescreve `resolved`
/// com o significado definitivo ("arquivo-alvo identificado").
pub fn apply_import_resolution(
    edge: &mut ImportEdge,
    language: &str,
    known_files: &HashSet<String>,
) {
    let target = resolve_import_target(
        &edge.from_file,
        language,
        &edge.to_module,
        edge.imported_symbol.as_deref(),
        known_files,
    );
    edge.resolved = target.is_some();
    edge.to_file = target;
}

// ---------------------------------------------------------------- helpers ---

fn dir_of(path: &str) -> String {
    match path.rfind('/') {
        Some(i) => path[..i].to_string(),
        None => String::new(),
    }
}

fn join(base: &str, parts: &[&str]) -> String {
    let mut out = base.trim_end_matches('/').to_string();
    for p in parts {
        if p.is_empty() || *p == "." {
            continue;
        }
        if out.is_empty() {
            out = (*p).to_string();
        } else {
            out.push('/');
            out.push_str(p);
        }
    }
    out
}

/// Colapsa `a/../b` e `.` no caminho.
fn collapse_dots(path: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                out.pop();
            }
            s => out.push(s),
        }
    }
    out.join("/")
}

fn try_with_extensions(base: &str, exts: &[&str], files: &HashSet<String>) -> Option<String> {
    let b = base.trim_matches('/');
    if b.is_empty() {
        return None;
    }
    // caminho já com extensão explícita
    if exts.iter().any(|e| b.ends_with(e)) && files.contains(b) {
        return Some(b.to_string());
    }
    for ext in exts {
        let cand = format!("{b}{ext}");
        if files.contains(&cand) {
            return Some(cand);
        }
    }
    None
}

/// Procura um arquivo cujo caminho TERMINE com `rel` (+ extensões).
/// Só devolve quando há exatamente uma correspondência (determinismo).
fn suffix_match(rel: &str, exts: &[&str], files: &HashSet<String>) -> Option<String> {
    let rel = rel.trim_matches('/');
    if rel.is_empty() {
        return None;
    }
    let mut hits: Vec<String> = Vec::new();
    for f in files {
        let f_norm = f.trim_start_matches("./");
        if exts
            .iter()
            .any(|e| f_norm == format!("{rel}{e}") || f_norm.ends_with(&format!("/{rel}{e}")))
        {
            hits.push(f_norm.to_string());
        }
    }
    hits.sort();
    hits.dedup();
    if hits.len() == 1 {
        Some(hits.remove(0))
    } else {
        None
    }
}

/// Diretórios que contêm `lib.rs` ou `main.rs` = raízes de crate Rust.
fn rust_crate_roots(files: &HashSet<String>) -> Vec<String> {
    let mut roots: Vec<String> = files
        .iter()
        .filter_map(|f| {
            let name = f.rsplit('/').next().unwrap_or("");
            if name == "lib.rs" || name == "main.rs" {
                let d = dir_of(f);
                Some(d)
            } else {
                None
            }
        })
        .collect();
    roots.push(String::new()); // raiz do escopo varrido, sempre tentada
    roots.sort();
    roots.dedup();
    roots
}

// ----------------------------------------------------------------- rust ---

fn resolve_rust(
    from_file: &str,
    to_module: &str,
    imported_symbol: Option<&str>,
    files: &HashSet<String>,
) -> Option<String> {
    let module = to_module.trim();
    if module.is_empty() {
        return None;
    }

    // `normalize_rust_import` pode ter deixado o último segmento em
    // `imported_symbol` (ex.: `use crate::db;` → to_module="crate",
    // imported_symbol="db"). A forma MAIS LONGA é tentada primeiro: em
    // `use self::b;` a forma curta ("self" → diretório do arquivo) casaria
    // `src/a.rs` antes de a forma longa casar `src/a/b.rs`.
    let mut fulls: Vec<String> = Vec::new();
    if let Some(sym) = imported_symbol {
        if !sym.is_empty() {
            fulls.push(format!("{module}::{sym}"));
        }
    }
    fulls.push(module.to_string());

    let roots = rust_crate_roots(files);
    let from_dir = dir_of(from_file);

    for full in &fulls {
        let parts: Vec<&str> = full.split("::").filter(|s| !s.is_empty()).collect();
        if parts.is_empty() {
            continue;
        }
        let mut bases: Vec<String> = Vec::new();
        match parts[0] {
            "crate" => {
                for r in &roots {
                    bases.push(join(r, &parts[1..]));
                }
            }
            "self" => bases.push(join(&from_dir, &parts[1..])),
            "super" => {
                let mut d = from_dir.clone();
                let mut i = 0;
                while i < parts.len() && parts[i] == "super" {
                    d = dir_of(&d);
                    i += 1;
                }
                bases.push(join(&d, &parts[i..]));
            }
            _ => {
                // crate externa ou módulo de topo: tenta as raízes de crate
                for r in &roots {
                    bases.push(join(r, &parts));
                }
            }
        }

        for b in bases {
            // O último segmento pode ser símbolo (função/struct), não módulo:
            // encurta progressivamente até casar um arquivo.
            let mut segs: Vec<&str> = b.split('/').filter(|s| !s.is_empty()).collect();
            while !segs.is_empty() {
                let cand = segs.join("/");
                if let Some(hit) = try_with_extensions(&cand, &[".rs"], files) {
                    return Some(hit);
                }
                let mod_rs = format!("{cand}/mod.rs");
                if files.contains(&mod_rs) {
                    return Some(mod_rs);
                }
                segs.pop();
            }
        }
    }
    None
}

// ----------------------------------------------------------- typescript ---

fn resolve_ts(from_file: &str, to_module: &str, files: &HashSet<String>) -> Option<String> {
    let m = to_module.trim().trim_matches('"').trim_matches('\'');
    if !m.starts_with('.') {
        return None; // especificador "bare" = dependência externa
    }
    let base = collapse_dots(&join(&dir_of(from_file), &m.split('/').collect::<Vec<_>>()));
    try_with_extensions(&base, &[".ts", ".tsx", ".js", ".jsx"], files).or_else(|| {
        [".ts", ".tsx", ".js", ".jsx", ""]
            .iter()
            .map(|e| format!("{base}/index{e}"))
            .find(|c| files.contains(c))
    })
}

// --------------------------------------------------------------- python ---

fn resolve_python(from_file: &str, to_module: &str, files: &HashSet<String>) -> Option<String> {
    let t = to_module.trim().trim_start_matches("from ");
    let module_part = if let Some(rest) = to_module.trim().strip_prefix("from ") {
        match rest.split_once(" import ") {
            Some((m, _)) => m.trim(),
            None => rest.trim(),
        }
    } else if let Some(rest) = to_module.trim().strip_prefix("import ") {
        rest.split(',').next().unwrap_or("").trim()
    } else {
        t.trim()
    };
    if module_part.is_empty() {
        return None;
    }

    if let Some(stripped) = module_part.strip_prefix('.') {
        // relativo: um '.' = diretório do arquivo, cada '.' extra sobe um nível
        let extra = module_part.len() - module_part.trim_start_matches('.').len() - 1;
        let mut d = dir_of(from_file);
        for _ in 0..extra {
            d = dir_of(&d);
        }
        let rest = stripped.trim_start_matches('.');
        if rest.is_empty() {
            let init = join(&d, &["__init__.py"]);
            return if files.contains(&init) {
                Some(init)
            } else {
                None
            };
        }
        let base = join(&d, &rest.split('.').collect::<Vec<_>>());
        return try_with_extensions(&base, &[".py"], files).or_else(|| {
            files
                .contains(&format!("{base}/__init__.py"))
                .then(|| format!("{base}/__init__.py"))
        });
    }

    // absoluto: casa por sufixo de caminho (ex.: `pkg.mod` → `pkg/mod.py`)
    let rel = module_part.replace('.', "/");
    try_with_extensions(&rel, &[".py"], files)
        .or_else(|| suffix_match(&rel, &[".py"], files))
        .or_else(|| suffix_match(&format!("{rel}/__init__"), &[".py"], files))
}

// ----------------------------------------------------------------- java ---

fn resolve_java(to_module: &str, files: &HashSet<String>) -> Option<String> {
    let m = to_module.trim().trim_end_matches(';').trim();
    if m.is_empty() || m.starts_with("java.") || m.starts_with("javax.") {
        return None; // biblioteca padrão não é indexada
    }
    let rel = m.replace('.', "/");
    suffix_match(&rel, &[".java"], files)
}

// ------------------------------------------------------------------- go ---

fn resolve_go(to_module: &str, files: &HashSet<String>) -> Option<String> {
    let m = to_module.trim().trim_matches('"').trim_matches('`');
    if m.is_empty() {
        return None;
    }
    // O import traz o caminho COMPLETO do pacote, com o prefixo do módulo do
    // `go.mod` (ex.: `e2edemo/internal/db`). O diretório local é um SUFIXO
    // desse caminho. Testa do mais específico ao mais curto; o empate (>1
    // arquivo) decide o resultado, não a ordem.
    let segs: Vec<&str> = m.split('/').filter(|s| !s.is_empty()).collect();
    for start in 0..segs.len() {
        let dir = segs[start..].join("/");
        let mut hits: Vec<String> = files
            .iter()
            .filter(|f| f.ends_with(".go") && !f.ends_with("_test.go"))
            .filter(|f| dir_of(f) == dir)
            .cloned()
            .collect();
        hits.sort();
        hits.dedup();
        if hits.len() == 1 {
            return hits.pop();
        }
        if hits.len() > 1 {
            // Pacote com vários arquivos e nenhum candidato mais específico
            // resolveu: escolher um seria arbitrário.
            return None;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files(list: &[&str]) -> HashSet<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    fn edge(from: &str, to_module: &str, sym: Option<&str>) -> ImportEdge {
        ImportEdge {
            id: "ie_test".into(),
            from_file: from.into(),
            to_module: to_module.into(),
            imported_symbol: sym.map(String::from),
            alias: None,
            import_kind: "named".into(),
            location: crate::domain::types::Location {
                start_line: 0,
                start_col: 0,
                end_line: 0,
                end_col: 0,
            },
            resolved: false,
            to_file: None,
        }
    }

    // ---- rust ----

    #[test]
    fn rust_crate_module_resolves_to_src_file() {
        let f = files(&["src/lib.rs", "src/db.rs", "src/metrics.rs"]);
        let r = resolve_import_target("src/main.rs", "rust", "crate::db", Some("connect"), &f);
        assert_eq!(r.as_deref(), Some("src/db.rs"));
    }

    #[test]
    fn rust_module_only_without_symbol_resolves() {
        // `use crate::db;` → normalize deixa to_module="crate", symbol="db"
        let f = files(&["src/lib.rs", "src/db.rs"]);
        let r = resolve_import_target("src/main.rs", "rust", "crate", Some("db"), &f);
        assert_eq!(r.as_deref(), Some("src/db.rs"));
    }

    #[test]
    fn rust_mod_rs_resolves() {
        let f = files(&["src/lib.rs", "src/adapters/mod.rs"]);
        let r = resolve_import_target("src/lib.rs", "rust", "crate::adapters", None, &f);
        assert_eq!(r.as_deref(), Some("src/adapters/mod.rs"));
    }

    #[test]
    fn rust_self_and_super_resolve() {
        let f = files(&["src/lib.rs", "src/a.rs", "src/a/b.rs"]);
        let r = resolve_import_target("src/a/b.rs", "rust", "super::a", None, &f);
        assert_eq!(r.as_deref(), Some("src/a.rs"));
        let f2 = files(&["src/lib.rs", "src/a.rs", "src/a/b.rs"]);
        let r2 = resolve_import_target("src/a/b.rs", "rust", "self", Some("b"), &f2);
        assert_eq!(r2.as_deref(), Some("src/a/b.rs"));
    }

    #[test]
    fn rust_external_crate_does_not_resolve() {
        let f = files(&["src/lib.rs", "src/main.rs"]);
        let r = resolve_import_target(
            "src/main.rs",
            "rust",
            "std::collections",
            Some("HashMap"),
            &f,
        );
        assert_eq!(r, None);
    }

    #[test]
    fn rust_function_name_as_last_segment_is_trimmed() {
        let f = files(&["src/lib.rs", "src/utils.rs"]);
        // to_module="crate::utils", symbol="format_name"
        let r = resolve_import_target(
            "src/main.rs",
            "rust",
            "crate::utils",
            Some("format_name"),
            &f,
        );
        assert_eq!(r.as_deref(), Some("src/utils.rs"));
    }

    #[test]
    fn nested_crate_root_is_honoured() {
        // repo com o crate em rust_indexer/src/
        let f = files(&[
            "rust_indexer/src/lib.rs",
            "rust_indexer/src/adapters/mod.rs",
        ]);
        let r = resolve_import_target(
            "rust_indexer/src/lib.rs",
            "rust",
            "crate::adapters",
            None,
            &f,
        );
        assert_eq!(r.as_deref(), Some("rust_indexer/src/adapters/mod.rs"));
    }

    // ---- typescript ----

    #[test]
    fn ts_relative_resolves() {
        let f = files(&["src/app.ts", "src/db.ts"]);
        let r = resolve_import_target("src/app.ts", "typescript", "./db", Some("connect"), &f);
        assert_eq!(r.as_deref(), Some("src/db.ts"));
    }

    #[test]
    fn ts_parent_relative_resolves() {
        let f = files(&["src/app.ts", "lib/util.ts"]);
        let r = resolve_import_target("src/app.ts", "typescript", "../lib/util", None, &f);
        assert_eq!(r.as_deref(), Some("lib/util.ts"));
    }

    #[test]
    fn ts_index_file_resolves() {
        let f = files(&["src/app.ts", "src/db/index.ts"]);
        let r = resolve_import_target("src/app.ts", "typescript", "./db", None, &f);
        assert_eq!(r.as_deref(), Some("src/db/index.ts"));
    }

    #[test]
    fn ts_bare_specifier_does_not_resolve() {
        let f = files(&["src/app.ts"]);
        let r = resolve_import_target("src/app.ts", "typescript", "react", None, &f);
        assert_eq!(r, None);
    }

    // ---- python ----

    #[test]
    fn python_relative_from_import_resolves() {
        let f = files(&["app/main.py", "app/db.py"]);
        let r = resolve_import_target("app/main.py", "python", "from .db import connect", None, &f);
        assert_eq!(r.as_deref(), Some("app/db.py"));
    }

    #[test]
    fn python_absolute_from_import_resolves() {
        let f = files(&["pkg/mod.py", "pkg/main.py"]);
        let r = resolve_import_target("pkg/main.py", "python", "from pkg.mod import x", None, &f);
        assert_eq!(r.as_deref(), Some("pkg/mod.py"));
    }

    #[test]
    fn python_import_statement_resolves() {
        let f = files(&["pkg/util.py", "pkg/main.py"]);
        let r = resolve_import_target("pkg/main.py", "python", "import pkg.util", None, &f);
        assert_eq!(r.as_deref(), Some("pkg/util.py"));
    }

    // ---- java ----

    #[test]
    fn java_class_resolves_by_suffix() {
        let f = files(&[
            "src/main/java/com/x/Foo.java",
            "src/main/java/com/x/Bar.java",
        ]);
        let r = resolve_import_target(
            "src/main/java/com/x/Bar.java",
            "java",
            "com.x.Foo",
            None,
            &f,
        );
        assert_eq!(r.as_deref(), Some("src/main/java/com/x/Foo.java"));
    }

    #[test]
    fn java_stdlib_does_not_resolve() {
        let f = files(&["src/main/java/com/x/Foo.java"]);
        let r = resolve_import_target(
            "src/main/java/com/x/Foo.java",
            "java",
            "java.util.List",
            None,
            &f,
        );
        assert_eq!(r, None);
    }

    // ---- go ----

    #[test]
    fn go_single_file_package_resolves() {
        let f = files(&["go.mod", "internal/db/db.go", "cmd/app/main.go"]);
        let r = resolve_import_target("cmd/app/main.go", "go", "e2edemo/internal/db", None, &f);
        assert_eq!(r.as_deref(), Some("internal/db/db.go"));
    }

    #[test]
    fn go_ambiguous_package_does_not_resolve() {
        // dois arquivos no pacote: escolher um seria arbitrário
        let f = files(&["internal/db/db.go", "internal/db/pool.go"]);
        let r = resolve_import_target("cmd/app/main.go", "go", "e2edemo/internal/db", None, &f);
        assert_eq!(r, None);
    }

    // ---- apply ----

    #[test]
    fn apply_sets_resolved_only_when_target_found() {
        let f = files(&["src/lib.rs", "src/db.rs"]);
        let mut ok = edge("src/main.rs", "crate::db", Some("connect"));
        apply_import_resolution(&mut ok, "rust", &f);
        assert_eq!(ok.to_file.as_deref(), Some("src/db.rs"));
        assert!(ok.resolved);

        let mut miss = edge("src/main.rs", "crate::nope", Some("x"));
        apply_import_resolution(&mut miss, "rust", &f);
        assert_eq!(miss.to_file, None);
        assert!(!miss.resolved);
    }
}

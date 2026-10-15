//! Unicidade de `symbol_id` por arquivo.
//!
//! O consumidor (memtier, ADR-002) monta a identidade de um símbolo como
//! `{file_path}:{symbol_id}` e **rejeita o lote inteiro** (HTTP 422) quando há
//! duplicidade dentro de um arquivo. Os adapters derivam o nome do símbolo da
//! AST; símbolos anônimos (`function_expression`, `lambda`, ...) e nomes
//! legítimos repetidos (ex.: dois métodos `handle`) produzem o MESMO id.
//!
//! `dedupe_symbol_ids` garante unicidade **determinística** sem introduzir `:`
//! no id (o consumidor resolve o nome por `split(':').pop()`; um `:` extra
//! corromperia nomes legítimos como `cache[1:2]`). Ids sem colisão ficam
//! intactos, preservando a resolução de arestas por nome.

use crate::domain::types::Symbol;
use std::collections::HashMap;

/// Torna os `id` dos símbolos únicos no conjunto. A **primeira** ocorrência de
/// um id colidente mantém o nome base (preserva a resolução de `caller_symbol_id`
/// das arestas, que casam pelo nome); as demais ganham sufixo posicional `@line`
/// (e `@line#k` em colisões na mesma linha).
pub fn dedupe_symbol_ids(symbols: &mut [Symbol]) {
    // Conta as ocorrências de cada (file_path, id) para saber quem colide.
    let mut counts: HashMap<(String, String), usize> = HashMap::new();
    for symbol in symbols.iter() {
        *counts
            .entry((symbol.file_path.clone(), symbol.id.clone()))
            .or_insert(0) += 1;
    }

    // Ordem determinística: o sufixo depende do índice de ocorrência na ordem
    // de travessia (estável) do adapter.
    let mut seen: HashMap<(String, String), usize> = HashMap::new();
    for symbol in symbols.iter_mut() {
        let key = (symbol.file_path.clone(), symbol.id.clone());
        if counts.get(&key).copied().unwrap_or(0) <= 1 {
            continue;
        }
        let occurrence = seen.entry(key).or_insert(0);
        let occ = *occurrence;
        *occurrence += 1;
        // A primeira mantém o nome base — é o que as arestas (`caller_symbol_id`)
        // referenciam. Só as colidências subsequentes são desambiguadas.
        if occ == 0 {
            continue;
        }
        let suffix = if occ == 1 {
            format!("@{}", symbol.start_line)
        } else {
            format!("@{}#{}", symbol.start_line, occ)
        };
        symbol.id = format!("{}{}", symbol.id, suffix);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sym(id: &str, start: usize) -> Symbol {
        Symbol {
            id: format!("f.rs:{id}"),
            name: id.to_string(),
            kind: "function".to_string(),
            scope: None,
            file_path: "f.rs".to_string(),
            start_line: start,
            end_line: start + 1,
            signature: None,
        }
    }

    #[test]
    fn leaves_unique_ids_untouched() {
        let mut syms = vec![sym("alpha", 1), sym("beta", 5)];
        dedupe_symbol_ids(&mut syms);
        assert_eq!(syms[0].id, "f.rs:alpha");
        assert_eq!(syms[1].id, "f.rs:beta");
    }

    #[test]
    fn disambiguates_anonymous_homonyms() {
        let mut syms = vec![sym("function_expression", 3), sym("function_expression", 7)];
        dedupe_symbol_ids(&mut syms);
        assert_ne!(syms[0].id, syms[1].id);
        // A PRIMEIRA ocorrência preserva o nome base (as arestas referenciam-no).
        assert_eq!(syms[0].id, "f.rs:function_expression");
        let bare = syms[1].id.strip_prefix("f.rs:").unwrap();
        assert!(!bare.contains(':'), "bare={bare:?}");
    }

    #[test]
    fn disambiguates_homonyms_on_same_line() {
        let mut syms = vec![sym("lambda", 9), sym("lambda", 9)];
        dedupe_symbol_ids(&mut syms);
        assert_ne!(syms[0].id, syms[1].id);
    }

    #[test]
    fn is_deterministic() {
        let build = || {
            vec![
                sym("dup", 2),
                sym("dup", 2),
                sym("dup", 2),
            ]
        };
        let mut a = build();
        let mut b = build();
        dedupe_symbol_ids(&mut a);
        dedupe_symbol_ids(&mut b);
        let ids_a: Vec<_> = a.iter().map(|s| s.id.clone()).collect();
        let ids_b: Vec<_> = b.iter().map(|s| s.id.clone()).collect();
        assert_eq!(ids_a, ids_b);
    }

    #[test]
    fn keeps_all_ids_unique_after_dedupe() {
        let mut syms = vec![
            sym("dup", 1),
            sym("dup", 1),
            sym("dup", 2),
            sym("unique", 3),
        ];
        dedupe_symbol_ids(&mut syms);
        let ids: Vec<_> = syms.iter().map(|s| s.id.clone()).collect();
        let unique: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(ids.len(), unique.len(), "ids: {ids:?}");
        // A primeira `dup` mantém o nome base.
        assert_eq!(syms[0].id, "f.rs:dup");
    }
}

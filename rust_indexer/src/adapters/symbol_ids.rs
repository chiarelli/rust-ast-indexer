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

use crate::domain::types::{CallEdge, Symbol};
use std::collections::{HashMap, HashSet};

/// Torna os `id` dos símbolos únicos no conjunto. A **primeira** ocorrência de
/// um id colidente mantém o nome base (preserva a resolução de `caller_symbol_id`
/// das arestas, que casam pelo nome); as demais ganham sufixo posicional `@line`
/// (e `@line#k` em colisões na mesma linha).
///
/// A unicidade é garantida contra **todos** os ids do arquivo — inclusive ids
/// que já são renomeados por OUTRO grupo ou cujo nome literal coincide com um
/// sufixo gerado (ex.: um símbolo legítimo chamado `foo@12` colidindo com o
/// sufixo `@12` de um `foo` repetido). Por isso o candidato é gerado em laço até
/// não colidir com nada já reservado.
pub fn dedupe_symbol_ids(symbols: &mut [Symbol]) {
    // Conta as ocorrências de cada (file_path, id) para saber quem colide.
    let mut counts: HashMap<(String, String), usize> = HashMap::new();
    for symbol in symbols.iter() {
        *counts
            .entry((symbol.file_path.clone(), symbol.id.clone()))
            .or_insert(0) += 1;
    }

    // Reserva os ids de grupos SEM colisão: eles não são renomeados e o gerador
    // de sufixos precisa evitá-los. Sem isto, o sufixo `@line` de um grupo pode
    // coincidir com o nome literal único de outro símbolo (bug de duplicata).
    let mut used: HashSet<(String, String)> = HashSet::new();
    for symbol in symbols.iter() {
        let key = (symbol.file_path.clone(), symbol.id.clone());
        if counts.get(&key).copied().unwrap_or(0) <= 1 {
            used.insert(key);
        }
    }

    // Ordem determinística: o sufixo depende do índice de ocorrência na ordem
    // de travessia (estável) do adapter.
    let mut seen: HashMap<(String, String), usize> = HashMap::new();
    for symbol in symbols.iter_mut() {
        let key = (symbol.file_path.clone(), symbol.id.clone());
        if counts.get(&key).copied().unwrap_or(0) <= 1 {
            continue;
        }
        let occurrence = seen.entry(key.clone()).or_insert(0);
        let occ = *occurrence;
        *occurrence += 1;

        // A primeira ocorrência tenta manter o nome base (é o que as arestas
        // `caller_symbol_id` referenciam). Se o base ainda estiver livre, fica.
        let base_free = used.insert(key.clone());
        if occ == 0 && base_free {
            continue;
        }

        // occ==1 → `@line`; occ>=2 → `@line#occ`. O `occ==0` que caiu aqui é o
        // caso raro em que o próprio base já estava reservado por outro grupo:
        // desambigua a partir de `@line` (k=1).
        let mut k = if occ == 0 { 1 } else { occ };
        loop {
            let suffix = if k == 1 {
                format!("@{}", symbol.start_line)
            } else {
                format!("@{}#{}", symbol.start_line, k)
            };
            let candidate = format!("{}{}", key.1, suffix);
            if used.insert((symbol.file_path.clone(), candidate.clone())) {
                symbol.id = candidate;
                break;
            }
            k += 1;
        }
    }
}

/// Remapeia `caller_symbol_id` de cada aresta para o id **dedupado** do símbolo
/// que contém a chamada, resolvido por posição (linha). Sem isto, chamadas
/// originadas no 2º..N-ésimo homônimo (`handle`, `lambda`, ...) apontariam para
/// o chunk do 1º — grafo silenciosamente incorreto.
///
/// A resolução escolhe o símbolo cujo span (`start_line..=end_line`) contém a
/// linha da chamada e cujo `name` é o nome base do caller; havendo aninhamento,
/// vence o mais interno (maior `start_line`, depois menor `end_line`).
///
/// Quando a resolução é **genuinamente ambígua** — dois homônimos com o MESMO
/// span (ex.: mesma linha, sem coluna para desempatar), ou nenhum span
/// contendo a chamada mas com mais de um homônimo do mesmo nome — o
/// `caller_symbol_id` é zerado (`None`) em vez de "chutar" um homônimo. Um
/// caller desconhecido é honesto; um caller errado corrompe o grafo em silêncio.
pub fn remap_callers(calls: &mut [CallEdge], symbols: &[Symbol]) {
    for call in calls.iter_mut() {
        let Some(raw) = call.caller_symbol_id.clone() else {
            continue;
        };
        let base = match call.from_file.as_deref() {
            Some(file) => {
                crate::domain::types::strip_symbol_file_prefix(file, &raw).to_string()
            }
            None => raw.clone(),
        };
        let line = call.location.start_line;

        let candidates: Vec<&Symbol> = symbols
            .iter()
            .filter(|s| s.name == base && s.start_line <= line && line <= s.end_line)
            .collect();

        let Some(best) = candidates.iter().copied().max_by(|a, b| {
            a.start_line
                .cmp(&b.start_line)
                .then_with(|| b.end_line.cmp(&a.end_line))
        }) else {
            // Nenhum span contém a chamada. Se há um único homônimo com esse
            // nome, manter o base é a única resolução possível (sem ambiguidade).
            // Com 2+, preservar o base apontaria sempre para o 1º — zera.
            let same_name = symbols.iter().filter(|s| s.name == base).count();
            if same_name > 1 {
                call.caller_symbol_id = None;
            }
            continue;
        };

        // Empate no span vencedor (ex.: dois homônimos na mesma linha, sem
        // coluna): indesambiguável → zera em vez de escolher arbitrariamente.
        let tied = candidates
            .iter()
            .filter(|s| s.start_line == best.start_line && s.end_line == best.end_line)
            .count()
            > 1;
        call.caller_symbol_id = if tied { None } else { Some(best.id.clone()) };
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

    /// Regressão: um símbolo cujo nome literal coincide com o sufixo gerado
    /// (`foo` repetido na linha 12 produz `foo@12`) NÃO pode colidir com um
    /// símbolo legítimo já chamado `foo@12`.
    #[test]
    fn suffix_does_not_collide_with_preexisting_literal_id() {
        let mut syms = vec![sym("foo", 5), sym("foo", 12), sym("foo@12", 99)];
        dedupe_symbol_ids(&mut syms);
        let ids: Vec<_> = syms.iter().map(|s| s.id.clone()).collect();
        let unique: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(ids.len(), unique.len(), "ids: {ids:?}");
        assert_eq!(syms[0].id, "f.rs:foo");
        assert_eq!(syms[2].id, "f.rs:foo@12", "o id literal deve ficar intacto");
        assert_ne!(syms[1].id, "f.rs:foo@12", "sufixo deve desviar do literal");
    }

    /// Regressão: colisão na variante `#k` contra um id literal `dup@9#2`.
    #[test]
    fn hash_suffix_does_not_collide_with_preexisting_literal_id() {
        let mut syms = vec![sym("dup", 9), sym("dup", 9), sym("dup", 9), sym("dup@9#2", 20)];
        dedupe_symbol_ids(&mut syms);
        let ids: Vec<_> = syms.iter().map(|s| s.id.clone()).collect();
        let unique: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(ids.len(), unique.len(), "ids: {ids:?}");
    }

    /// Regressão: o próprio nome base de um colidente já está reservado por um
    /// grupo sem colisão — a primeira ocorrência não pode reivindicar o base.
    #[test]
    fn first_occurrence_yields_base_when_base_already_reserved() {
        // `x@1` é único; um `x` na linha 1 tenta gerar `@1` e colidiria.
        let mut syms = vec![sym("x", 1), sym("x", 3), sym("x@1", 4)];
        dedupe_symbol_ids(&mut syms);
        let ids: Vec<_> = syms.iter().map(|s| s.id.clone()).collect();
        let unique: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(ids.len(), unique.len(), "ids: {ids:?}");
        assert_eq!(syms[2].id, "f.rs:x@1");
        assert_ne!(syms[0].id, "f.rs:x@1");
    }

    fn call(caller: &str, line: usize) -> CallEdge {
        CallEdge {
            id: format!("ce:{line}"),
            from_file: Some("f.rs".to_string()),
            caller_symbol_id: Some(format!("f.rs:{caller}")),
            callee_name: "g".to_string(),
            callee_symbol_id: None,
            call_kind: "static".to_string(),
            location: crate::domain::types::Location {
                start_line: line,
                start_col: 0,
                end_line: line,
                end_col: 1,
            },
            resolved: false,
        }
    }

    /// Regressão (ALTO): chamadas do 2º homônimo resolvem para o id dedupado
    /// correto (por posição), não para o chunk do 1º.
    #[test]
    fn remap_callers_targets_the_containing_homonym() {
        // Dois `handle`: linhas 10 e 40. Após dedupe: `handle` (1º) e `handle@40`.
        let mut syms = vec![sym("handle", 10), sym("handle", 40)];
        syms[0].end_line = 20;
        syms[1].end_line = 50;
        dedupe_symbol_ids(&mut syms);
        assert_eq!(syms[1].id, "f.rs:handle@40");

        let mut calls = vec![call("handle", 12), call("handle", 42)];
        remap_callers(&mut calls, &syms);
        assert_eq!(calls[0].caller_symbol_id.as_deref(), Some("f.rs:handle"));
        assert_eq!(
            calls[1].caller_symbol_id.as_deref(),
            Some("f.rs:handle@40"),
            "chamada do 2º homônimo deve apontar para o id dedupado"
        );
    }

    /// Sem candidato por posição e havendo um ÚNICO homônimo, preserva o valor
    /// (única resolução possível, sem ambiguidade).
    #[test]
    fn remap_callers_preserves_when_single_symbol_matches_by_name() {
        let syms = vec![sym("handle", 10)];
        let mut calls = vec![call("handle", 999)];
        remap_callers(&mut calls, &syms);
        assert_eq!(calls[0].caller_symbol_id.as_deref(), Some("f.rs:handle"));
    }

    /// Sem candidato por posição e havendo 2+ homônimos, preservar o base
    /// apontaria sempre para o 1º — zera em vez de misatribuir.
    #[test]
    fn remap_callers_clears_when_ambiguous_and_unmatched() {
        let mut syms = vec![sym("handle", 10), sym("handle", 40)];
        syms[0].end_line = 20;
        syms[1].end_line = 50;
        dedupe_symbol_ids(&mut syms);
        let mut calls = vec![call("handle", 999)];
        remap_callers(&mut calls, &syms);
        assert_eq!(calls[0].caller_symbol_id, None);
    }

    /// Dois homônimos com o MESMO span (mesma linha, sem coluna para
    /// desempatar) é indesambiguável — zera em vez de escolher arbitrariamente.
    #[test]
    fn remap_callers_clears_on_identical_span_tie() {
        let mut syms = vec![sym("lambda", 9), sym("lambda", 9)];
        syms[0].end_line = 9;
        syms[1].end_line = 9;
        dedupe_symbol_ids(&mut syms);
        let mut calls = vec![call("lambda", 9)];
        remap_callers(&mut calls, &syms);
        assert_eq!(calls[0].caller_symbol_id, None);
    }

    /// Aninhamento (spans distintos, um contendo o outro) resolve para o mais
    /// interno — determinístico e sem ambiguidade.
    #[test]
    fn remap_callers_prefers_inner_symbol_on_nesting() {
        let mut outer = sym("handle", 1);
        outer.end_line = 30;
        let mut inner = sym("handle", 10);
        inner.end_line = 20;
        // diverge o id sem depender do dedupe (spans distintos)
        inner.name = "handle".to_string();
        inner.id = "f.rs:handle".to_string();
        outer.id = "f.rs:handle".to_string();
        let mut syms = vec![outer, inner];
        dedupe_symbol_ids(&mut syms);
        let mut calls = vec![call("handle", 15)];
        remap_callers(&mut calls, &syms);
        let chosen = calls[0].caller_symbol_id.clone().unwrap();
        // o vencedor deve ser o span mais interno (start 10)
        let inner_id = syms
            .iter()
            .find(|s| s.start_line == 10)
            .map(|s| s.id.clone())
            .unwrap();
        assert_eq!(chosen, inner_id);
    }
}

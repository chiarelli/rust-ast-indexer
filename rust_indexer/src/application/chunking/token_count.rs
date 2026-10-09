use std::collections::HashMap;

use serde_json::Value;

#[cfg(feature = "token_counting")]
use std::sync::{Arc, OnceLock};

#[cfg(feature = "token_counting")]
use wordchipper::{disk_cache::WordchipperDiskCache, load_vocab, Tokenizer, TokenizerOptions};

#[cfg(feature = "token_counting")]
pub fn maybe_token_count(text: &str) -> Option<usize> {
    static TOKENIZER: OnceLock<Option<Arc<Tokenizer<u32>>>> = OnceLock::new();
    let tokenizer = TOKENIZER.get_or_init(load_tokenizer).as_ref()?;
    tokenizer
        .encoder()
        .try_encode(text, None)
        .ok()
        .map(|tokens| tokens.len())
}

#[cfg(feature = "token_counting")]
fn load_tokenizer() -> Option<Arc<Tokenizer<u32>>> {
    let mut disk_cache = WordchipperDiskCache::default();
    let loaded = load_vocab("openai:o200k_harmony", &mut disk_cache).ok()?;
    Some(TokenizerOptions::default().build(loaded.vocab().clone()))
}

#[cfg(not(feature = "token_counting"))]
pub fn maybe_token_count(_text: &str) -> Option<usize> {
    None
}

pub fn apply_token_count(metadata: &mut HashMap<String, Value>, text: &str) {
    if let Some(token_count) = maybe_token_count(text) {
        metadata.insert(
            "token_count".to_string(),
            Value::Number(serde_json::Number::from(token_count as u64)),
        );
    }
}

#[cfg(all(test, feature = "token_counting"))]
mod tests {
    use super::*;

    /// O tokenizer carrega o vocabulario `openai:o200k_harmony` via wordchipper,
    /// que baixa e cacheia em disco na primeira vez. Se a maquina nao tiver
    /// rede nem cache, `maybe_token_count` devolve `None` **em silencio** — por
    /// isso o teste de plumbing abaixo e tolerante a ambiente, e este aqui
    /// apenas reporta o estado (sem quebrar um CI sem rede).
    #[test]
    fn tokenizer_reports_availability() {
        match maybe_token_count("fn main() {}") {
            Some(n) => eprintln!("tokenizer disponivel ({} tokens)", n),
            None => eprintln!("AVISO: tokenizer indisponivel (vocabulario sem rede/cache)"),
        }
    }

    #[test]
    fn apply_token_count_writes_metadata_when_tokenizer_is_available() {
        let mut md = HashMap::new();
        apply_token_count(&mut md, "fn main() {}");
        if maybe_token_count("fn main() {}").is_none() {
            eprintln!("AVISO: pulando — tokenizer indisponivel neste ambiente");
            return;
        }
        assert!(
            md.contains_key("token_count"),
            "tokenizer disponivel mas metadata ficou sem token_count: {:?}",
            md
        );
        assert!(md["token_count"].as_u64().unwrap() > 0);
    }
}

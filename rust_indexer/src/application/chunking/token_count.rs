use std::collections::HashMap;

use serde_json::Value;

#[cfg(feature = "token_counting")]
use std::sync::{Arc, OnceLock};

#[cfg(feature = "token_counting")]
use wordchipper::{disk_cache::WordchipperDiskCache, load_vocab, TokenEncoder, Tokenizer, TokenizerOptions};

#[cfg(feature = "token_counting")]
pub fn maybe_token_count(text: &str) -> Option<usize> {
    static TOKENIZER: OnceLock<Option<Arc<Tokenizer<u32>>>> = OnceLock::new();
    let tokenizer = TOKENIZER.get_or_init(load_tokenizer).as_ref()?;
    tokenizer.encoder().try_encode(text, None).ok().map(|tokens| tokens.len())
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

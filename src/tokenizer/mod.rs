// src/tokenizer/mod.rs
// Placeholder tokenizer implementation for ast-pack.

pub struct Tokenizer {};

impl Tokenizer {
    /// Creates a new tokenizer. Currently a stub that always succeeds.
    pub fn new(_source: &str) -> Result<Self, std::io::Error> {
        Ok(Self {})
    }

    /// Returns a dummy token count. Real implementation will return actual token count.
    pub fn count(&self, _source: &str) -> usize {
        0
    }
}


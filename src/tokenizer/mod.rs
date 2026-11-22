use std::sync::OnceLock;

use tiktoken_rs::CoreBPE;

use crate::error::PackError;

pub struct Tokenizer {
    kind: TokenizerKind,
    bpe: &'static CoreBPE,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenizerKind {
    O200k,
    Cl100k,
}

impl Tokenizer {
    pub fn new(name: &str) -> Result<Self, PackError> {
        let kind = match name {
            "o200k" | "o200k_base" => TokenizerKind::O200k,
            "cl100k" | "cl100k_base" => TokenizerKind::Cl100k,
            other => {
                return Err(PackError::msg(format!(
                    "unknown tokenizer `{other}` (expected o200k or cl100k)"
                )))
            }
        };
        let bpe = match kind {
            TokenizerKind::O200k => o200k(),
            TokenizerKind::Cl100k => cl100k(),
        }?;
        Ok(Self { kind, bpe })
    }

    pub fn kind(&self) -> TokenizerKind {
        self.kind
    }

    pub fn name(&self) -> &'static str {
        match self.kind {
            TokenizerKind::O200k => "o200k",
            TokenizerKind::Cl100k => "cl100k",
        }
    }

    pub fn count(&self, text: &str) -> usize {
        if text.is_empty() {
            return 0;
        }
        self.bpe.encode_ordinary(text).len()
    }
}

fn o200k() -> Result<&'static CoreBPE, PackError> {
    static CELL: OnceLock<CoreBPE> = OnceLock::new();
    if let Some(existing) = CELL.get() {
        return Ok(existing);
    }
    let bpe = tiktoken_rs::o200k_base().map_err(|err| PackError::msg(format!("tokenizer: {err}")))?;
    Ok(CELL.get_or_init(|| bpe))
}

fn cl100k() -> Result<&'static CoreBPE, PackError> {
    static CELL: OnceLock<CoreBPE> = OnceLock::new();
    if let Some(existing) = CELL.get() {
        return Ok(existing);
    }
    let bpe = tiktoken_rs::cl100k_base().map_err(|err| PackError::msg(format!("tokenizer: {err}")))?;
    Ok(CELL.get_or_init(|| bpe))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_are_monotonic() {
        let tok = Tokenizer::new("o200k").unwrap();
        let hello = tok.count("hello");
        let more = tok.count("hello world");
        assert!(hello >= 1);
        assert!(more >= hello);
        assert_eq!(tok.count(""), 0);
    }
}

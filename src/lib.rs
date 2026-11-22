//! ast-pack folds a repository into a hard LLM token budget and unfolds any
//! pruned body on demand.

pub mod cache;
pub mod cli;
pub mod diff;
pub mod error;
pub mod fold;
pub mod mcp;
pub mod mode;
pub mod model;
pub mod parse;
pub mod pipeline;
pub mod render;
pub mod setup;
pub mod tokenizer;
pub mod utils;
pub mod walk;

pub use error::PackError;
pub use fold::{unfold, Unfolded};
pub use mode::PackMode;
pub use pipeline::{pack, FoldSummary, PackOptions, PackOutput};
pub use render::Format;
pub use tokenizer::Tokenizer;

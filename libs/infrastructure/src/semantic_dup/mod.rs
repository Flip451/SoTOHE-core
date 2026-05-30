//! Infrastructure adapters for the semantic duplicate detection feature.
//!
//! Implements the secondary ports defined in `usecase::semantic_dup`:
//!
//! - [`embedding::FastEmbedAdapter`]: computes code embeddings via
//!   fastembed-rs (ONNX Runtime, Jina v2 base code model, synchronous API).
//! - [`index::LanceDbSemanticIndexAdapter`]: stores and searches embeddings
//!   in a local LanceDB vector database.
//!
//! Also provides a standalone workspace scanner:
//!
//! - [`extractor::extract_code_fragments`]: walks a workspace root recursively,
//!   finds `*.rs` files, and yields item-level [`domain::semantic_dup::CodeFragment`]
//!   values for use by the CLI composition root.

pub mod embedding;
pub mod extractor;
pub mod index;

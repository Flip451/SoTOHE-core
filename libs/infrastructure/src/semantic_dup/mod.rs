//! Infrastructure adapters for the semantic duplicate detection feature.
//!
//! Implements the secondary ports defined in `usecase::semantic_dup`:
//!
//! - [`embedding::FastEmbedAdapter`]: computes code embeddings via
//!   fastembed-rs (ONNX Runtime, Jina v2 base code model, synchronous API).
//! - [`index::LanceDbSemanticIndexAdapter`]: stores and searches embeddings
//!   in a local LanceDB vector database.

pub mod embedding;
pub mod index;

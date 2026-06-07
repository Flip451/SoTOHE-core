//! Pure helper functions that do not require a `ParseCtx`.

use rustdoc_types::{Id, Path, Type};

use super::constants::UNRESOLVED_CRATE_ID;

// ---------------------------------------------------------------------------
// Unresolved marker
// ---------------------------------------------------------------------------

/// Builds an "unresolved marker" `rustdoc_types::Type`.
///
/// Uses sentinel crate_id `u32::MAX` to mark identifiers that could not be
/// resolved at A-codec time (open-world). Closed-world validation in Phase 1
/// will reject any markers that remain after Delete processing.
#[must_use]
pub(crate) fn unresolved_type(name: &str) -> Type {
    Type::ResolvedPath(Path { path: name.to_string(), id: Id(UNRESOLVED_CRATE_ID), args: None })
}

// ---------------------------------------------------------------------------
// Canonical path helpers
// ---------------------------------------------------------------------------

/// Returns the canonical `std::module::TypeName` path for a short name that
/// appears in `STD_PRELUDE_TYPES`.
///
/// Using full canonical paths lets downstream consumers distinguish real
/// standard-library types (which have a known module hierarchy) from truly
/// unresolved identifiers that also carry `UNRESOLVED_CRATE_ID`.
#[must_use]
pub(crate) fn std_canonical_path(short_name: &str) -> String {
    match short_name {
        "Vec" => "std::vec::Vec",
        "Option" => "std::option::Option",
        "Result" => "std::result::Result",
        "String" => "std::string::String",
        "Box" => "std::boxed::Box",
        "Iterator" => "std::iter::Iterator",
        "Default" => "std::default::Default",
        "Clone" => "std::clone::Clone",
        "Copy" => "std::marker::Copy",
        "Debug" => "std::fmt::Debug",
        "Display" => "std::fmt::Display",
        "PartialEq" => "std::cmp::PartialEq",
        "Eq" => "std::cmp::Eq",
        "Hash" => "std::hash::Hash",
        "Ord" => "std::cmp::Ord",
        "PartialOrd" => "std::cmp::PartialOrd",
        "Send" => "std::marker::Send",
        "Sync" => "std::marker::Sync",
        "Sized" => "std::marker::Sized",
        "Unpin" => "std::marker::Unpin",
        "Drop" => "std::ops::Drop",
        "AsRef" => "std::convert::AsRef",
        "AsMut" => "std::convert::AsMut",
        "Deref" => "std::ops::Deref",
        "DerefMut" => "std::ops::DerefMut",
        "From" => "std::convert::From",
        "Into" => "std::convert::Into",
        "TryFrom" => "std::convert::TryFrom",
        "TryInto" => "std::convert::TryInto",
        "IntoIterator" => "std::iter::IntoIterator",
        "DoubleEndedIterator" => "std::iter::DoubleEndedIterator",
        "ExactSizeIterator" => "std::iter::ExactSizeIterator",
        "FnOnce" => "std::ops::FnOnce",
        "FnMut" => "std::ops::FnMut",
        "Fn" => "std::ops::Fn",
        "ToString" => "std::string::ToString",
        "ToOwned" => "std::borrow::ToOwned",
        "BorrowMut" => "std::borrow::BorrowMut",
        "Borrow" => "std::borrow::Borrow",
        "Pin" => "std::pin::Pin",
        "PhantomData" => "std::marker::PhantomData",
        "HashMap" => "std::collections::HashMap",
        "BTreeMap" => "std::collections::BTreeMap",
        "HashSet" => "std::collections::HashSet",
        "BTreeSet" => "std::collections::BTreeSet",
        "VecDeque" => "std::collections::VecDeque",
        "LinkedList" => "std::collections::LinkedList",
        "Arc" => "std::sync::Arc",
        "Rc" => "std::rc::Rc",
        "Mutex" => "std::sync::Mutex",
        "RwLock" => "std::sync::RwLock",
        // std::error
        "Error" => "std::error::Error",
        // std::io
        "Read" => "std::io::Read",
        "Write" => "std::io::Write",
        "Seek" => "std::io::Seek",
        "BufRead" => "std::io::BufRead",
        // std::fmt
        "Formatter" => "std::fmt::Formatter",
        // std::ops (arithmetic, bitwise, assignment, index)
        "Add" => "std::ops::Add",
        "Sub" => "std::ops::Sub",
        "Mul" => "std::ops::Mul",
        "Div" => "std::ops::Div",
        "Rem" => "std::ops::Rem",
        "Neg" => "std::ops::Neg",
        "Not" => "std::ops::Not",
        "BitAnd" => "std::ops::BitAnd",
        "BitOr" => "std::ops::BitOr",
        "BitXor" => "std::ops::BitXor",
        "Shl" => "std::ops::Shl",
        "Shr" => "std::ops::Shr",
        "Index" => "std::ops::Index",
        "IndexMut" => "std::ops::IndexMut",
        "AddAssign" => "std::ops::AddAssign",
        "SubAssign" => "std::ops::SubAssign",
        "MulAssign" => "std::ops::MulAssign",
        "DivAssign" => "std::ops::DivAssign",
        "RemAssign" => "std::ops::RemAssign",
        "BitAndAssign" => "std::ops::BitAndAssign",
        "BitOrAssign" => "std::ops::BitOrAssign",
        "BitXorAssign" => "std::ops::BitXorAssign",
        "ShlAssign" => "std::ops::ShlAssign",
        "ShrAssign" => "std::ops::ShrAssign",
        // std::iter (additional)
        "FromIterator" => "std::iter::FromIterator",
        "Extend" => "std::iter::Extend",
        "Sum" => "std::iter::Sum",
        "Product" => "std::iter::Product",
        // std::str
        "FromStr" => "std::str::FromStr",
        // std::hash
        "Hasher" => "std::hash::Hasher",
        "BuildHasher" => "std::hash::BuildHasher",
        // Fall back to `std::{name}` for any unknown entry.
        other => return format!("std::{other}"),
    }
    .to_string()
}

/// Maps a well-known `core` trait short name to its canonical rustdoc path.
///
/// `core` re-exports most of the same traits as `std`, but under `core::*`
/// module paths.  When a catalogue entry declares `origin_crate = "core"`,
/// the rustdoc JSON emitted for C-side impls will use `core::convert::From`
/// rather than `std::convert::From`.  This function mirrors `std_canonical_path`
/// but produces `core::*` paths so the S-side codec generates identity keys
/// that match the C-side rustdoc output.
///
/// Falls back to `"core::{name}"` for any trait not in the lookup table.
pub(crate) fn core_canonical_path(short_name: &str) -> String {
    match short_name {
        // core::convert
        "From" => "core::convert::From",
        "Into" => "core::convert::Into",
        "TryFrom" => "core::convert::TryFrom",
        "TryInto" => "core::convert::TryInto",
        "AsRef" => "core::convert::AsRef",
        "AsMut" => "core::convert::AsMut",
        // core::clone
        "Clone" => "core::clone::Clone",
        // core::marker
        "Copy" => "core::marker::Copy",
        "Send" => "core::marker::Send",
        "Sync" => "core::marker::Sync",
        "Sized" => "core::marker::Sized",
        "Unpin" => "core::marker::Unpin",
        "PhantomData" => "core::marker::PhantomData",
        // core::fmt
        "Debug" => "core::fmt::Debug",
        "Display" => "core::fmt::Display",
        "Formatter" => "core::fmt::Formatter",
        // core::cmp
        "PartialEq" => "core::cmp::PartialEq",
        "Eq" => "core::cmp::Eq",
        "Ord" => "core::cmp::Ord",
        "PartialOrd" => "core::cmp::PartialOrd",
        // core::hash
        "Hash" => "core::hash::Hash",
        "Hasher" => "core::hash::Hasher",
        "BuildHasher" => "core::hash::BuildHasher",
        // core::default
        "Default" => "core::default::Default",
        // core::iter
        "Iterator" => "core::iter::Iterator",
        "IntoIterator" => "core::iter::IntoIterator",
        "DoubleEndedIterator" => "core::iter::DoubleEndedIterator",
        "ExactSizeIterator" => "core::iter::ExactSizeIterator",
        "FromIterator" => "core::iter::FromIterator",
        "Extend" => "core::iter::Extend",
        "Sum" => "core::iter::Sum",
        "Product" => "core::iter::Product",
        // core::ops
        "Drop" => "core::ops::Drop",
        "Deref" => "core::ops::Deref",
        "DerefMut" => "core::ops::DerefMut",
        "FnOnce" => "core::ops::FnOnce",
        "FnMut" => "core::ops::FnMut",
        "Fn" => "core::ops::Fn",
        "Add" => "core::ops::Add",
        "Sub" => "core::ops::Sub",
        "Mul" => "core::ops::Mul",
        "Div" => "core::ops::Div",
        "Rem" => "core::ops::Rem",
        "Neg" => "core::ops::Neg",
        "Not" => "core::ops::Not",
        "BitAnd" => "core::ops::BitAnd",
        "BitOr" => "core::ops::BitOr",
        "BitXor" => "core::ops::BitXor",
        "Shl" => "core::ops::Shl",
        "Shr" => "core::ops::Shr",
        "Index" => "core::ops::Index",
        "IndexMut" => "core::ops::IndexMut",
        "AddAssign" => "core::ops::AddAssign",
        "SubAssign" => "core::ops::SubAssign",
        "MulAssign" => "core::ops::MulAssign",
        "DivAssign" => "core::ops::DivAssign",
        "RemAssign" => "core::ops::RemAssign",
        "BitAndAssign" => "core::ops::BitAndAssign",
        "BitOrAssign" => "core::ops::BitOrAssign",
        "BitXorAssign" => "core::ops::BitXorAssign",
        "ShlAssign" => "core::ops::ShlAssign",
        "ShrAssign" => "core::ops::ShrAssign",
        // core::str — rustdoc emits the full re-export path including the private
        // `traits` submodule: `core::str::traits::FromStr`.
        "FromStr" => "core::str::traits::FromStr",
        // core::borrow
        "Borrow" => "core::borrow::Borrow",
        "BorrowMut" => "core::borrow::BorrowMut",
        // core::pin
        "Pin" => "core::pin::Pin",
        // core::error (stable since Rust 1.81)
        "Error" => "core::error::Error",
        // Fall back to `core::{name}` for any unknown entry.
        other => return format!("core::{other}"),
    }
    .to_string()
}

// ---------------------------------------------------------------------------
// Expression helpers
// ---------------------------------------------------------------------------

/// Converts a `syn::Expr` to a textual representation.
///
/// Best-effort: literal integers and paths are rendered verbatim; other forms
/// fall back to `"<const_expr>"`.
#[must_use]
pub(crate) fn syn_expr_to_string(expr: &syn::Expr) -> String {
    match expr {
        syn::Expr::Lit(lit_expr) => match &lit_expr.lit {
            syn::Lit::Int(i) => i.base10_digits().to_string(),
            syn::Lit::Str(s) => s.value(),
            syn::Lit::Bool(b) => b.value().to_string(),
            _ => "<const_expr>".to_string(),
        },
        syn::Expr::Path(path_expr) => path_expr
            .path
            .segments
            .iter()
            .map(|s| s.ident.to_string())
            .collect::<Vec<_>>()
            .join("::"),
        _ => "<const_expr>".to_string(),
    }
}

/// Converts a `syn::Expr` const array length to a string representation.
///
/// Preserves integer literals verbatim, named constants, and binary/unary
/// arithmetic expressions. Falls back to `"<const_len>"` only for forms that
/// cannot be represented as a simple token string.
#[must_use]
pub(crate) fn array_len_to_string(expr: &syn::Expr) -> String {
    expr_to_token_string(expr)
}

/// Renders a `syn::Expr` as a token string suitable for embedding in a type
/// description.
///
/// Handles literals, path constants, binary ops, unary ops, parenthesized
/// sub-expressions, and casts. Falls back to `"<const_expr>"` for forms that
/// are too complex to render without `quote!`.
pub(super) fn expr_to_token_string(expr: &syn::Expr) -> String {
    match expr {
        syn::Expr::Lit(lit_expr) => match &lit_expr.lit {
            syn::Lit::Int(i) => i.base10_digits().to_string(),
            syn::Lit::Str(s) => format!("{:?}", s.value()),
            syn::Lit::Bool(b) => b.value().to_string(),
            _ => "<const_expr>".to_string(),
        },
        syn::Expr::Path(path_expr) => path_expr
            .path
            .segments
            .iter()
            .map(|s| s.ident.to_string())
            .collect::<Vec<_>>()
            .join("::"),
        syn::Expr::Binary(bin_expr) => {
            let left = expr_to_token_string(&bin_expr.left);
            let right = expr_to_token_string(&bin_expr.right);
            let op = match &bin_expr.op {
                syn::BinOp::Add(_) => "+",
                syn::BinOp::Sub(_) => "-",
                syn::BinOp::Mul(_) => "*",
                syn::BinOp::Div(_) => "/",
                syn::BinOp::Rem(_) => "%",
                syn::BinOp::BitAnd(_) => "&",
                syn::BinOp::BitOr(_) => "|",
                syn::BinOp::BitXor(_) => "^",
                syn::BinOp::Shl(_) => "<<",
                syn::BinOp::Shr(_) => ">>",
                _ => "<op>",
            };
            format!("{left} {op} {right}")
        }
        syn::Expr::Unary(unary_expr) => {
            let inner = expr_to_token_string(&unary_expr.expr);
            let op = match &unary_expr.op {
                syn::UnOp::Neg(_) => "-",
                syn::UnOp::Not(_) => "!",
                _ => "<unary>",
            };
            format!("{op}{inner}")
        }
        syn::Expr::Paren(paren_expr) => {
            format!("({})", expr_to_token_string(&paren_expr.expr))
        }
        syn::Expr::Cast(cast_expr) => {
            // `N as usize` — preserve both the expression and the target type.
            let inner = expr_to_token_string(&cast_expr.expr);
            let target_ty = syn_type_to_string(&cast_expr.ty);
            format!("{inner} as {target_ty}")
        }
        _ => "<const_expr>".to_string(),
    }
}

/// Renders a `syn::Type` as a short string for use in cast expressions.
///
/// Only handles the common cases (primitives, simple paths); falls back to `_`
/// for complex forms.
pub(super) fn syn_type_to_string(ty: &syn::Type) -> String {
    match ty {
        syn::Type::Path(tp) if tp.qself.is_none() => {
            tp.path.segments.iter().map(|s| s.ident.to_string()).collect::<Vec<_>>().join("::")
        }
        syn::Type::Never(_) => "!".to_string(),
        _ => "_".to_string(),
    }
}

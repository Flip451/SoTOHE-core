//! Lexical validation for declared generic parameter names.
//!
//! Chain ③ compares catalogue and rustdoc representations lexically.  This
//! module intentionally does not classify Rust syntax or rewrite keyword
//! occurrences.  The compiler/rustdoc side supplies legal syntax; catalogue
//! declarations using raw identifiers or keywords are rejected at the codec
//! boundary instead of being guessed at here.

/// Returns whether `name` is a plain, non-keyword Rust identifier suitable for
/// a catalogue generic parameter declaration.
pub(crate) fn is_plain_generic_param_name(name: &str) -> bool {
    is_plain_identifier(name) && !is_rust_keyword(name)
}

/// Validate the declaration context supplied to the lexical adapter.
///
/// Parsing and semantic interpretation of the input expression remain delegated to the
/// existing `syn` boundary; this helper only validates the declared generic-name context.
pub(crate) fn validate(_input: &str, generic_params: &[&str]) -> Result<(), String> {
    for name in generic_params {
        if !is_plain_generic_param_name(name) {
            return Err(format!(
                "generic parameter name `{name}` must be a plain non-keyword Rust identifier"
            ));
        }
    }
    Ok(())
}

fn is_plain_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphabetic() || first == '_' => {
            chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        }
        _ => false,
    }
}

pub(super) fn is_rust_keyword(name: &str) -> bool {
    matches!(
        name,
        "as" | "async"
            | "await"
            | "break"
            | "const"
            | "continue"
            | "crate"
            | "dyn"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "fn"
            | "for"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "ref"
            | "return"
            | "self"
            | "Self"
            | "static"
            | "struct"
            | "super"
            | "trait"
            | "true"
            | "type"
            | "unsafe"
            | "union"
            | "use"
            | "where"
            | "while"
            | "abstract"
            | "become"
            | "box"
            | "do"
            | "final"
            | "gen"
            | "macro"
            | "macro_rules"
            | "override"
            | "priv"
            | "raw"
            | "safe"
            | "try"
            | "typeof"
            | "unsized"
            | "virtual"
            | "yield"
    ) || name == "_"
}

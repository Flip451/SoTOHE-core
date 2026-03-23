//! Domain code scanner using `syn` AST.
//!
//! Parses Rust source files to extract:
//! - Public type names (`pub struct Foo`, `pub enum Foo`, enum variants)
//! - State-transition functions (impl methods whose receiver type and return type
//!   are both known domain types)
//!
//! This module belongs to the infrastructure layer: pure parsing logic
//! (`scan_domain_code`) is I/O-free; `scan_domain_directory` performs file I/O.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use domain::CodeScanResult;
use syn::{
    FnArg, GenericArgument, ImplItem, Item, PathArguments, ReturnType, Type, TypePath, Visibility,
};

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Scans Rust source code and returns a `CodeScanResult`.
///
/// Detects:
/// - `pub struct Foo` and `pub enum Foo` names → added to `found_types`
/// - Enum variants of public enums → added to `found_types`
/// - `impl StateA { fn f(self) -> Result<StateB, _> }` → added to
///   `transition_map["StateA"]`
///
/// Self-transitions (`StateA → StateA`) are excluded.
/// Private types (no `pub`) are excluded.
///
/// Unparseable source (e.g. empty string, incomplete snippets) returns an
/// empty `CodeScanResult` rather than panicking.
#[must_use]
pub fn scan_domain_code(source_code: &str) -> CodeScanResult {
    let file = match syn::parse_str::<syn::File>(source_code) {
        Ok(f) => f,
        Err(_) => return CodeScanResult::new(HashSet::new(), HashMap::new()),
    };

    // --- Pass 1: collect all public type names ---
    let mut found_types: HashSet<String> = HashSet::new();
    for item in &file.items {
        collect_pub_types(item, &mut found_types);
    }

    // --- Pass 2: detect transition functions ---
    let mut transition_map: HashMap<String, HashSet<String>> = HashMap::new();
    for item in &file.items {
        collect_transitions(item, &found_types, &mut transition_map);
    }

    CodeScanResult::new(found_types, transition_map)
}

/// Recursively scans all `.rs` files in `dir` and merges the results.
///
/// Uses a two-pass strategy so that cross-file transitions are detected correctly:
/// - Pass 1: collect all public type names from every `.rs` file in the directory tree.
/// - Pass 2: scan each file's `impl` blocks using the full type set from pass 1.
///
/// This ensures that a layout like
/// ```text
/// // state_a.rs
/// pub struct Draft;
///
/// // transitions.rs
/// impl Draft { pub fn publish(self) -> Result<Published, Error> { … } }
/// ```
/// produces the edge `Draft → Published` even though `Draft` is defined in a
/// different file from its `impl` block.
///
/// # Errors
///
/// Returns `std::io::Error` if the directory cannot be read or a file cannot
/// be opened.
/// Error type for domain directory scanning.
#[derive(Debug, thiserror::Error)]
pub enum DomainScanError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{count} file(s) failed to parse: {files:?}")]
    ParseFailures { count: usize, files: Vec<String> },
}

pub fn scan_domain_directory(dir: &Path) -> Result<CodeScanResult, DomainScanError> {
    // Collect all source files first so we iterate twice without re-walking.
    let mut sources: Vec<(String, String)> = Vec::new(); // (file_path, content)
    collect_sources_recursive(dir, &mut sources)?;

    // Track parse failures so they can be reported.
    let mut parse_failures: Vec<String> = Vec::new();

    // Pass 1: aggregate all public type names across the entire directory tree.
    let mut found_types: HashSet<String> = HashSet::new();
    let mut parsed_files: Vec<syn::File> = Vec::new();
    for (path, source) in &sources {
        match syn::parse_str::<syn::File>(source) {
            Ok(f) => {
                for item in &f.items {
                    collect_pub_types(item, &mut found_types);
                }
                parsed_files.push(f);
            }
            Err(_) => {
                parse_failures.push(path.clone());
            }
        }
    }

    if !parse_failures.is_empty() {
        return Err(DomainScanError::ParseFailures {
            count: parse_failures.len(),
            files: parse_failures,
        });
    }

    // Pass 2: detect transitions using the complete type set.
    let mut transition_map: HashMap<String, HashSet<String>> = HashMap::new();
    for file in &parsed_files {
        for item in &file.items {
            collect_transitions(item, &found_types, &mut transition_map);
        }
    }

    Ok(CodeScanResult::new(found_types, transition_map))
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Recursively collects the text of all `.rs` files under `dir`.
fn collect_sources_recursive(
    dir: &Path,
    out: &mut Vec<(String, String)>,
) -> Result<(), std::io::Error> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_sources_recursive(&path, out)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            let source = std::fs::read_to_string(&path)?;
            out.push((path.display().to_string(), source));
        }
    }
    Ok(())
}

/// Collect public struct / enum names (and enum variants) into `found_types`.
fn collect_pub_types(item: &Item, found_types: &mut HashSet<String>) {
    match item {
        Item::Struct(s) => {
            if is_pub(&s.vis) {
                found_types.insert(s.ident.to_string());
            }
        }
        Item::Enum(e) => {
            if is_pub(&e.vis) {
                found_types.insert(e.ident.to_string());
                for variant in &e.variants {
                    found_types.insert(variant.ident.to_string());
                }
            }
        }
        Item::Mod(m) => {
            // Handle inline modules.
            if let Some((_, items)) = &m.content {
                for inner in items {
                    collect_pub_types(inner, found_types);
                }
            }
        }
        _ => {}
    }
}

/// Walk `impl` blocks and detect transition functions.
fn collect_transitions(
    item: &Item,
    found_types: &HashSet<String>,
    transition_map: &mut HashMap<String, HashSet<String>>,
) {
    match item {
        Item::Impl(impl_block) => {
            // Determine the Self type name.
            let self_type_name = match impl_type_name(&impl_block.self_ty) {
                Some(n) => n,
                None => return,
            };

            // Only process if the impl's Self type is a known domain type.
            if !found_types.contains(&self_type_name) {
                return;
            }

            for impl_item in &impl_block.items {
                if let ImplItem::Fn(method) = impl_item {
                    // Check whether there is a `self` / `&self` receiver.
                    if !has_self_receiver(method) {
                        continue;
                    }

                    // Extract return type target names.
                    let return_targets = extract_return_type_names(&method.sig.output, found_types);

                    for target in return_targets {
                        if target != self_type_name {
                            transition_map
                                .entry(self_type_name.clone())
                                .or_default()
                                .insert(target);
                        }
                    }
                }
            }
        }
        Item::Mod(m) => {
            if let Some((_, items)) = &m.content {
                for inner in items {
                    collect_transitions(inner, found_types, transition_map);
                }
            }
        }
        _ => {}
    }
}

/// Returns the simple type name for the `Self` type of an `impl` block.
/// Returns `None` for complex types (references, generics, etc.).
fn impl_type_name(ty: &Type) -> Option<String> {
    match ty {
        Type::Path(TypePath { qself: None, path }) => {
            // Take the last path segment without generics.
            path.segments.last().map(|seg| seg.ident.to_string())
        }
        _ => None,
    }
}

/// Returns `true` if a method has a `self` or `&self` receiver.
fn has_self_receiver(method: &syn::ImplItemFn) -> bool {
    method.sig.inputs.iter().any(|arg| matches!(arg, FnArg::Receiver(_)))
}

/// Extract all domain type names referenced in the return type.
///
/// Handles:
/// - `TypeName` directly
/// - `Result<TypeName, _>` → unwrap the first generic argument
/// - `Option<TypeName>` → unwrap the first generic argument
/// - Nested generics are unwrapped one level deep.
fn extract_return_type_names(
    return_type: &ReturnType,
    found_types: &HashSet<String>,
) -> Vec<String> {
    let ty = match return_type {
        ReturnType::Default => return vec![],
        ReturnType::Type(_, ty) => ty.as_ref(),
    };
    let mut results = Vec::new();
    collect_type_names(ty, found_types, &mut results);
    results
}

/// Recursively collect domain type names from a `syn::Type`, unwrapping
/// `Result<T, E>` (first argument) and `Option<T>`.
fn collect_type_names(ty: &Type, found_types: &HashSet<String>, out: &mut Vec<String>) {
    if let Type::Path(TypePath { qself: None, path }) = ty {
        let last = match path.segments.last() {
            Some(s) => s,
            None => return,
        };
        let name = last.ident.to_string();

        match name.as_str() {
            "Result" => {
                // Unwrap first generic argument (the Ok type).
                if let PathArguments::AngleBracketed(args) = &last.arguments {
                    if let Some(GenericArgument::Type(inner)) = args.args.first() {
                        collect_type_names(inner, found_types, out);
                    }
                }
            }
            "Option" => {
                // Unwrap first generic argument.
                if let PathArguments::AngleBracketed(args) = &last.arguments {
                    if let Some(GenericArgument::Type(inner)) = args.args.first() {
                        collect_type_names(inner, found_types, out);
                    }
                }
            }
            _ => {
                if found_types.contains(&name) {
                    out.push(name);
                }
            }
        }
    }
}

/// Returns `true` if the visibility is `pub`.
fn is_pub(vis: &Visibility) -> bool {
    matches!(vis, Visibility::Public(_))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use super::*;

    // --- T003-T001: pub struct detection ---

    #[test]
    fn test_scan_detects_pub_struct() {
        let src = "pub struct Draft;";
        let result = scan_domain_code(src);
        assert!(result.has_type("Draft"), "expected 'Draft' in found_types");
    }

    // --- T003-T002: pub enum + variant detection ---

    #[test]
    fn test_scan_detects_pub_enum_and_variants() {
        let src = "pub enum State { Draft, Published }";
        let result = scan_domain_code(src);
        assert!(result.has_type("State"), "expected 'State' in found_types");
        assert!(result.has_type("Draft"), "expected 'Draft' (variant) in found_types");
        assert!(result.has_type("Published"), "expected 'Published' (variant) in found_types");
    }

    // --- T003-T003: transition function detection ---

    #[test]
    fn test_scan_detects_transition_function() {
        let src = r#"
            pub struct Draft;
            pub struct Published;
            pub struct Error;
            impl Draft {
                pub fn publish(self) -> Result<Published, Error> { todo!() }
            }
        "#;
        let result = scan_domain_code(src);
        let targets = result.transitions_from("Draft").unwrap();
        assert!(targets.contains("Published"), "expected 'Published' in transitions from 'Draft'");
    }

    // --- T003-T004: Option unwrapping ---

    #[test]
    fn test_scan_unwraps_option() {
        let src = r#"
            pub struct Draft;
            pub struct Published;
            impl Draft {
                pub fn maybe(self) -> Option<Published> { None }
            }
        "#;
        let result = scan_domain_code(src);
        let targets = result.transitions_from("Draft").unwrap();
        assert!(targets.contains("Published"), "expected 'Published' via Option unwrap");
    }

    // --- T003-T005: self-transition excluded ---

    #[test]
    fn test_scan_skips_self_transition() {
        let src = r#"
            pub struct Draft;
            pub struct Error;
            impl Draft {
                pub fn update(self) -> Result<Draft, Error> { todo!() }
            }
        "#;
        let result = scan_domain_code(src);
        // There should be no entry at all, or the entry should not contain "Draft".
        match result.transitions_from("Draft") {
            None => {} // No map entry at all — correct
            Some(targets) => {
                assert!(
                    !targets.contains("Draft"),
                    "self-transition Draft->Draft must be excluded"
                );
            }
        }
    }

    // --- T003-T006: private types excluded ---

    #[test]
    fn test_scan_private_types_excluded() {
        let src = "struct Internal;";
        let result = scan_domain_code(src);
        assert!(!result.has_type("Internal"), "'Internal' (private) must not be in found_types");
    }

    // --- T003-T007: multiple transitions ---

    #[test]
    fn test_scan_multiple_transitions() {
        let src = r#"
            pub struct Draft;
            pub struct Published;
            pub struct Archived;
            pub struct Error;
            impl Draft {
                pub fn publish(self) -> Result<Published, Error> { todo!() }
                pub fn archive(self) -> Result<Archived, Error> { todo!() }
            }
        "#;
        let result = scan_domain_code(src);
        let targets = result.transitions_from("Draft").unwrap();
        assert!(targets.contains("Published"));
        assert!(targets.contains("Archived"));
    }

    // --- T003-T008: empty source ---

    #[test]
    fn test_scan_empty_source() {
        let result = scan_domain_code("");
        assert!(result.found_types().is_empty(), "empty source should yield empty found_types");
        assert!(
            result.transition_map().is_empty(),
            "empty source should yield empty transition_map"
        );
    }

    // --- Additional edge cases ---

    #[test]
    fn test_scan_ref_self_receiver_detects_transition() {
        // &self receiver should also work
        let src = r#"
            pub struct Draft;
            pub struct Summary;
            impl Draft {
                pub fn summarize(&self) -> Option<Summary> { None }
            }
        "#;
        let result = scan_domain_code(src);
        let targets = result.transitions_from("Draft").unwrap();
        assert!(targets.contains("Summary"));
    }

    #[test]
    fn test_scan_impl_of_unknown_type_ignored() {
        // impl block for a non-public type should not appear in transitions
        let src = r#"
            pub struct Published;
            struct Hidden;
            impl Hidden {
                pub fn go(self) -> Published { todo!() }
            }
        "#;
        let result = scan_domain_code(src);
        assert!(
            result.transitions_from("Hidden").is_none(),
            "impl of private type must be ignored"
        );
    }

    // --- scan_domain_directory: cross-file transition detection (two-pass) ---

    #[test]
    fn test_scan_directory_detects_cross_file_transition() {
        // Draft is defined in one file; impl Draft is in another file.
        // Two-pass scan must still detect the Draft → Published transition.
        let dir = tempfile::tempdir().unwrap();
        let types_file = dir.path().join("types.rs");
        let impl_file = dir.path().join("transitions.rs");

        std::fs::write(
            &types_file,
            "pub struct Draft;\npub struct Published;\npub struct Error;\n",
        )
        .unwrap();
        std::fs::write(
            &impl_file,
            "impl Draft { pub fn publish(self) -> Result<Published, Error> { todo!() } }\n",
        )
        .unwrap();

        let result = scan_domain_directory(dir.path()).unwrap();
        assert!(result.has_type("Draft"), "Draft must be found");
        assert!(result.has_type("Published"), "Published must be found");
        let targets = result.transitions_from("Draft").unwrap();
        assert!(targets.contains("Published"), "Draft -> Published must be detected across files");
    }

    #[test]
    fn test_scan_directory_single_file_still_works() {
        // Single-file layout must still produce correct results.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("state.rs"),
            r#"
pub struct Draft;
pub struct Published;
pub struct Error;
impl Draft {
    pub fn publish(self) -> Result<Published, Error> { todo!() }
}
"#,
        )
        .unwrap();

        let result = scan_domain_directory(dir.path()).unwrap();
        let targets = result.transitions_from("Draft").unwrap();
        assert!(targets.contains("Published"));
    }

    #[test]
    fn test_scan_directory_empty_dir_returns_empty_result() {
        let dir = tempfile::tempdir().unwrap();
        let result = scan_domain_directory(dir.path()).unwrap();
        assert!(result.found_types().is_empty());
        assert!(result.transition_map().is_empty());
    }

    #[test]
    fn test_scan_directory_parse_failure_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        // Valid file
        std::fs::write(dir.path().join("good.rs"), "pub struct Draft;").unwrap();
        // Malformed file (missing semicolon)
        std::fs::write(dir.path().join("bad.rs"), "pub struct Broken").unwrap();

        let err = scan_domain_directory(dir.path()).unwrap_err();
        match err {
            DomainScanError::ParseFailures { count, files } => {
                assert_eq!(count, 1);
                assert!(
                    files[0].contains("bad.rs"),
                    "error should name the failing file: {files:?}"
                );
            }
            other => panic!("expected ParseFailures, got: {other}"),
        }
    }
}

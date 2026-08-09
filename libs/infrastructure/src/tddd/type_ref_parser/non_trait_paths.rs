//! Definitive knowledge of standard-library non-trait paths.
//!
//! Supports the closed grammar's trait-position check (rustc E0404,
//! "expected trait, found struct"): only paths this module can PROVE to name
//! a standard-library struct / enum are rejected; everything else stays
//! open-world. Bare short names are never proven — the trait-path resolver
//! checks local catalogue items first, so `T: Vec<u8>` may legitimately name
//! a local trait `Vec`.

/// Standard-library types (structs / enums) whose std / core canonical paths
/// are definitively known.
const KNOWN_NON_TRAIT_STD_TYPES: &[&str] = &[
    "Vec",
    "Option",
    "Result",
    "String",
    "Box",
    "HashMap",
    "BTreeMap",
    "HashSet",
    "BTreeSet",
    "VecDeque",
    "LinkedList",
    "Arc",
    "Rc",
    "Mutex",
    "RwLock",
    "Pin",
    "PhantomData",
    "Formatter",
];

/// Returns the rendered path when it DEFINITIVELY names a known
/// standard-library non-trait type: the exact `std::` / `core::` canonical
/// path for one of the known structs / enums. Bare short names and unknown
/// paths return `None` (open-world — local items may shadow the prelude).
pub(super) fn known_non_trait_type(path: &syn::Path) -> Option<String> {
    let segments: Vec<String> =
        path.segments.iter().map(|segment| segment.ident.to_string()).collect();
    if segments.len() < 2 {
        return None;
    }
    let last = segments.last()?;
    if !KNOWN_NON_TRAIT_STD_TYPES.contains(&last.as_str()) {
        return None;
    }
    let rendered = segments.join("::");
    let canonical_std = super::helpers::std_canonical_path(last);
    let canonical_core = known_non_trait_core_path(last);
    (rendered == canonical_std || canonical_core == Some(rendered.as_str())).then_some(rendered)
}

/// The `core` spelling for known non-trait types.  This is intentionally
/// separate from `core_canonical_path`, whose fallback is intended for trait
/// resolution and therefore cannot supply module-qualified type paths.
fn known_non_trait_core_path(short_name: &str) -> Option<&'static str> {
    match short_name {
        "Vec" => Some("core::vec::Vec"),
        "Option" => Some("core::option::Option"),
        "Result" => Some("core::result::Result"),
        "String" => Some("core::string::String"),
        "Box" => Some("core::boxed::Box"),
        "HashMap" => Some("core::collections::HashMap"),
        "BTreeMap" => Some("core::collections::BTreeMap"),
        "HashSet" => Some("core::collections::HashSet"),
        "BTreeSet" => Some("core::collections::BTreeSet"),
        "VecDeque" => Some("core::collections::VecDeque"),
        "LinkedList" => Some("core::collections::LinkedList"),
        "Arc" => Some("core::sync::Arc"),
        "Rc" => Some("core::rc::Rc"),
        "Mutex" => Some("core::sync::Mutex"),
        "RwLock" => Some("core::sync::RwLock"),
        "Pin" => Some("core::pin::Pin"),
        "PhantomData" => Some("core::marker::PhantomData"),
        "Formatter" => Some("core::fmt::Formatter"),
        _ => None,
    }
}

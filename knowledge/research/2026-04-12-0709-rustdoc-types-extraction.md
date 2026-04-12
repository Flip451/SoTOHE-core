# rustdoc_types 0.57 Signature Extraction Research

> Research for TDDD-01 Multilayer Extension (Phase 1 Task 4)
> Source: Gemini CLI (researcher capability) via gemini-system skill
> Date: 2026-04-12

This research summary details how to extract structured method signatures using `rustdoc-types` 0.57 for the SoTOHE-core multilayer type catalogue.

## 1. Key `rustdoc_types` AST Fields

> **[CORRECTED — 2026-04-12]**: The original field names below (`Function.decl`, `ResolvedPath.name`,
> `BorrowedRef { mutable }`) do not match `rustdoc_types` 0.57 as used in this codebase.
> See `## Addendum` at the bottom of this file for the verified field names. The corrected
> names are annotated inline with `→ actual:` markers.

To extract signature data from an `Item` where `inner` is `ItemEnum::Function`, use these paths:

- **`is_async`**: `Function.header.is_async` (boolean). *(unchanged — this is correct)*
- **`params`**: `Function.decl.inputs` which is a `Vec<(String, Type)>`. → **actual: `Function.sig.inputs: Vec<(String, Type)>`** (`sig` is a `FunctionSignature` struct)
- **`returns`**: `Function.decl.output` which is an `Option<Type>`. `None` represents `()`. → **actual: `Function.sig.output: Option<Type>`**
- **`receiver`**: The first element of `Function.decl.inputs` if the string name is exactly `"self"`. → **actual: first element of `Function.sig.inputs`**
- **Type Names**: `Type::ResolvedPath.name` (full path) or `Type::Generic` (type parameter name). → **actual: `Type::ResolvedPath(p)` where `p.path` is the string and `p.args` holds generics**

---

## 2. Implementation Examples

> **[CORRECTED — 2026-04-12]**: Code examples below were updated to use verified
> `rustdoc_types` 0.57 field names. See `## Addendum` for the full correction table.

### Field 1 & 3: `is_async` and `receiver`

```rust
use rustdoc_types::{Function, Type};

fn extract_header_info(func: &Function) -> (bool, Option<String>) {
    let is_async = func.header.is_async;

    // Corrected: use func.sig.inputs (not func.decl.inputs)
    let receiver = func.sig.inputs.first().and_then(|(name, ty)| {
        if name == "self" {
            match ty {
                // Corrected: BorrowedRef uses is_mutable (not mutable), type_ (not type_prev)
                Type::BorrowedRef { is_mutable: false, .. } => Some("&self".to_string()),
                Type::BorrowedRef { is_mutable: true, .. } => Some("&mut self".to_string()),
                _ => Some("self".to_string()),
            }
        } else {
            None
        }
    });

    (is_async, receiver)
}
```

### Field 2 & 4: `params` and `returns` (Stringification)

The core logic requires a recursive helper to handle nested generics while stripping paths.

```rust
use rustdoc_types::{Type, GenericArgs, GenericArg, Path};

fn format_type(ty: &Type) -> String {
    match ty {
        // Corrected: ResolvedPath(p) where p is rustdoc_types::Path; use p.path and p.args
        Type::ResolvedPath(p) => {
            let short_name = p.path.split("::").last().unwrap_or(&p.path);
            if let Some(generic_args) = &p.args {
                format!("{}<{}>", short_name, format_args(generic_args))
            } else {
                short_name.to_string()
            }
        }
        Type::Generic(name) => name.clone(),
        Type::Primitive(name) => name.clone(),
        // Corrected: BorrowedRef uses is_mutable and type_ (not mutable / type_prev)
        Type::BorrowedRef { is_mutable, type_: inner, .. } => {
            format!("&{}{}", if *is_mutable { "mut " } else { "" }, format_type(inner))
        }
        Type::Tuple(tys) => format!("({})", tys.iter().map(format_type).collect::<Vec<_>>().join(", ")),
        _ => "unknown".to_string(), // Simplified for brevity
    }
}

fn format_args(args: &GenericArgs) -> String {
    match args {
        GenericArgs::AngleBracketed { args, .. } => args.iter()
            .filter_map(|arg| if let GenericArg::Type(t) = arg { Some(format_type(t)) } else { None })
            .collect::<Vec<_>>()
            .join(", "),
        _ => String::new(),
    }
}
```

---

## 3. Generic Structure & `ResolvedPath`

In `rustdoc_types` 0.57, `ResolvedPath` renders generics via the `args` field (an `Option<Box<GenericArgs>>`).

- **Preservation**: The `Vec<GenericArg>` inside `AngleBracketed` preserves the **exact order** defined in source code.
- **Recursion**: Because `GenericArg` can contain another `Type`, the recursive `format_type` approach naturally handles complex nesting like `Result<Option<User>, Error>`.
- **Shortening**: By only applying `split("::").last()` to the `name` field of `ResolvedPath`, we keep the generic brackets and commas intact while flattening the module hierarchy.

---

## 4. Edge Cases

- **`Self` Return**: Usually appears as a `ResolvedPath` with the name `"Self"`. If the trait/impl context is known, you can resolve it, but for L1, stringifying it as `"Self"` is idiomatic.
- **`impl Trait`**: Represented as `Type::ImplTrait(Vec<GenericBound>)`. This should be stringified as `impl TraitName`.
- **Unit Return**: `sig.output` (`Option<Type>`) will be `None`. Your logic must explicitly map this to `"()"`. *(corrected: `FnDecl.output` → `sig.output`)*
- **Associated Functions**: If `sig.inputs[0].0` is not `"self"`, `receiver` is `None`. This correctly identifies `static` methods. *(corrected: `inputs[0]` → `sig.inputs[0]`)*
- **Where Clauses**: These are stored in `Function.generics.where_predicates`. These should be **ignored** for L1 short-name mapping as they don't change the base type string in the signature.

---

## 5. References

- **Crate**: [`rustdoc-types` 0.57.0](https://docs.rs/rustdoc-types/0.57.0/rustdoc_types/)
- **Type Enum**: [rustdoc_types::Type](https://docs.rs/rustdoc-types/0.57.0/rustdoc_types/enum.Type.html)
- **Function Struct**: [rustdoc_types::Function](https://docs.rs/rustdoc-types/0.57.0/rustdoc_types/struct.Function.html)
- **FunctionSignature Struct**: `Function.sig` (replaces `FnDecl` — see Addendum for corrected field names)

## Addendum: Actual API Shape in This Codebase (Verified 2026-04-12)

The Gemini research above contains several field names that do not match the actual
`rustdoc_types` 0.57 struct definitions as used in `libs/infrastructure/src/schema_export.rs`.
Use the verified names below when implementing Task 4:

| Research document says | Actual field name in `rustdoc_types` 0.57 |
|---|---|
| `Function.decl.inputs` | `Function.sig` is a `FunctionSignature`; inputs are `sig.inputs: Vec<(String, Type)>` |
| `Function.decl.output` | `sig.output: Option<Type>` |
| `FnDecl` struct | `FunctionSignature` struct (accessed as the `sig` field on `Function`) |
| `Type::ResolvedPath.name` | `Type::ResolvedPath(p)` where `p` is `rustdoc_types::Path`; the path string is `p.path` |
| `Type::ResolvedPath.args` | `p.args: Option<Box<GenericArgs>>` |
| `BorrowedRef { mutable, .. }` | `BorrowedRef { is_mutable, type_: inner, .. }` |
| `BorrowedRef { type_prev, .. }` | `BorrowedRef { type_: inner, .. }` (field is named `type_`) |

The `format_type`, `format_args`, and `extract_header_info` examples in Section 2 must be
rewritten using the verified field names above before use in Task 4.
The Integration Notes in the final section reference `decl.inputs` / `decl.output` — replace
with `sig.inputs` / `sig.output`.

---

## Integration Notes for Task 4

When updating `libs/infrastructure/src/schema_export.rs::extract_methods`:

> **Note**: Steps 2–3 below originally used `decl.inputs` / `decl.output`. The correct field
> names for `rustdoc_types` 0.57 are `sig.inputs` / `sig.output` — see the `## Addendum`
> section at the bottom of this file and confirm against the existing extractor in
> `libs/infrastructure/src/schema_export.rs`. Use the corrected names below.

1. Add `format_type(&Type)` / `format_args(&GenericArgs)` as private helpers in the same module
2. Extract `params: Vec<(String, String)>` by mapping over `sig.inputs` (type: `Vec<(String, Type)>`) and skipping index 0 when the name is `"self"` (already accounted for by `receiver`)
3. Populate `FunctionInfo.returns` from `sig.output.as_ref().map_or("()".into(), format_type)` (type: `Option<Type>`)
4. Keep the existing `extract_return_type_names` behavior for `FunctionInfo.return_type_names` (used by `build_type_graph` to compute `outgoing` — see planner Q4 decision)
5. Map `Self` return type to the literal string `"Self"` — matches L1 catalog authoring convention where `impl Foo { fn clone(&self) -> Self }` is declared with `returns: "Self"`.

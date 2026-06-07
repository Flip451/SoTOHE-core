//! ABI formatting helper.

use rustdoc_types::Abi;

/// Formats a `rustdoc_types::Abi` as an `extern "…"` string prefix.
///
/// Returns an empty string for `Abi::Rust` (implicit ABI requires no prefix).
/// All other ABIs render as `extern "<name>" ` with a trailing space so the
/// caller can unconditionally prepend it to the `fn` keyword.
pub(crate) fn format_abi(abi: &Abi) -> String {
    match abi {
        Abi::Rust => String::new(),
        Abi::C { unwind: false } => "extern \"C\" ".to_string(),
        Abi::C { unwind: true } => "extern \"C-unwind\" ".to_string(),
        Abi::Cdecl { unwind: false } => "extern \"cdecl\" ".to_string(),
        Abi::Cdecl { unwind: true } => "extern \"cdecl-unwind\" ".to_string(),
        Abi::Stdcall { unwind: false } => "extern \"stdcall\" ".to_string(),
        Abi::Stdcall { unwind: true } => "extern \"stdcall-unwind\" ".to_string(),
        Abi::Fastcall { unwind: false } => "extern \"fastcall\" ".to_string(),
        Abi::Fastcall { unwind: true } => "extern \"fastcall-unwind\" ".to_string(),
        Abi::Aapcs { unwind: false } => "extern \"aapcs\" ".to_string(),
        Abi::Aapcs { unwind: true } => "extern \"aapcs-unwind\" ".to_string(),
        Abi::Win64 { unwind: false } => "extern \"win64\" ".to_string(),
        Abi::Win64 { unwind: true } => "extern \"win64-unwind\" ".to_string(),
        Abi::SysV64 { unwind: false } => "extern \"sysv64\" ".to_string(),
        Abi::SysV64 { unwind: true } => "extern \"sysv64-unwind\" ".to_string(),
        Abi::System { unwind: false } => "extern \"system\" ".to_string(),
        Abi::System { unwind: true } => "extern \"system-unwind\" ".to_string(),
        Abi::Other(name) => format!("extern \"{name}\" "),
    }
}

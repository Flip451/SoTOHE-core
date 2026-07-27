//! Typed YAML front-matter parsing for provider-native definitions.

use std::collections::BTreeMap;

use serde::de::Visitor;
use serde::{Deserialize, Deserializer};

/// The line that opens and closes a front-matter block.
const DELIMITER: &str = "---";

/// Every character `serde_yaml`'s parser treats as a line break.
///
/// This is the repository's single definition of that set. The reader below uses
/// it to open, scan, close, and trim delimiters, and
/// `conventions_resolve`'s empty-block test borrows it rather than keeping a
/// copy — two spellings of "what the parser treats as a break" drift apart, and
/// every gap between this set and the parser's own is a document read as
/// declaring nothing.
///
/// Established by feeding `serde_yaml` the input and asserting what it returns,
/// not by reading a specification and not by reading the C scanner underneath:
/// NEL, LS, and PS are breaks in YAML 1.1 and were dropped in 1.2, and this
/// parser accepts them. The set therefore describes *this* parser. If the
/// dependency is ever replaced, re-establish it the same way rather than
/// assuming it carried over.
pub(crate) const YAML_LINE_BREAKS: [char; 5] = ['\n', '\r', '\u{85}', '\u{2028}', '\u{2029}'];

/// Byte-order mark, which an editor may write at the head of a UTF-8 file.
///
/// The parser skips one, so a definition carrying it is ordinary rather than
/// malformed and its front matter has to be found all the same.
const BYTE_ORDER_MARK: char = '\u{FEFF}';

/// Locates the front-matter block of `content`, if it opens with one.
///
/// This is the repository's single notion of where a front-matter block starts
/// and ends, shared by the provider-definition parsers and the convention
/// codec, so what it accepts decides what every one of them can read.
///
/// Both delimiters are matched a whole line at a time, and a line ends at any
/// break in [`YAML_LINE_BREAKS`] — the same set used to open, scan, close, and
/// trim, so no document can be opened by a break the scan cannot then advance
/// over. Recognising a narrower set would make the line-ending style of the
/// consumer's checkout decide whether a document has front matter at all: the
/// opening match fails and the file is reported as carrying no block, which is
/// not a refusal anyone sees but a document read as declaring nothing. Every
/// step has to agree about the set, because accepting an opener the scan cannot
/// cross would turn that silence into an unclosed-block rejection of the same
/// ordinary file — a fail-open traded for a fail-closed one, both wrong.
///
/// A leading byte-order mark is dropped before any of that, since it would
/// otherwise push the opening delimiter off byte zero.
///
/// A closing delimiter that ends the file rather than a line is accepted for
/// the same reason: a definition whose last byte is the final `-` is an
/// ordinary file, and the block it closes is closed. The opening delimiter is
/// not read that way, because a file that is nothing but `---` opens nothing —
/// treating it as an opener would turn a document with no front matter into an
/// unclosed-block rejection.
///
/// The block is returned without the line break that precedes its closing
/// delimiter, so a caller parses the same YAML text whichever break was used.
/// Breaks *inside* the block are left exactly as the file spelled them: YAML
/// folds a CRLF break to LF itself before producing any value, so rewriting
/// them here would be this reader editing content it is only supposed to
/// delimit.
///
/// # Errors
///
/// Returns an unclosed-block message when `content` opens a block that no later
/// line closes.
pub(crate) fn read_front_matter(content: &str) -> Result<Option<&str>, String> {
    // Dropped before anything is matched, because the delimiter has to be the
    // first thing on the first line and a mark sits in front of it. The parser
    // skips one, so a file carrying it has front matter like any other; leaving
    // it in place would push `---` off byte zero and report the block as absent.
    let content = content.strip_prefix(BYTE_ORDER_MARK).unwrap_or(content);

    let Some(after_open) = strip_delimiter_line(content) else {
        return Ok(None);
    };

    let mut offset = 0;
    loop {
        let remaining = &after_open[offset..];
        // The closing delimiter may also be the last line of the file, which
        // the opening one cannot be: a file that is nothing but `---` opens no
        // block, while one whose final line is `---` closes the block it
        // opened.
        if remaining == DELIMITER || strip_delimiter_line(remaining).is_some() {
            // `offset` sits at the start of the closing delimiter's own line,
            // so everything before it is the block plus the break that ended
            // its last line.
            return Ok(Some(trim_one_line_break(&after_open[..offset])));
        }
        // Scanned with the same set the delimiters are matched with. Advancing
        // on `\n` alone would never reach the next line of a file broken by
        // lone carriage returns, so its closing delimiter would go unseen and
        // an ordinary document would be rejected as unclosed.
        let Some(line_break) = remaining.find(YAML_LINE_BREAKS) else {
            return Err("adapter definition has an unclosed YAML front matter block".to_owned());
        };
        offset += line_break + line_break_len(&remaining[line_break..]);
    }
}

/// Length in bytes of the line break `text` begins with.
///
/// A CRLF pair is one break rather than two, so it is measured whole; every
/// other break is a single character, and they are not all one byte wide.
fn line_break_len(text: &str) -> usize {
    if text.starts_with("\r\n") {
        return "\r\n".len();
    }
    text.chars().next().map_or(0, char::len_utf8)
}

/// Consumes a lone `---` line at the start of `content`, yielding what follows
/// it, or [`None`] when `content` does not begin with one.
///
/// The delimiter must be the whole line: a line merely starting with `---` is
/// content, and matching it would let a YAML document-start marker or a text
/// rule end a block the file did not end.
fn strip_delimiter_line(content: &str) -> Option<&str> {
    let rest = content.strip_prefix(DELIMITER)?;
    let first = rest.chars().next()?;
    // Measured whole, so a CRLF break is consumed as one rather than leaving
    // its line feed at the head of the next line.
    YAML_LINE_BREAKS.contains(&first).then(|| &rest[line_break_len(rest)..])
}

/// Removes one trailing line break from `block`, whichever way it is spelled.
///
/// Exactly one, and only at the end: it is the break that separated the block's
/// last line from the closing delimiter and so belongs to neither. A blank line
/// the block itself ended with is content and stays.
fn trim_one_line_break(block: &str) -> &str {
    if let Some(trimmed) = block.strip_suffix("\r\n") {
        return trimmed;
    }
    match block.chars().next_back() {
        Some(last) if YAML_LINE_BREAKS.contains(&last) => &block[..block.len() - last.len_utf8()],
        _ => block,
    }
}

/// Structurally validated YAML front matter from a provider-native definition.
///
/// Fields whose values influence provider dispatch are decoded only from the
/// top-level mapping. Additional definition fields must be scalar, which
/// prevents a nested mapping from hiding an identity, `sandbox`, `model`, or
/// `tools` declaration that a line-oriented parser might accidentally accept.
#[derive(Debug, Deserialize)]
pub(crate) struct ProviderDefinitionFrontMatter {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    sandbox: SandboxDeclaration,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    tools: Option<ToolsDeclaration>,
    #[serde(flatten)]
    _other_fields: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ToolsDeclaration {
    Scalar(String),
    List(Vec<String>),
}

#[derive(Debug, Default)]
enum SandboxDeclaration {
    #[default]
    Absent,
    Declared(String),
}

impl<'de> Deserialize<'de> for SandboxDeclaration {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct SandboxVisitor;

        impl Visitor<'_> for SandboxVisitor {
            type Value = SandboxDeclaration;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a non-null sandbox declaration string")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(SandboxDeclaration::Declared(value.to_owned()))
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(SandboxDeclaration::Declared(value))
            }

            fn visit_none<E>(self) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Err(E::custom("sandbox declaration must not be null"))
            }

            fn visit_unit<E>(self) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Err(E::custom("sandbox declaration must not be null"))
            }
        }

        deserializer.deserialize_any(SandboxVisitor)
    }
}

impl ProviderDefinitionFrontMatter {
    /// Ensures this definition is discoverable and registered for the expected capability.
    ///
    /// Both supported provider formats require `name` and `description` for discovery.
    pub(crate) fn validate_identity(
        &self,
        expected_capability: &str,
        definition_kind: &str,
    ) -> Result<(), String> {
        let name = self
            .name
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| format!("{definition_kind} must declare a non-empty name field"))?;
        if name != expected_capability {
            return Err(format!(
                "{definition_kind} name '{name}' does not match requested capability '{expected_capability}'"
            ));
        }
        if self.description.as_deref().is_none_or(|value| value.trim().is_empty()) {
            return Err(format!("{definition_kind} must declare a non-empty description field"));
        }
        Ok(())
    }

    pub(crate) fn sandbox(&self) -> Result<Option<&str>, String> {
        match &self.sandbox {
            SandboxDeclaration::Absent => Ok(None),
            SandboxDeclaration::Declared(value) if value.trim().is_empty() => {
                Err("Codex skill sandbox declaration must not be empty".to_owned())
            }
            SandboxDeclaration::Declared(value) => Ok(Some(value)),
        }
    }

    pub(crate) fn model(&self) -> Option<&str> {
        self.model.as_deref().filter(|value| !value.trim().is_empty())
    }

    /// Returns the explicitly declared Claude tool surface.
    pub(crate) fn tools(&self) -> Option<Vec<&str>> {
        match self.tools.as_ref() {
            Some(ToolsDeclaration::Scalar(value)) if !value.trim().is_empty() => {
                Some(vec![value.as_str()])
            }
            Some(ToolsDeclaration::List(values))
                if !values.is_empty() && values.iter().all(|value| !value.trim().is_empty()) =>
            {
                Some(values.iter().map(String::as_str).collect())
            }
            Some(ToolsDeclaration::Scalar(_)) | Some(ToolsDeclaration::List(_)) | None => None,
        }
    }
}

pub(crate) fn parse_provider_definition_front_matter(
    front_matter: &str,
) -> Result<ProviderDefinitionFrontMatter, String> {
    serde_yaml::from_str(front_matter)
        .map_err(|error| format!("invalid YAML front matter: {error}"))
}

//! Shared YAML frontmatter parsing for verify modules.
//!
//! Both `spec_attribution` and `spec_frontmatter` need to locate the frontmatter
//! region in a markdown file. This module provides a single implementation to
//! avoid duplicating `---` delimiter logic.

/// Result of parsing YAML frontmatter from a markdown file.
///
/// `body_start` is the line index (0-based) of the first line after the closing
/// `---` delimiter.  `frontmatter` is the text between the opening and closing
/// delimiters (excluding the delimiters themselves).
#[derive(Debug)]
pub struct Frontmatter {
    /// 0-based line index where the body (post-frontmatter) begins.
    pub body_start: usize,
    /// The raw text between the opening and closing `---` delimiters,
    /// with lines joined by `\n` (CRLF normalized).
    pub frontmatter: String,
}

/// Parses YAML frontmatter from markdown content.
///
/// The opening `---` must be the very first line (exact match, no leading/trailing
/// whitespace).  The closing `---` must also be an exact match at column 0 to
/// avoid matching `---` inside YAML block scalars.
///
/// Returns `None` when:
/// - The first line is not exactly `---`.
/// - No closing `---` is found (unclosed frontmatter).
pub fn parse_yaml_frontmatter(content: &str) -> Option<Frontmatter> {
    let lines: Vec<&str> = content.lines().collect();

    // Opening delimiter must be the very first line, exactly "---"
    if lines.first().is_none_or(|l| *l != "---") {
        return None;
    }

    // Search for closing `---` at column 0 (exact match, no whitespace).
    // Use byte-offset tracking via the original content to handle CRLF correctly.
    // `str::lines()` strips both \n and \r\n, so we walk the content with `find`
    // to locate the actual byte boundaries.
    for (i, line) in lines.iter().enumerate().skip(1) {
        if *line == "---" {
            // Collect frontmatter lines (between opening and closing ---)
            let fm_text = lines.get(1..i).map(|slice| slice.join("\n")).unwrap_or_default();
            return Some(Frontmatter { body_start: i + 1, frontmatter: fm_text });
        }
    }

    // No closing delimiter found
    None
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_frontmatter() {
        // line 0="---", 1="status: draft", 2="version: \"1.0\"", 3="---"
        // body starts at line 4
        let content = "---\nstatus: draft\nversion: \"1.0\"\n---\n# Content\n";
        let fm = parse_yaml_frontmatter(content).unwrap();
        assert_eq!(fm.body_start, 4);
        assert!(fm.frontmatter.contains("status: draft"));
        assert!(fm.frontmatter.contains("version: \"1.0\""));
    }

    #[test]
    fn test_no_opening_delimiter() {
        let content = "# No frontmatter\nSome content\n";
        assert!(parse_yaml_frontmatter(content).is_none());
    }

    #[test]
    fn test_no_closing_delimiter() {
        let content = "---\nstatus: draft\nversion: \"1.0\"\n";
        assert!(parse_yaml_frontmatter(content).is_none());
    }

    #[test]
    fn test_indented_opening_rejected() {
        let content = "  ---\nstatus: draft\n---\n";
        assert!(parse_yaml_frontmatter(content).is_none());
    }

    #[test]
    fn test_trailing_whitespace_opening_rejected() {
        let content = "---  \nstatus: draft\n---\n";
        assert!(parse_yaml_frontmatter(content).is_none());
    }

    #[test]
    fn test_trailing_whitespace_closing_rejected() {
        let content = "---\nstatus: draft\n---  \n# Content\n";
        assert!(parse_yaml_frontmatter(content).is_none());
    }

    #[test]
    fn test_indented_closing_rejected() {
        let content = "---\nstatus: draft\n  ---\n# Content\n";
        assert!(parse_yaml_frontmatter(content).is_none());
    }

    #[test]
    fn test_empty_frontmatter() {
        let content = "---\n---\n# Content\n";
        let fm = parse_yaml_frontmatter(content).unwrap();
        assert_eq!(fm.body_start, 2);
        assert!(fm.frontmatter.is_empty());
    }

    #[test]
    fn test_body_start_index() {
        // line 0="---", line 1="a: 1", line 2="b: 2", line 3="c: 3", line 4="---"
        // body starts at line 5
        let content = "---\na: 1\nb: 2\nc: 3\n---\nbody line\n";
        let fm = parse_yaml_frontmatter(content).unwrap();
        assert_eq!(fm.body_start, 5);
    }

    #[test]
    fn test_crlf_frontmatter() {
        let content = "---\r\nstatus: draft\r\nversion: \"1.0\"\r\n---\r\n# Content\r\n";
        let fm = parse_yaml_frontmatter(content).unwrap();
        assert_eq!(fm.body_start, 4);
        assert!(fm.frontmatter.contains("status: draft"));
        assert!(fm.frontmatter.contains("version: \"1.0\""));
    }
}

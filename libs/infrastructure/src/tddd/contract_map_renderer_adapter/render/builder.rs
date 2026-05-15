//! Mermaid output accumulator for the T006 renderer.
//!
//! `MermaidBuilder` collects subgraph/node lines, edge lines, and class attach
//! lines separately, then joins them into the final mermaid flowchart string via
//! [`MermaidBuilder::build`].

// ---------------------------------------------------------------------------
// Shape formatting helper
// ---------------------------------------------------------------------------

/// Formats a mermaid node line from a node id, display name, and shape string.
///
/// Shape values come from `[node.<Category>].shape` in the style config TOML.
/// Known mappings:
/// - `"round"` → `<id>(<name>)`
/// - `"stadium"` → `<id>([<name>])`
/// - `"subroutine"` → `<id>[[<name>]]`
///
/// Unknown shape values fall back to `"round"` so that invalid config values
/// produce valid (if visually unexpected) Mermaid instead of broken output.
fn format_node(id: &str, name: &str, shape: &str) -> String {
    match shape {
        "round" => format!("{id}({name})"),
        "stadium" => format!("{id}([{name}])"),
        "subroutine" => format!("{id}[[{name}]]"),
        _ => format!("{id}({name})"),
    }
}

// ---------------------------------------------------------------------------
// MermaidBuilder — line-by-line mermaid output accumulator
// ---------------------------------------------------------------------------

/// Accumulates mermaid flowchart lines during rendering.
///
/// Tracks indentation level and collects edge definitions separately so they
/// can be emitted after all subgraph declarations (per mermaid best practice).
pub(super) struct MermaidBuilder {
    /// Subgraph / node lines emitted so far (in order, with indentation).
    graph_lines: Vec<String>,
    /// Edge definition lines collected during rendering (emitted after subgraphs).
    edge_lines: Vec<String>,
    /// Class attach lines collected during rendering (emitted after edges, T008 finalizes).
    class_lines: Vec<String>,
    /// Current indentation level (2 spaces per level).
    indent: usize,
}

impl MermaidBuilder {
    pub(super) fn new() -> Self {
        Self { graph_lines: Vec::new(), edge_lines: Vec::new(), class_lines: Vec::new(), indent: 0 }
    }

    /// Appends an indented line to the graph section.
    pub(super) fn push(&mut self, line: impl Into<String>) {
        let prefix = "  ".repeat(self.indent);
        self.graph_lines.push(format!("{prefix}{}", line.into()));
    }

    /// Appends a line to the edge section (no indentation — edges are top-level).
    pub(super) fn push_edge(&mut self, line: impl Into<String>) {
        self.edge_lines.push(line.into());
    }

    /// Appends a `class <id> <className>` line to the class section.
    pub(super) fn push_class(&mut self, node_id: &str, class_name: &str) {
        self.class_lines.push(format!("class {node_id} {class_name}"));
    }

    /// Opens a `subgraph <id>["<label>"]` block, incrementing indent.
    pub(super) fn open_subgraph(&mut self, id: &str, label: &str) {
        self.push(format!("subgraph {id}[\"{label}\"]"));
        self.indent += 1;
    }

    /// Closes an `end` block, decrementing indent.
    pub(super) fn close_subgraph(&mut self) {
        if self.indent > 0 {
            self.indent -= 1;
        }
        self.push("end");
    }

    /// Emits a method node with the given mermaid shape inside the current subgraph.
    ///
    /// `shape` is the value from `[node.Method].shape` in the style config (e.g. `"round"`).
    /// Known shape mappings (from `format_node`):
    ///
    /// - `"round"` → `(<name>)`
    /// - `"stadium"` → `([<name>])`
    /// - `"subroutine"` → `[[<name>]]`
    ///
    /// Unknown shapes fall back to `(<name>)` (round) to avoid producing invalid Mermaid.
    pub(super) fn push_method_node(&mut self, method_id: &str, method_name: &str, shape: &str) {
        let line = format_node(method_id, method_name, shape);
        self.push(line);
    }

    /// Emits a standalone function node with the given mermaid shape.
    ///
    /// `shape` is the value from `[node.Function].shape` in the style config (e.g. `"subroutine"`).
    /// Known shape mappings (from `format_node`):
    ///
    /// - `"round"` → `(<name>)`
    /// - `"stadium"` → `([<name>])`
    /// - `"subroutine"` → `[[<name>]]`
    ///
    /// Unknown shapes fall back to `(<name>)` (round) to avoid producing invalid Mermaid.
    pub(super) fn push_function_node(&mut self, fn_id: &str, fn_name: &str, shape: &str) {
        let line = format_node(fn_id, fn_name, shape);
        self.push(line);
    }

    /// Builds the final mermaid flowchart string.
    ///
    /// Structure:
    /// ```text
    /// flowchart TD
    /// <classDef lines (minimal for T006)>
    ///
    /// <subgraph + node lines>
    ///
    /// <edge lines>
    /// <class attach lines>
    /// ```
    pub(super) fn build(self, classdefs: &[String]) -> String {
        let mut out = String::new();
        out.push_str("flowchart TD\n");
        for cd in classdefs {
            out.push_str(cd);
            out.push('\n');
        }
        if !classdefs.is_empty() {
            out.push('\n');
        }
        for line in &self.graph_lines {
            out.push_str(line);
            out.push('\n');
        }
        if !self.edge_lines.is_empty() {
            out.push('\n');
        }
        for line in &self.edge_lines {
            out.push_str(line);
            out.push('\n');
        }
        if !self.class_lines.is_empty() {
            out.push('\n');
        }
        for line in &self.class_lines {
            out.push_str(line);
            out.push('\n');
        }
        out
    }
}

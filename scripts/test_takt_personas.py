"""Tests for takt persona documents — verify metadata.json SSoT migration."""

from __future__ import annotations

import unittest
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parent.parent


class TestPersonaMetadataReferences(unittest.TestCase):
    """Verify personas reference metadata.json APIs, not plan.md checkboxes."""

    def test_rust_implementer_references_metadata_api(self) -> None:
        content = (
            PROJECT_ROOT / ".takt" / "personas" / "rust-implementer.md"
        ).read_text(encoding="utf-8")
        self.assertIn("metadata.json", content)
        self.assertIn("transition_task()", content)

    def test_rust_implementer_no_checkbox_mutation(self) -> None:
        content = (
            PROJECT_ROOT / ".takt" / "personas" / "rust-implementer.md"
        ).read_text(encoding="utf-8")
        # Should not instruct direct plan.md checkbox editing
        self.assertNotIn("`[ ]` → `[~]`", content)
        self.assertNotIn("`[~]` → `[x]`", content)

    def test_note_writer_references_metadata_json(self) -> None:
        content = (PROJECT_ROOT / ".takt" / "personas" / "note-writer.md").read_text(
            encoding="utf-8"
        )
        self.assertIn("metadata.json", content)

    def test_note_writer_no_plan_checkbox_parsing(self) -> None:
        content = (PROJECT_ROOT / ".takt" / "personas" / "note-writer.md").read_text(
            encoding="utf-8"
        )
        # Should not instruct parsing [x] from plan.md
        self.assertNotIn("`[x]` items in `plan.md`", content)
        self.assertNotIn("[x] item in plan.md", content)
        self.assertNotIn("from the completed plan.md item", content)


if __name__ == "__main__":
    unittest.main()

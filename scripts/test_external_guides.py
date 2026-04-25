import io
import json
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
from unittest import mock

import scripts.external_guides as external_guides


class ExternalGuidesTest(unittest.TestCase):
    def setUp(self) -> None:
        self.registry = {
            "usage_policy": {
                "goal": "Goal text",
                "read_order": ["step 1", "step 2"],
                "copyright_notes": ["note 1"],
            },
            "guides": [
                {
                    "id": "guide-a",
                    "title": "Guide A",
                    "source_url": "https://example.com/source",
                    "raw_url": "https://example.com/raw",
                    "license": "CC-BY-4.0",
                    "cache_path": ".cache/external-guides/guide-a.md",
                    "trigger_keywords": ["alpha", "beta"],
                    "summary": ["summary"],
                    "project_usage": ["usage"],
                }
            ],
        }

    def test_list_guides(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            with mock.patch.object(external_guides, "project_root", return_value=root):
                stdout = io.StringIO()
                with redirect_stdout(stdout):
                    code = external_guides.list_guides(self.registry)

        output = stdout.getvalue()
        self.assertEqual(code, 0)
        self.assertIn("External Guide Registry", output)
        self.assertIn("- guide-a", output)
        self.assertIn("status: missing", output)

    def test_show_usage(self) -> None:
        stdout = io.StringIO()
        with redirect_stdout(stdout):
            code = external_guides.show_usage(self.registry)

        output = stdout.getvalue()
        self.assertEqual(code, 0)
        self.assertIn("Usage Policy", output)
        self.assertIn("Goal text", output)
        self.assertIn("- step 1", output)

    def test_show_setup(self) -> None:
        stdout = io.StringIO()
        with redirect_stdout(stdout):
            code = external_guides.show_setup(self.registry)

        output = stdout.getvalue()
        self.assertEqual(code, 0)
        self.assertIn("External Guide Setup", output)
        self.assertIn("cargo make guides-fetch <guide-id>", output)
        self.assertIn("- guide-a", output)

    def test_fetch_unknown_guide_id(self) -> None:
        stderr = io.StringIO()
        with redirect_stderr(stderr):
            code = external_guides.fetch_guides(self.registry, ["missing-guide"])

        self.assertEqual(code, 1)
        self.assertIn("Unknown guide ids: missing-guide", stderr.getvalue())

    def test_find_relevant_guides_matches_ascii_word_boundaries(self) -> None:
        matches = external_guides.find_relevant_guides(
            "Please review the alpha migration plan before implementation",
            self.registry,
        )

        self.assertEqual(len(matches), 1)
        guide, trigger = matches[0]
        self.assertEqual(guide["id"], "guide-a")
        self.assertEqual(trigger, "alpha")

    def test_find_relevant_guides_ignores_ascii_word_fragments(self) -> None:
        matches = external_guides.find_relevant_guides(
            "alphabet soup should not trigger the guide",
            self.registry,
        )

        self.assertEqual(matches, [])

    def test_find_relevant_guides_for_track_workflow_reads_latest_track_context(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            track_dir = root / "track" / "items" / "demo"
            track_dir.mkdir(parents=True, exist_ok=True)
            (track_dir / "metadata.json").write_text(
                json.dumps(
                    {
                        "schema_version": 3,
                        "id": "demo",
                        "title": "Demo",
                        "status": "planned",
                        "branch": None,
                        "created_at": "2026-03-08T00:00:00Z",
                        "updated_at": "2026-03-08T00:00:00Z",
                        "tasks": [],
                        "plan": {"summary": [], "sections": []},
                    }
                ),
                encoding="utf-8",
            )
            (track_dir / "spec.md").write_text(
                "Schema work touches alpha tables.\n", encoding="utf-8"
            )
            (track_dir / "plan.md").write_text(
                "Review migration ordering.\n", encoding="utf-8"
            )
            # observations.md is optional (post-2026-04-25 ADR); the test does not create it.

            matches = external_guides.find_relevant_guides_for_track_workflow(
                "/track:implement current task",
                self.registry,
                track_context=external_guides.latest_track_context(root),
            )

        self.assertEqual(len(matches), 1)
        guide, trigger = matches[0]
        self.assertEqual(guide["id"], "guide-a")
        self.assertEqual(trigger, "alpha")

    def test_latest_track_context_returns_empty_when_tracks_are_missing(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)

            self.assertEqual(external_guides.latest_track_context(root), "")

    def test_latest_track_dir_prefers_larger_updated_at(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            older = root / "track" / "items" / "old"
            newer = root / "track" / "items" / "new"
            older.mkdir(parents=True, exist_ok=True)
            newer.mkdir(parents=True, exist_ok=True)

            (older / "metadata.json").write_text(
                json.dumps({"updated_at": "2024-01-01"}), encoding="utf-8"
            )
            (newer / "metadata.json").write_text(
                json.dumps({"updated_at": "2025-06-15"}), encoding="utf-8"
            )

            self.assertEqual(external_guides.latest_track_dir(root), newer)

    def test_latest_track_dir_prefers_materialized_active_over_newer_plan_only(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            materialized = root / "track" / "items" / "materialized"
            plan_only = root / "track" / "items" / "plan-only"
            materialized.mkdir(parents=True, exist_ok=True)
            plan_only.mkdir(parents=True, exist_ok=True)

            (materialized / "metadata.json").write_text(
                json.dumps(
                    {
                        "schema_version": 3,
                        "status": "in_progress",
                        "branch": "track/materialized",
                        "updated_at": "2025-06-01",
                    }
                ),
                encoding="utf-8",
            )
            (plan_only / "metadata.json").write_text(
                json.dumps(
                    {
                        "schema_version": 3,
                        "status": "planned",
                        "branch": None,
                        "updated_at": "2025-06-15",
                    }
                ),
                encoding="utf-8",
            )

            self.assertEqual(external_guides.latest_track_dir(root), materialized)

    def test_latest_track_dir_falls_back_to_epoch_when_metadata_missing(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            no_meta = root / "track" / "items" / "zzz-no-meta"
            with_meta = root / "track" / "items" / "aaa-with-meta"
            no_meta.mkdir(parents=True, exist_ok=True)
            with_meta.mkdir(parents=True, exist_ok=True)

            (with_meta / "metadata.json").write_text(
                json.dumps({"updated_at": "2025-01-01"}), encoding="utf-8"
            )
            # no_meta has no metadata.json → treated as epoch, ranked last

            self.assertEqual(external_guides.latest_track_dir(root), with_meta)

    def test_latest_track_dir_falls_back_to_epoch_when_metadata_invalid_json(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            bad_json = root / "track" / "items" / "zzz-bad"
            with_meta = root / "track" / "items" / "aaa-good"
            bad_json.mkdir(parents=True, exist_ok=True)
            with_meta.mkdir(parents=True, exist_ok=True)

            (bad_json / "metadata.json").write_text("not-json", encoding="utf-8")
            (with_meta / "metadata.json").write_text(
                json.dumps({"updated_at": "2025-03-01"}), encoding="utf-8"
            )

            self.assertEqual(external_guides.latest_track_dir(root), with_meta)

    def test_latest_track_dir_falls_back_to_epoch_when_metadata_not_a_dict(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            bad_type = root / "track" / "items" / "zzz-bad-type"
            with_meta = root / "track" / "items" / "aaa-good"
            bad_type.mkdir(parents=True, exist_ok=True)
            with_meta.mkdir(parents=True, exist_ok=True)

            (bad_type / "metadata.json").write_text("[]", encoding="utf-8")
            (with_meta / "metadata.json").write_text(
                json.dumps({"updated_at": "2025-05-01"}), encoding="utf-8"
            )

            self.assertEqual(external_guides.latest_track_dir(root), with_meta)

    def test_latest_track_dir_falls_back_to_epoch_on_extreme_offset_overflow(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            overflow = root / "track" / "items" / "zzz-overflow"
            with_meta = root / "track" / "items" / "aaa-good"
            overflow.mkdir(parents=True, exist_ok=True)
            with_meta.mkdir(parents=True, exist_ok=True)

            # UTC conversion of year-1 with +14:00 offset overflows datetime range
            (overflow / "metadata.json").write_text(
                json.dumps({"updated_at": "0001-01-01T00:00:00+14:00"}),
                encoding="utf-8",
            )
            (with_meta / "metadata.json").write_text(
                json.dumps({"updated_at": "2025-04-01"}), encoding="utf-8"
            )

            self.assertEqual(external_guides.latest_track_dir(root), with_meta)

    def test_latest_track_dir_returns_none_when_track_items_is_a_file(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            items_path = root / "track" / "items"
            items_path.parent.mkdir(parents=True, exist_ok=True)
            items_path.write_text("not-a-dir\n", encoding="utf-8")

            self.assertIsNone(external_guides.latest_track_dir(root))

    def test_latest_track_dir_returns_none_when_only_v3_tracks_are_invalid(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            broken = root / "track" / "items" / "broken-v3"
            broken.mkdir(parents=True, exist_ok=True)
            (broken / "metadata.json").write_text(
                json.dumps(
                    {
                        "schema_version": 3,
                        "id": "broken-v3",
                        "title": "Broken",
                        "status": "planned",
                        "created_at": "2026-03-08T00:00:00Z",
                        "updated_at": "2026-03-08T00:00:00Z",
                        "tasks": [],
                        "plan": {"summary": [], "sections": []},
                    }
                ),
                encoding="utf-8",
            )

            self.assertIsNone(external_guides.latest_track_dir(root))

    def test_latest_track_context_reads_available_spec_without_plan(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            track_dir = root / "track" / "items" / "demo"
            track_dir.mkdir(parents=True, exist_ok=True)
            (track_dir / "metadata.json").write_text(
                json.dumps(
                    {
                        "schema_version": 3,
                        "id": "demo",
                        "title": "Demo",
                        "status": "planned",
                        "branch": None,
                        "created_at": "2026-03-08T00:00:00Z",
                        "updated_at": "2026-03-08T00:00:00Z",
                        "tasks": [],
                        "plan": {"summary": [], "sections": []},
                    }
                ),
                encoding="utf-8",
            )
            (track_dir / "spec.md").write_text("spec only\n", encoding="utf-8")

            self.assertEqual(external_guides.latest_track_context(root), "spec only\n")

    def test_find_relevant_guides_for_track_workflow_uses_prompt_only_when_context_empty(
        self,
    ) -> None:
        matches = external_guides.find_relevant_guides_for_track_workflow(
            "/track:plan beta rollout",
            self.registry,
            track_context="",
        )

        self.assertEqual(len(matches), 1)
        guide, trigger = matches[0]
        self.assertEqual(guide["id"], "guide-a")
        self.assertEqual(trigger, "beta")

    def test_main_dispatches_help(self) -> None:
        stdout = io.StringIO()
        with mock.patch.object(
            external_guides, "load_registry", return_value=self.registry
        ):
            with redirect_stdout(stdout):
                code = external_guides.main(["external_guides.py", "help"])

        self.assertEqual(code, 0)
        self.assertIn("cargo make guides-list", stdout.getvalue())
        self.assertIn("cargo make guides-add --", stdout.getvalue())

    def test_main_help_does_not_require_registry(self) -> None:
        stdout = io.StringIO()
        with mock.patch.object(
            external_guides,
            "load_registry",
            side_effect=AssertionError("should not load"),
        ):
            with redirect_stdout(stdout):
                code = external_guides.main(["external_guides.py", "--help"])

        self.assertEqual(code, 0)
        self.assertIn("cargo make guides-setup", stdout.getvalue())

    def test_add_guide_derives_raw_url_and_cache_path(self) -> None:
        registry = json.loads(json.dumps(self.registry))
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            ext_dir = root / "knowledge" / "external"
            ext_dir.mkdir(parents=True, exist_ok=True)
            registry_file = ext_dir / "guides.json"
            registry_file.write_text(
                json.dumps(registry, ensure_ascii=False, indent=2), encoding="utf-8"
            )

            with mock.patch.object(external_guides, "project_root", return_value=root):
                stdout = io.StringIO()
                with redirect_stdout(stdout):
                    code = external_guides.main(
                        [
                            "external_guides.py",
                            "add",
                            "--id",
                            "new-guide",
                            "--title",
                            "New Guide",
                            "--source-url",
                            "https://github.com/example/repo/blob/main/docs/guide.md",
                            "--license",
                            "MIT",
                            "--trigger",
                            "sql",
                            "--summary",
                            "Use for SQL design",
                            "--project-usage",
                            "Check before schema changes",
                        ]
                    )

            updated = json.loads(registry_file.read_text(encoding="utf-8"))

        self.assertEqual(code, 0)
        self.assertIn("[OK] Added guide entry: new-guide", stdout.getvalue())
        added = next(guide for guide in updated["guides"] if guide["id"] == "new-guide")
        self.assertEqual(
            added["raw_url"],
            "https://raw.githubusercontent.com/example/repo/main/docs/guide.md",
        )
        self.assertEqual(added["cache_path"], ".cache/external-guides/new-guide.md")
        self.assertEqual(added["trigger_keywords"], ["sql"])

    def test_add_guide_accepts_cargo_make_separator(self) -> None:
        registry = json.loads(json.dumps(self.registry))
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            ext_dir = root / "knowledge" / "external"
            ext_dir.mkdir(parents=True, exist_ok=True)
            registry_file = ext_dir / "guides.json"
            registry_file.write_text(
                json.dumps(registry, ensure_ascii=False, indent=2), encoding="utf-8"
            )

            with mock.patch.object(external_guides, "project_root", return_value=root):
                code = external_guides.main(
                    [
                        "external_guides.py",
                        "add",
                        "--",
                        "--id",
                        "separator-guide",
                        "--title",
                        "Separator Guide",
                        "--source-url",
                        "https://example.com/guide.md",
                        "--license",
                        "MIT",
                    ]
                )

            updated = json.loads(registry_file.read_text(encoding="utf-8"))

        self.assertEqual(code, 0)
        self.assertTrue(
            any(guide["id"] == "separator-guide" for guide in updated["guides"])
        )

    def test_add_guide_rejects_duplicate_id(self) -> None:
        registry = json.loads(json.dumps(self.registry))
        stderr = io.StringIO()
        with redirect_stderr(stderr):
            code = external_guides.add_guide(
                registry,
                [
                    "external_guides.py",
                    "add",
                    "--id",
                    "guide-a",
                    "--title",
                    "Guide A",
                    "--source-url",
                    "https://example.com/guide-a.md",
                    "--license",
                    "CC-BY-4.0",
                ],
            )

        self.assertEqual(code, 1)
        self.assertIn("Guide id already exists: guide-a", stderr.getvalue())

    def test_add_guide_rejects_non_https_urls(self) -> None:
        registry = json.loads(json.dumps(self.registry))
        stderr = io.StringIO()
        with redirect_stderr(stderr):
            code = external_guides.add_guide(
                registry,
                [
                    "external_guides.py",
                    "add",
                    "--id",
                    "local-guide",
                    "--title",
                    "Local Guide",
                    "--source-url",
                    "file:///tmp/local.md",
                    "--license",
                    "MIT",
                ],
            )

        self.assertEqual(code, 1)
        self.assertIn("Only https URLs are allowed", stderr.getvalue())

    def test_add_guide_rejects_absolute_cache_path(self) -> None:
        registry = json.loads(json.dumps(self.registry))
        stderr = io.StringIO()
        with redirect_stderr(stderr):
            code = external_guides.add_guide(
                registry,
                [
                    "external_guides.py",
                    "add",
                    "--id",
                    "bad-cache",
                    "--title",
                    "Bad Cache",
                    "--source-url",
                    "https://example.com/guide.md",
                    "--license",
                    "MIT",
                    "--cache-path",
                    "/tmp/bad-cache.md",
                ],
            )

        self.assertEqual(code, 1)
        self.assertIn("Invalid cache_path", stderr.getvalue())

    def test_fetch_guides_rejects_cache_path_outside_cache_directory(self) -> None:
        registry = json.loads(json.dumps(self.registry))
        registry["guides"][0]["cache_path"] = "../guide-a.md"
        stderr = io.StringIO()
        with mock.patch("urllib.request.urlopen") as urlopen:
            with redirect_stderr(stderr):
                code = external_guides.fetch_guides(registry, ["guide-a"])

        self.assertEqual(code, 1)
        self.assertIn("Invalid cache_path for guide-a", stderr.getvalue())
        urlopen.assert_not_called()

    def test_fetch_guides_rejects_non_https_raw_url(self) -> None:
        registry = json.loads(json.dumps(self.registry))
        registry["guides"][0]["raw_url"] = "file:///etc/passwd"
        stderr = io.StringIO()
        with mock.patch("urllib.request.urlopen") as urlopen:
            with redirect_stderr(stderr):
                code = external_guides.fetch_guides(registry, ["guide-a"])

        self.assertEqual(code, 1)
        self.assertIn("Unsupported URL scheme for guide-a", stderr.getvalue())
        urlopen.assert_not_called()

    def test_fetch_guides_rejects_downloads_over_size_limit(self) -> None:
        registry = json.loads(json.dumps(self.registry))

        class FakeResponse:
            def __enter__(self):
                return self

            def __exit__(self, exc_type, exc, tb):
                return False

            def read(self, _size=None):
                return b"x" * (external_guides.MAX_DOWNLOAD_BYTES + 1)

        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            stderr = io.StringIO()
            with mock.patch.object(external_guides, "project_root", return_value=root):
                with mock.patch("urllib.request.urlopen", return_value=FakeResponse()):
                    with redirect_stderr(stderr):
                        code = external_guides.fetch_guides(registry, ["guide-a"])

        self.assertEqual(code, 1)
        self.assertIn("response exceeded download limit", stderr.getvalue())


class DeriveRawUrlTest(unittest.TestCase):
    """Unit tests for derive_raw_url() query/fragment stripping."""

    def test_plain_blob_url_converts_to_raw(self) -> None:
        url = "https://github.com/example/repo/blob/main/docs/guide.md"
        self.assertEqual(
            external_guides.derive_raw_url(url),
            "https://raw.githubusercontent.com/example/repo/main/docs/guide.md",
        )

    def test_blob_url_with_query_strips_query_before_converting(self) -> None:
        url = "https://github.com/example/repo/blob/main/docs/guide.md?plain=1"
        self.assertEqual(
            external_guides.derive_raw_url(url),
            "https://raw.githubusercontent.com/example/repo/main/docs/guide.md",
        )

    def test_blob_url_with_fragment_strips_fragment_before_converting(self) -> None:
        url = "https://github.com/example/repo/blob/main/docs/guide.md#L10-L20"
        self.assertEqual(
            external_guides.derive_raw_url(url),
            "https://raw.githubusercontent.com/example/repo/main/docs/guide.md",
        )

    def test_blob_url_with_query_and_fragment_strips_both(self) -> None:
        url = "https://github.com/example/repo/blob/main/docs/guide.md?plain=1#L10-L20"
        self.assertEqual(
            external_guides.derive_raw_url(url),
            "https://raw.githubusercontent.com/example/repo/main/docs/guide.md",
        )

    def test_non_blob_url_is_returned_unchanged_without_query(self) -> None:
        url = "https://raw.githubusercontent.com/example/repo/main/docs/guide.md"
        self.assertEqual(external_guides.derive_raw_url(url), url)

    def test_non_blob_url_with_query_is_returned_unchanged(self) -> None:
        # Non-GitHub-blob URLs (e.g. signed or versioned raw URLs) must keep their query params.
        url = "https://example.com/docs/guide.md?version=2&token=abc"
        self.assertEqual(external_guides.derive_raw_url(url), url)

    def test_non_blob_url_with_query_and_fragment_is_returned_unchanged(self) -> None:
        url = "https://example.com/docs/guide.md?v=1#section"
        self.assertEqual(external_guides.derive_raw_url(url), url)

    def test_url_with_github_blob_only_in_query_does_not_crash(self) -> None:
        # marker/blob are present in URL but only inside a query param — must not crash.
        url = "https://example.com/path?redirect=https://github.com/example/repo/blob/main/docs/guide.md"
        self.assertEqual(external_guides.derive_raw_url(url), url)


class FetchWithRetryTest(unittest.TestCase):
    def test_succeeds_after_transient_failure(self) -> None:
        responses = [
            urllib.error.HTTPError(
                "https://example.com", 503, "Service Unavailable", {}, None
            ),
            mock.MagicMock(
                read=mock.MagicMock(return_value=b"ok"),
                __enter__=mock.MagicMock(),
                __exit__=mock.MagicMock(),
            ),
        ]
        responses[1].__enter__.return_value = responses[1]

        with mock.patch(
            "scripts.external_guides.urllib.request.urlopen", side_effect=responses
        ):
            with mock.patch("scripts.external_guides.time_mod.sleep"):
                body = external_guides.fetch_with_retry(
                    "https://example.com/guide.md", max_retries=3, backoff_base=0.01
                )
        self.assertEqual(body, b"ok")

    def test_raises_after_max_retries(self) -> None:
        err = urllib.error.HTTPError(
            "https://example.com", 503, "Service Unavailable", {}, None
        )

        with mock.patch(
            "scripts.external_guides.urllib.request.urlopen",
            side_effect=[err, err, err],
        ):
            with mock.patch("scripts.external_guides.time_mod.sleep"):
                with self.assertRaises(urllib.error.HTTPError):
                    external_guides.fetch_with_retry(
                        "https://example.com/guide.md", max_retries=3, backoff_base=0.01
                    )

    def test_does_not_retry_on_4xx(self) -> None:
        err = urllib.error.HTTPError("https://example.com", 404, "Not Found", {}, None)

        with mock.patch(
            "scripts.external_guides.urllib.request.urlopen", side_effect=err
        ) as mock_open:
            with mock.patch("scripts.external_guides.time_mod.sleep"):
                with self.assertRaises(urllib.error.HTTPError):
                    external_guides.fetch_with_retry(
                        "https://example.com/guide.md", max_retries=3, backoff_base=0.01
                    )
        mock_open.assert_called_once()

    def test_raises_value_error_when_max_retries_zero(self) -> None:
        with self.assertRaises(ValueError):
            external_guides.fetch_with_retry(
                "https://example.com/guide.md", max_retries=0
            )

    def test_raises_value_error_when_max_retries_negative(self) -> None:
        with self.assertRaises(ValueError):
            external_guides.fetch_with_retry(
                "https://example.com/guide.md", max_retries=-1
            )

    def test_sends_user_agent_header(self) -> None:
        resp = mock.MagicMock(
            read=mock.MagicMock(return_value=b"ok"),
            __enter__=mock.MagicMock(),
            __exit__=mock.MagicMock(),
        )
        resp.__enter__.return_value = resp

        with mock.patch(
            "scripts.external_guides.urllib.request.urlopen", return_value=resp
        ) as mock_open:
            external_guides.fetch_with_retry("https://example.com/guide.md")
        request_arg = mock_open.call_args[0][0]
        self.assertEqual(
            request_arg.get_header("User-agent"), external_guides.USER_AGENT
        )


class CleanGuidesTest(unittest.TestCase):
    def setUp(self) -> None:
        self.registry = {
            "usage_policy": {"goal": "", "read_order": [], "copyright_notes": []},
            "guides": [
                {
                    "id": "guide-a",
                    "title": "Guide A",
                    "source_url": "https://example.com/a.md",
                    "raw_url": "https://raw.example.com/a.md",
                    "license": "MIT",
                    "cache_path": ".cache/external-guides/guide-a.md",
                    "trigger_keywords": [],
                    "summary": [],
                    "project_usage": [],
                },
            ],
        }

    def test_clean_removes_cached_file(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            cache_dir = root / ".cache" / "external-guides"
            cache_dir.mkdir(parents=True)
            cache_file = cache_dir / "guide-a.md"
            cache_file.write_text("cached content", encoding="utf-8")

            stdout = io.StringIO()
            with mock.patch.object(external_guides, "project_root", return_value=root):
                with redirect_stdout(stdout):
                    code = external_guides.clean_guides(self.registry, ["guide-a"])

            self.assertEqual(code, 0)
            self.assertFalse(cache_file.exists())
            self.assertIn("Removed cache", stdout.getvalue())

    def test_clean_skips_missing_cache(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / ".cache" / "external-guides").mkdir(parents=True)

            stdout = io.StringIO()
            with mock.patch.object(external_guides, "project_root", return_value=root):
                with redirect_stdout(stdout):
                    code = external_guides.clean_guides(self.registry, ["guide-a"])

            self.assertEqual(code, 0)
            self.assertIn("SKIP", stdout.getvalue())

    def test_clean_rejects_unknown_id(self) -> None:
        stderr = io.StringIO()
        with redirect_stderr(stderr):
            code = external_guides.clean_guides(self.registry, ["unknown-id"])
        self.assertEqual(code, 1)
        self.assertIn("Unknown guide ids", stderr.getvalue())

    def test_clean_all_removes_all_cached_files(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            cache_dir = root / ".cache" / "external-guides"
            cache_dir.mkdir(parents=True)
            cache_file = cache_dir / "guide-a.md"
            cache_file.write_text("cached", encoding="utf-8")

            stdout = io.StringIO()
            with mock.patch.object(external_guides, "project_root", return_value=root):
                with redirect_stdout(stdout):
                    code = external_guides.clean_guides(self.registry, [])

            self.assertEqual(code, 0)
            self.assertFalse(cache_file.exists())


import urllib.error

if __name__ == "__main__":
    unittest.main()

"""Tests for atomic_write.py — _find_sotp() binary selection and atomic_write_file() fallback."""

import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import atomic_write


class FindSotpTest(unittest.TestCase):
    def setUp(self) -> None:
        # Reset cached state before each test.
        atomic_write._SOTP_COMPATIBLE = None
        atomic_write._SOTP_SEARCHED = False

    def tearDown(self) -> None:
        # Reset cached state after each test.
        atomic_write._SOTP_COMPATIBLE = None
        atomic_write._SOTP_SEARCHED = False

    def test_incompatible_path_binary_selects_compatible_local(self) -> None:
        """When PATH binary fails probe but a local build binary succeeds, use the local one."""
        project_root = Path(atomic_write.__file__).resolve().parent.parent
        debug_binary = str(project_root / "target/debug/sotp")

        def fake_probe(binary: str) -> bool:
            # PATH binary is incompatible; local debug build is compatible.
            return binary == debug_binary

        with (
            mock.patch("atomic_write.shutil.which", return_value="/usr/bin/sotp"),
            mock.patch(
                "atomic_write._probe_supports_file_write_atomic",
                side_effect=fake_probe,
            ),
            mock.patch.object(Path, "is_file", return_value=True),
        ):
            result = atomic_write._find_sotp()
            self.assertEqual(result, debug_binary)

    def test_no_compatible_binary_returns_none(self) -> None:
        """When no binary supports file write-atomic, _find_sotp() returns None."""
        with (
            mock.patch("atomic_write.shutil.which", return_value=None),
            mock.patch.object(Path, "is_file", return_value=False),
        ):
            result = atomic_write._find_sotp()
            self.assertIsNone(result)

    def test_probe_result_is_cached(self) -> None:
        """Second call to _find_sotp() uses cached result without re-probing."""
        with (
            mock.patch("atomic_write.shutil.which", return_value=None),
            mock.patch.object(Path, "is_file", return_value=False),
        ):
            result1 = atomic_write._find_sotp()
            self.assertIsNone(result1)
            self.assertTrue(atomic_write._SOTP_SEARCHED)

        # Second call should use cache, not re-probe
        with mock.patch(
            "atomic_write._probe_supports_file_write_atomic"
        ) as mock_probe:
            result2 = atomic_write._find_sotp()
            self.assertIsNone(result2)
            mock_probe.assert_not_called()


class AtomicWriteFileFallbackTest(unittest.TestCase):
    def setUp(self) -> None:
        atomic_write._SOTP_COMPATIBLE = None
        atomic_write._SOTP_SEARCHED = False

    def tearDown(self) -> None:
        atomic_write._SOTP_COMPATIBLE = None
        atomic_write._SOTP_SEARCHED = False

    def test_fallback_to_write_text_when_no_binary(self) -> None:
        """When no compatible binary exists, atomic_write_file falls back to Path.write_text."""
        with (
            mock.patch("atomic_write.shutil.which", return_value=None),
            mock.patch.object(Path, "is_file", return_value=False),
            tempfile.TemporaryDirectory() as tmpdir,
        ):
            target = Path(tmpdir) / "output.txt"
            atomic_write.atomic_write_file(target, "hello world")
            self.assertEqual(target.read_text(encoding="utf-8"), "hello world")

    def test_raises_on_sotp_runtime_failure(self) -> None:
        """When a compatible binary is found but fails at runtime, OSError is raised."""
        atomic_write._SOTP_SEARCHED = True
        atomic_write._SOTP_COMPATIBLE = "/fake/sotp"

        with mock.patch(
            "atomic_write.subprocess.run",
            return_value=subprocess.CompletedProcess(
                args=[], returncode=1, stdout=b"", stderr=b"disk full"
            ),
        ):
            with self.assertRaises(OSError) as ctx:
                atomic_write.atomic_write_file(Path("/tmp/test.txt"), "data")
            self.assertIn("disk full", str(ctx.exception))


class ProbeSupportTest(unittest.TestCase):
    def test_probe_returns_false_on_oserror(self) -> None:
        """_probe_supports_file_write_atomic returns False when binary is not found."""
        with mock.patch(
            "atomic_write.subprocess.run", side_effect=OSError("not found")
        ):
            self.assertFalse(atomic_write._probe_supports_file_write_atomic("/fake"))

    def test_probe_returns_false_on_timeout(self) -> None:
        """_probe_supports_file_write_atomic returns False when binary times out."""
        with mock.patch(
            "atomic_write.subprocess.run",
            side_effect=subprocess.TimeoutExpired(cmd="sotp", timeout=5),
        ):
            self.assertFalse(atomic_write._probe_supports_file_write_atomic("/fake"))

    def test_probe_returns_false_on_nonzero_exit(self) -> None:
        """_probe_supports_file_write_atomic returns False when binary exits non-zero."""
        with mock.patch(
            "atomic_write.subprocess.run",
            return_value=subprocess.CompletedProcess(
                args=[], returncode=1, stdout=b"", stderr=b""
            ),
        ):
            self.assertFalse(atomic_write._probe_supports_file_write_atomic("/fake"))

    def test_probe_returns_true_on_zero_exit(self) -> None:
        """_probe_supports_file_write_atomic returns True when binary exits zero."""
        with mock.patch(
            "atomic_write.subprocess.run",
            return_value=subprocess.CompletedProcess(
                args=[], returncode=0, stdout=b"", stderr=b""
            ),
        ):
            self.assertTrue(atomic_write._probe_supports_file_write_atomic("/fake"))


if __name__ == "__main__":
    unittest.main()

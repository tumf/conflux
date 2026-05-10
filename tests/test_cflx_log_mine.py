#!/usr/bin/env python3
"""Regression tests for scripts/cflx-log-mine.py.

These tests intentionally use synthetic temporary logs only. They exercise the
stdlib-only helper as both an imported module (unit-like logic checks) and as a
CLI (integration checks over a generated log root).
"""

from __future__ import annotations

import importlib.util
import json
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path
from types import ModuleType


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPT_PATH = REPO_ROOT / "scripts" / "cflx-log-mine.py"


def load_log_miner() -> ModuleType:
    spec = importlib.util.spec_from_file_location("cflx_log_mine", SCRIPT_PATH)
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


LOG_MINER = load_log_miner()


class CflxLogMineTests(unittest.TestCase):
    def make_log_root(self) -> tempfile.TemporaryDirectory[str]:
        temp_dir = tempfile.TemporaryDirectory()
        marker = Path(temp_dir.name) / ".last-checked"
        marker.write_text("marker\n", encoding="utf-8")
        old_time = time.time() - 60
        import os

        os.utime(marker, (old_time, old_time))
        return temp_dir

    def write_large_fixture(self, log_root: Path) -> Path:
        log_path = log_root / "conflux-test" / "2026-05-10.log"
        log_path.parent.mkdir(parents=True)
        with log_path.open("w", encoding="utf-8") as handle:
            for idx in range(4000):
                handle.write(f"2026-05-10T00:00:{idx % 60:02d}Z INFO noise: filler line {idx}\n")
            handle.write(
                "2026-05-10T00:01:00Z ERROR cflx::runner: src/run.rs:42: "
                "failed change_id=\"alpha\" project_id=\"volatile-project\" "
                "branch=\"feature/random\" pid=123 pgid=456 /Users/example/repo/src/main.rs\n"
            )
            handle.write(
                "2026-05-10T00:01:01Z INFO tui: M key pressed ResolveMerge change_id=\"alpha\"\n"
            )
            handle.write(
                "2026-05-10T00:01:02Z INFO merge: ResolveStarted change_id=\"alpha\"\n"
            )
            for idx in range(4000, 4300):
                handle.write(f"2026-05-10T00:02:{idx % 60:02d}Z INFO noise: tail line {idx}\n")
            handle.write(
                "2026-05-10T00:03:00Z ERROR cflx::runner: src/run.rs:43: "
                "failed change_id=\"beta\" project_id=\"other-project\" branch=\"other\"\n"
            )
        log_path.touch()
        return log_path

    def run_cli(self, *args: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [sys.executable, str(SCRIPT_PATH), *args],
            cwd=REPO_ROOT,
            check=True,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

    def test_mine_streams_large_logs_and_reports_markers(self) -> None:
        with self.make_log_root() as temp_dir:
            self.write_large_fixture(Path(temp_dir))
            report = LOG_MINER.mine(Path(temp_dir), 0.0, max_examples=3, context_radius=2)

        self.assertEqual(len(report["files_seen"]), 1)
        self.assertGreater(report["total_lines_scanned"], 4000)
        self.assertGreaterEqual(len(report["groups"]), 1)
        self.assertTrue(report["manual_events"])
        self.assertTrue(report["action_events"])
        alpha_example = report["groups"][0]["examples"][0]
        self.assertIn("alpha", "\n".join([alpha_example["text"], *alpha_example["context"]]))
        self.assertLessEqual(len(alpha_example["context"]), 5)
        self.assertFalse(hasattr(LOG_MINER, "read_lines"))

    def test_json_output_keeps_top_level_schema(self) -> None:
        with self.make_log_root() as temp_dir:
            self.write_large_fixture(Path(temp_dir))
            completed = self.run_cli(
                "--log-root",
                temp_dir,
                "--format",
                "json",
                "--top",
                "30",
            )

        report = json.loads(completed.stdout)
        for key in (
            "log_root",
            "since_mtime",
            "since_iso",
            "files_seen",
            "total_lines_scanned",
            "groups",
            "manual_events",
            "action_events",
        ):
            self.assertIn(key, report)
        self.assertIsInstance(report["groups"][0]["examples"][0]["context"], list)
        self.assertIsInstance(report["manual_events"][0]["line"], int)
        self.assertIsInstance(report["action_events"][0]["file"], str)

    def test_change_id_filtering_keeps_only_matching_hits(self) -> None:
        with self.make_log_root() as temp_dir:
            self.write_large_fixture(Path(temp_dir))
            completed = self.run_cli(
                "--log-root",
                temp_dir,
                "--change-id",
                "alpha",
                "--format",
                "json",
            )

        report = json.loads(completed.stdout)
        blobs: list[str] = []
        for group in report["groups"]:
            self.assertNotIn("beta", group["key"])
            for example in group["examples"]:
                blobs.append("\n".join([example["text"], *example["context"]]))
        for hit in [*report["manual_events"], *report["action_events"]]:
            blobs.append("\n".join([hit["text"], *hit["context"]]))
        self.assertTrue(blobs)
        self.assertTrue(all("alpha" in blob for blob in blobs))
        self.assertTrue(all("beta" not in blob for blob in blobs))

    def test_normalize_redacts_volatile_group_key_values(self) -> None:
        line = (
            'failed at /Users/example/private/repo project_id="abc123" '
            'branch="feature/alpha" change_id="alpha" pid=123 pgid=456 after 789 ms'
        )
        normalized = LOG_MINER.normalize(line)
        self.assertIn("/*", normalized)
        self.assertIn("project_id=*", normalized)
        self.assertIn("branch=*", normalized)
        self.assertIn("change_id=*", normalized)
        self.assertIn("pid=*", normalized)
        self.assertIn("pgid=*", normalized)
        self.assertIn("after * ms", normalized)
        self.assertNotIn("/Users/example", normalized)
        self.assertNotIn("abc123", normalized)
        self.assertNotIn("feature/alpha", normalized)

    def test_text_cli_large_log_report_contains_all_sections(self) -> None:
        with self.make_log_root() as temp_dir:
            self.write_large_fixture(Path(temp_dir))
            completed = self.run_cli("--log-root", temp_dir, "--top", "30")

        output = completed.stdout
        self.assertIn("Log root:", output)
        self.assertIn("Top error/warning groups:", output)
        self.assertIn("Manual operation markers:", output)
        self.assertIn("Action timeline markers:", output)
        self.assertIn("Recommended follow-up queries:", output)
        self.assertIn("alpha", output)


if __name__ == "__main__":
    unittest.main()

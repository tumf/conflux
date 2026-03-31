"""Tests for .no-delta marker support in cflx-workflow cflx.py."""

import importlib.util
from pathlib import Path


def _load_manager_class():
    repo_root = Path(__file__).resolve().parents[2]
    script_path = repo_root / "skills" / "cflx-workflow" / "scripts" / "cflx.py"

    spec = importlib.util.spec_from_file_location("cflx_workflow_script", script_path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"Failed to load module spec from {script_path}")

    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.OpenSpecManager


def _write_change(
    change_dir: Path, with_marker: bool = False, with_delta_dir: bool = False
) -> None:
    change_dir.mkdir(parents=True)
    (change_dir / "proposal.md").write_text(
        "# Change: test\n\n## Why\nTest change\n",
        encoding="utf-8",
    )
    (change_dir / "tasks.md").write_text(
        "## Implementation Tasks\n\n- [ ] test task\n",
        encoding="utf-8",
    )

    specs_dir = change_dir / "specs"
    specs_dir.mkdir()

    if with_marker:
        (specs_dir / ".no-delta").write_text("intentional no-delta\n", encoding="utf-8")

    if with_delta_dir:
        delta_dir = specs_dir / "demo"
        delta_dir.mkdir()
        (delta_dir / "spec.md").write_text(
            "## ADDED Requirements\n\n"
            "### Requirement: Demo\n\n"
            "#### Scenario: Demo\n"
            "- **GIVEN** a\n- **WHEN** b\n- **THEN** c\n",
            encoding="utf-8",
        )


class TestNoDeltaMarkerValidation:
    def test_strict_validation_passes_with_only_no_delta_marker(self, tmp_path):
        OpenSpecManager = _load_manager_class()
        change_id = "marker-only"
        change_dir = tmp_path / "openspec" / "changes" / change_id
        _write_change(change_dir, with_marker=True, with_delta_dir=False)

        manager = OpenSpecManager(root_dir=str(tmp_path))
        ok, errors = manager.validate_change(change_id, strict=True)

        assert ok
        assert errors == []

    def test_strict_validation_fails_when_marker_and_delta_dir_coexist(self, tmp_path):
        OpenSpecManager = _load_manager_class()
        change_id = "marker-conflict"
        change_dir = tmp_path / "openspec" / "changes" / change_id
        _write_change(change_dir, with_marker=True, with_delta_dir=True)

        manager = OpenSpecManager(root_dir=str(tmp_path))
        ok, errors = manager.validate_change(change_id, strict=True)

        assert not ok
        assert any(".no-delta" in err and "conflicts" in err for err in errors)

    def test_strict_validation_fails_without_marker_or_delta_dir(self, tmp_path):
        OpenSpecManager = _load_manager_class()
        change_id = "no-marker-no-delta"
        change_dir = tmp_path / "openspec" / "changes" / change_id
        _write_change(change_dir, with_marker=False, with_delta_dir=False)

        manager = OpenSpecManager(root_dir=str(tmp_path))
        ok, errors = manager.validate_change(change_id, strict=True)

        assert not ok
        assert any("No spec deltas found" in err for err in errors)


class TestNoDeltaMarkerArchiveFlow:
    def test_archive_change_succeeds_with_no_delta_marker(self, tmp_path):
        OpenSpecManager = _load_manager_class()
        change_id = "archive-marker"
        change_dir = tmp_path / "openspec" / "changes" / change_id
        _write_change(change_dir, with_marker=True, with_delta_dir=False)

        manager = OpenSpecManager(root_dir=str(tmp_path))
        success, message = manager.archive_change(change_id)

        assert success
        assert "Archived to openspec/changes/archive" in message
        assert (tmp_path / "openspec" / "changes" / "archive" / change_id).exists()

    def test_simulate_spec_promotion_returns_empty_errors_without_spec_dirs(
        self, tmp_path
    ):
        OpenSpecManager = _load_manager_class()
        change_id = "simulate-empty"
        change_dir = tmp_path / "openspec" / "changes" / change_id
        _write_change(change_dir, with_marker=True, with_delta_dir=False)

        manager = OpenSpecManager(root_dir=str(tmp_path))
        errors = manager._simulate_spec_promotion(change_dir)

        assert errors == []

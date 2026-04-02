"""Tests invalid change directory filtering for cflx.py list/show flows."""

import importlib.util
from pathlib import Path

import pytest


SCRIPT_RELATIVE_PATHS = [
    "skills/cflx-workflow/scripts/cflx.py",
    "skills/cflx-proposal/scripts/cflx.py",
]


def _load_manager_class(script_relative_path: str):
    repo_root = Path(__file__).resolve().parents[2]
    script_path = repo_root / script_relative_path

    spec = importlib.util.spec_from_file_location(script_path.stem, script_path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"Failed to load module spec from {script_path}")

    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.OpenSpecManager


def _write_valid_change(change_dir: Path, title: str = "valid") -> None:
    change_dir.mkdir(parents=True)
    (change_dir / "proposal.md").write_text(
        f"# Change: {title}\n\n## Why\nTest\n",
        encoding="utf-8",
    )
    (change_dir / "tasks.md").write_text(
        "## Implementation Tasks\n\n- [ ] verify\n",
        encoding="utf-8",
    )


@pytest.mark.parametrize("script_relative_path", SCRIPT_RELATIVE_PATHS)
def test_list_changes_ignores_invalid_dir(script_relative_path: str, tmp_path, capsys):
    OpenSpecManager = _load_manager_class(script_relative_path)

    valid_change = tmp_path / "openspec" / "changes" / "valid-change"
    _write_valid_change(valid_change)

    invalid_change = tmp_path / "openspec" / "changes" / "broken-dir"
    invalid_change.mkdir(parents=True)
    (invalid_change / "tasks.md").write_text(
        "## Implementation Tasks\n\n- [ ] missing proposal\n",
        encoding="utf-8",
    )

    manager = OpenSpecManager(root_dir=str(tmp_path))
    changes = manager.list_changes()

    captured = capsys.readouterr()
    ids = {change["id"] for change in changes}

    assert "valid-change" in ids
    assert "broken-dir" not in ids
    assert "Warning: Ignoring invalid change directory 'broken-dir'" in captured.err


@pytest.mark.parametrize("script_relative_path", SCRIPT_RELATIVE_PATHS)
def test_find_change_dir_ignores_invalid(script_relative_path: str, tmp_path, capsys):
    OpenSpecManager = _load_manager_class(script_relative_path)

    invalid_change = tmp_path / "openspec" / "changes" / "ghost-dir"
    invalid_change.mkdir(parents=True)
    (invalid_change / "tasks.md").write_text(
        "## Implementation Tasks\n\n- [ ] missing proposal\n",
        encoding="utf-8",
    )

    manager = OpenSpecManager(root_dir=str(tmp_path))
    result = manager._find_change_dir("ghost-dir")

    captured = capsys.readouterr()

    assert result is None
    assert "Warning: Ignoring invalid change directory 'ghost-dir'" in captured.err

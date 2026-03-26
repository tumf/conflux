"""Tests for spec-only acceptance behavior (archive-readiness checks)."""

import sys
from pathlib import Path

import pytest

SKILL_ROOT = Path(__file__).parent.parent / "cflx-proposal" / "scripts"
sys.path.insert(0, str(SKILL_ROOT))

from cflx import OpenSpecManager  # noqa: E402

FIXTURES = Path(__file__).parent / "fixtures" / "proposal_modes"


def _make_manager(tmp_path: Path) -> OpenSpecManager:
    manager = OpenSpecManager(root_dir=str(tmp_path))
    return manager


def _create_spec_only_change(
    tmp_path: Path,
    change_id: str,
    delta_content: str,
    *,
    canonical_content: str = "",
) -> OpenSpecManager:
    """Helper: scaffold a spec-only change and optional canonical spec."""
    change_dir = tmp_path / "openspec" / "changes" / change_id
    change_dir.mkdir(parents=True)

    (change_dir / "proposal.md").write_text(
        f"# Change: {change_id}\n\n**Change Type**: spec-only\n\n## Why\nTest.\n",
        encoding="utf-8",
    )
    (change_dir / "tasks.md").write_text(
        "## Specification Tasks\n\n"
        "- [ ] Promote spec delta (expected canonical result: new requirement added)\n",
        encoding="utf-8",
    )

    specs = change_dir / "specs" / "demo-capability"
    specs.mkdir(parents=True)
    (specs / "spec.md").write_text(delta_content, encoding="utf-8")

    if canonical_content:
        canonical = tmp_path / "openspec" / "specs" / "demo-capability"
        canonical.mkdir(parents=True)
        (canonical / "spec.md").write_text(canonical_content, encoding="utf-8")

    return OpenSpecManager(root_dir=str(tmp_path))


class TestSpecOnlyAcceptancePass:
    """Spec-only changes with ADDED deltas pass archive-readiness checks."""

    def test_added_delta_passes_validation(self, tmp_path):
        delta = (
            "## ADDED Requirements\n\n"
            "### Requirement: New feature\n\n"
            "#### Scenario: Basic\n\n"
            "- **GIVEN** setup\n- **WHEN** action\n- **THEN** result\n"
        )
        manager = _create_spec_only_change(tmp_path, "spec-only-pass", delta)
        ok, errors, warnings = manager.validate_change("spec-only-pass", strict=True)
        assert ok, f"Expected validation to pass. Errors: {errors}"
        # No archive-risk warning for ADDED-only delta
        risk_warnings = [w for w in warnings if "ARCHIVE-RISK" in w]
        assert risk_warnings == []

    def test_fixture_spec_only_passes(self):
        """The canonical spec-only fixture passes strict validation."""
        manager = OpenSpecManager(root_dir=str(FIXTURES.parent.parent))
        manager.changes_dir = FIXTURES
        manager.archive_dir = FIXTURES / "archive"
        manager.specs_dir = FIXTURES.parent / "canonical_specs"
        ok, errors, warnings = manager.validate_change("spec-only", strict=True)
        change_type_errors = [e for e in errors if "Change Type" in e]
        assert change_type_errors == []


class TestSpecOnlyAcceptanceFail:
    """Spec-only changes with archive-risk deltas produce warnings that acceptance should fail on."""

    def test_modified_only_delta_produces_archive_risk_warning(self, tmp_path):
        """MODIFIED-only spec-only delta emits an ARCHIVE-RISK warning (no-op promotion risk)."""
        delta = (
            "## MODIFIED Requirements\n\n"
            "### Requirement: Existing feature\n\n"
            "Updated description.\n\n"
            "#### Scenario: Existing\n\n"
            "- **GIVEN** setup\n- **WHEN** action\n- **THEN** updated result\n"
        )
        manager = _create_spec_only_change(tmp_path, "spec-only-mod", delta)
        ok, errors, warnings = manager.validate_change("spec-only-mod", strict=True)
        risk_warnings = [w for w in warnings if "ARCHIVE-RISK" in w]
        assert len(risk_warnings) >= 1, "Expected ARCHIVE-RISK warning for MODIFIED-only delta"

    def test_fixture_spec_only_risky_produces_warning(self):
        """The spec-only-risky fixture produces an ARCHIVE-RISK warning."""
        manager = OpenSpecManager(root_dir=str(FIXTURES.parent.parent))
        manager.changes_dir = FIXTURES
        manager.archive_dir = FIXTURES / "archive"
        manager.specs_dir = FIXTURES.parent / "canonical_specs"
        ok, errors, warnings = manager.validate_change("spec-only-risky", strict=True)
        risk_warnings = [w for w in warnings if "ARCHIVE-RISK" in w]
        assert len(risk_warnings) >= 1, (
            f"Expected ARCHIVE-RISK warning for spec-only-risky fixture. Got warnings: {warnings}"
        )

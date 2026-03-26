"""Tests for Change Type classification and archive-risk warning in cflx.py validation."""

import sys
from pathlib import Path

import pytest

# Allow importing cflx.py from the cflx-proposal skill
SKILL_ROOT = Path(__file__).parent.parent / "cflx-proposal" / "scripts"
sys.path.insert(0, str(SKILL_ROOT))

from cflx import OpenSpecManager  # noqa: E402

FIXTURES = Path(__file__).parent / "fixtures" / "proposal_modes"


def _manager_for_fixture(name: str) -> tuple[OpenSpecManager, str]:
    """Return a manager rooted at the fixture directory and the change_id."""
    fixture_dir = FIXTURES / name
    # The fixture dir acts as the openspec/changes/<id> directory.
    # To simulate the OpenSpecManager, we need a root where openspec/changes/<id> exists.
    # We use the fixtures/proposal_modes directory as the changes root by creating a
    # temporary manager that points one level up.
    manager = OpenSpecManager(root_dir=str(FIXTURES.parent.parent))
    # Override the changes_dir to point to our fixtures
    manager.changes_dir = FIXTURES
    manager.archive_dir = FIXTURES / "archive"
    manager.specs_dir = FIXTURES.parent / "canonical_specs"
    return manager, name


# ---------------------------------------------------------------------------
# change_type_validation
# ---------------------------------------------------------------------------

class TestChangeTypeValidation:
    """Task 1.1: validate_change rejects missing or invalid Change Type in strict mode."""

    def test_spec_only_type_accepted(self):
        manager, cid = _manager_for_fixture("spec-only")
        ok, errors, warnings = manager.validate_change(cid, strict=True)
        change_type_errors = [e for e in errors if "Change Type" in e]
        assert change_type_errors == [], f"Unexpected Change Type errors: {change_type_errors}"

    def test_implementation_type_accepted(self):
        manager, cid = _manager_for_fixture("implementation")
        ok, errors, warnings = manager.validate_change(cid, strict=True)
        change_type_errors = [e for e in errors if "Change Type" in e]
        assert change_type_errors == [], f"Unexpected Change Type errors: {change_type_errors}"

    def test_hybrid_type_accepted(self):
        manager, cid = _manager_for_fixture("hybrid")
        ok, errors, warnings = manager.validate_change(cid, strict=True)
        change_type_errors = [e for e in errors if "Change Type" in e]
        assert change_type_errors == [], f"Unexpected Change Type errors: {change_type_errors}"

    def test_missing_change_type_rejected_in_strict_mode(self, tmp_path):
        """A proposal without a Change Type field fails strict validation."""
        change_dir = tmp_path / "openspec" / "changes" / "missing-type"
        change_dir.mkdir(parents=True)
        (change_dir / "proposal.md").write_text(
            "# Change: No type\n\n## Why\nTest.\n", encoding="utf-8"
        )
        (change_dir / "tasks.md").write_text(
            "## Implementation Tasks\n\n- [ ] Do something (verification: `pytest`)\n",
            encoding="utf-8",
        )
        specs = change_dir / "specs" / "demo"
        specs.mkdir(parents=True)
        (specs / "spec.md").write_text(
            "## ADDED Requirements\n\n### Requirement: X\n\n#### Scenario: Y\n\n- **GIVEN** a\n- **WHEN** b\n- **THEN** c\n",
            encoding="utf-8",
        )
        manager = OpenSpecManager(root_dir=str(tmp_path))
        ok, errors, warnings = manager.validate_change("missing-type", strict=True)
        assert not ok
        assert any("Change Type" in e for e in errors)

    def test_invalid_change_type_rejected(self, tmp_path):
        """A proposal with an unrecognised Change Type fails strict validation."""
        change_dir = tmp_path / "openspec" / "changes" / "bad-type"
        change_dir.mkdir(parents=True)
        (change_dir / "proposal.md").write_text(
            "# Change: Bad type\n\n**Change Type**: foobar\n\n## Why\nTest.\n",
            encoding="utf-8",
        )
        (change_dir / "tasks.md").write_text(
            "## Implementation Tasks\n\n- [ ] Do something (verification: `pytest`)\n",
            encoding="utf-8",
        )
        specs = change_dir / "specs" / "demo"
        specs.mkdir(parents=True)
        (specs / "spec.md").write_text(
            "## ADDED Requirements\n\n### Requirement: X\n\n#### Scenario: Y\n\n- **GIVEN** a\n- **WHEN** b\n- **THEN** c\n",
            encoding="utf-8",
        )
        manager = OpenSpecManager(root_dir=str(tmp_path))
        ok, errors, warnings = manager.validate_change("bad-type", strict=True)
        assert not ok
        assert any("invalid Change Type" in e for e in errors)

    def test_change_type_not_required_in_non_strict_mode(self, tmp_path):
        """Missing Change Type is tolerated in non-strict mode."""
        change_dir = tmp_path / "openspec" / "changes" / "no-type-lenient"
        change_dir.mkdir(parents=True)
        (change_dir / "proposal.md").write_text(
            "# Change: No type lenient\n\n## Why\nTest.\n", encoding="utf-8"
        )
        (change_dir / "tasks.md").write_text(
            "## Implementation Tasks\n\n- [ ] Do something\n", encoding="utf-8"
        )
        manager = OpenSpecManager(root_dir=str(tmp_path))
        ok, errors, warnings = manager.validate_change("no-type-lenient", strict=False)
        change_type_errors = [e for e in errors if "Change Type" in e]
        assert change_type_errors == []


# ---------------------------------------------------------------------------
# archive_risk_warning
# ---------------------------------------------------------------------------

class TestArchiveRiskWarning:
    """Task 2.2: spec-only proposals with MODIFIED/REMOVED-only deltas emit a warning."""

    def test_spec_only_added_delta_no_warning(self):
        """ADDED-only spec-only delta does not trigger archive-risk warning."""
        manager, cid = _manager_for_fixture("spec-only")
        ok, errors, warnings = manager.validate_change(cid, strict=True)
        risk_warnings = [w for w in warnings if "ARCHIVE-RISK" in w]
        assert risk_warnings == [], f"Unexpected archive-risk warnings: {risk_warnings}"

    def test_spec_only_modified_only_delta_triggers_warning(self):
        """MODIFIED-only spec-only delta triggers archive-risk warning."""
        manager, cid = _manager_for_fixture("spec-only-risky")
        ok, errors, warnings = manager.validate_change(cid, strict=True)
        risk_warnings = [w for w in warnings if "ARCHIVE-RISK" in w]
        assert len(risk_warnings) >= 1, "Expected at least one ARCHIVE-RISK warning"
        assert "MODIFIED" in risk_warnings[0] or "canonical promotion" in risk_warnings[0]

    def test_implementation_proposal_no_archive_warning(self):
        """Implementation proposals never get archive-risk warnings."""
        manager, cid = _manager_for_fixture("implementation")
        ok, errors, warnings = manager.validate_change(cid, strict=True)
        risk_warnings = [w for w in warnings if "ARCHIVE-RISK" in w]
        assert risk_warnings == []

    def test_hybrid_proposal_no_archive_warning(self):
        """Hybrid proposals with ADDED deltas do not get archive-risk warnings."""
        manager, cid = _manager_for_fixture("hybrid")
        ok, errors, warnings = manager.validate_change(cid, strict=True)
        risk_warnings = [w for w in warnings if "ARCHIVE-RISK" in w]
        assert risk_warnings == []

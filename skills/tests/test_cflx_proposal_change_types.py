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
        assert change_type_errors == [], (
            f"Unexpected Change Type errors: {change_type_errors}"
        )

    def test_implementation_type_accepted(self):
        manager, cid = _manager_for_fixture("implementation")
        ok, errors, warnings = manager.validate_change(cid, strict=True)
        change_type_errors = [e for e in errors if "Change Type" in e]
        assert change_type_errors == [], (
            f"Unexpected Change Type errors: {change_type_errors}"
        )

    def test_hybrid_type_accepted(self):
        manager, cid = _manager_for_fixture("hybrid")
        ok, errors, warnings = manager.validate_change(cid, strict=True)
        change_type_errors = [e for e in errors if "Change Type" in e]
        assert change_type_errors == [], (
            f"Unexpected Change Type errors: {change_type_errors}"
        )

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
        assert (
            "MODIFIED" in risk_warnings[0] or "canonical promotion" in risk_warnings[0]
        )

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


class TestBehaviorVerificationWarnings:
    """Behavior-changing proposals should carry ownership and runnable verification guidance."""

    def test_behavior_task_requires_verification_ownership_in_warning_mode(
        self, tmp_path
    ):
        change_dir = tmp_path / "openspec" / "changes" / "behavior-missing-ownership"
        change_dir.mkdir(parents=True)
        (change_dir / "proposal.md").write_text(
            "# Change: Add CLI export\n\n"
            "**Change Type**: implementation\n\n"
            "## Problem/Context\n"
            "Add a CLI command that exports records to disk.\n\n"
            "## Acceptance Criteria\n"
            "- Export command writes the expected file for valid input\n",
            encoding="utf-8",
        )
        (change_dir / "tasks.md").write_text(
            "## Implementation Tasks\n\n"
            "- [ ] Implement export CLI command and persist output file (verification: cargo test --test export_cli)\n",
            encoding="utf-8",
        )
        specs = change_dir / "specs" / "demo"
        specs.mkdir(parents=True)
        (specs / "spec.md").write_text(
            "## ADDED Requirements\n\n"
            "### Requirement: Export CLI\n\n"
            "#### Scenario: Export succeeds\n\n"
            "- **GIVEN** valid input\n"
            "- **WHEN** the user runs the export command\n"
            "- **THEN** the output file is written\n",
            encoding="utf-8",
        )
        manager = OpenSpecManager(root_dir=str(tmp_path))
        ok, errors, warnings = manager.validate_change(
            "behavior-missing-ownership", strict=True, evidence_mode="warn"
        )
        assert ok
        assert errors == []
        assert any("verification ownership" in w for w in warnings)

    def test_executable_surface_without_runnable_verification_warns(self, tmp_path):
        change_dir = tmp_path / "openspec" / "changes" / "cli-no-runnable-check"
        change_dir.mkdir(parents=True)
        (change_dir / "proposal.md").write_text(
            "# Change: Add preview CLI\n\n"
            "**Change Type**: implementation\n\n"
            "## Problem/Context\n"
            "Add a CLI preview command with safe no-op behavior.\n\n"
            "## Acceptance Criteria\n"
            "- Preview command shows planned actions without writing state\n",
            encoding="utf-8",
        )
        (change_dir / "tasks.md").write_text(
            "## Implementation Tasks\n\n"
            "- [ ] Define preview command responsibilities and safety notes (verification: manual - reviewer inspects proposal wording)\n"
            "- [ ] Document CLI error cases and dry-run semantics (verification: manual - reviewer checks docs)\n",
            encoding="utf-8",
        )
        specs = change_dir / "specs" / "demo"
        specs.mkdir(parents=True)
        (specs / "spec.md").write_text(
            "## ADDED Requirements\n\n"
            "### Requirement: Preview CLI\n\n"
            "#### Scenario: Dry run avoids writes\n\n"
            "- **GIVEN** pending actions\n"
            "- **WHEN** preview mode runs\n"
            "- **THEN** no persistent state changes occur\n",
            encoding="utf-8",
        )
        manager = OpenSpecManager(root_dir=str(tmp_path))
        ok, errors, warnings = manager.validate_change(
            "cli-no-runnable-check", strict=True, evidence_mode="warn"
        )
        assert ok
        assert errors == []
        assert any("no runnable verification" in w for w in warnings)
        assert any("executable surface" in w for w in warnings)
        assert any("Artifact-oriented tasks dominate" in w for w in warnings)

    def test_runtime_claim_without_behavior_task_warns(self, tmp_path):
        change_dir = tmp_path / "openspec" / "changes" / "runtime-claim-no-impl"
        change_dir.mkdir(parents=True)
        (change_dir / "proposal.md").write_text(
            "# Change: Add webhook delivery\n\n"
            "**Change Type**: implementation\n\n"
            "## Problem/Context\n"
            "Implement webhook delivery and retry behavior for notifications.\n\n"
            "## Acceptance Criteria\n"
            "- Webhook delivery retries failed notifications\n",
            encoding="utf-8",
        )
        (change_dir / "tasks.md").write_text(
            "## Implementation Tasks\n\n"
            "- [ ] Document webhook delivery responsibilities (verification: manual - reviewer reads proposal)\n"
            "- [ ] Define retry policy notes (verification: manual - reviewer checks tasks)\n",
            encoding="utf-8",
        )
        specs = change_dir / "specs" / "demo"
        specs.mkdir(parents=True)
        (specs / "spec.md").write_text(
            "## ADDED Requirements\n\n"
            "### Requirement: Webhook delivery\n\n"
            "#### Scenario: Retry on failure\n\n"
            "- **GIVEN** a failed webhook attempt\n"
            "- **WHEN** retry logic runs\n"
            "- **THEN** the notification is retried\n",
            encoding="utf-8",
        )
        manager = OpenSpecManager(root_dir=str(tmp_path))
        ok, errors, warnings = manager.validate_change(
            "runtime-claim-no-impl", strict=True, evidence_mode="warn"
        )
        assert ok
        assert errors == []
        assert any("claim runtime behavior changes" in w for w in warnings)

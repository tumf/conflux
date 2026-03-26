"""
Regression tests for the shared spec promotion engine.

Covers:
- ADDED-only deltas (append new requirements)
- MODIFIED-only deltas (replace matching requirements in-place)
- REMOVED-only deltas (delete matching requirements)
- Mixed deltas (ADDED + MODIFIED + REMOVED together)
- No-op or missing-target error cases (spec_only_no_op, missing MODIFIED/REMOVED targets)
"""
from pathlib import Path

import pytest

from shared.cflx_spec_promotion import (
    delta_to_canonical,
    merge_spec_delta,
    simulate_promotion,
)

FIXTURES = Path(__file__).parent / "fixtures" / "archive_promotion"


def _load(fixture: str) -> tuple:
    """Return (canonical_text, delta_text) for a named fixture."""
    base = FIXTURES / fixture
    canonical = (base / "canonical.md").read_text(encoding="utf-8")
    delta = (base / "delta.md").read_text(encoding="utf-8")
    return canonical, delta


# ---------------------------------------------------------------------------
# ADDED-only
# ---------------------------------------------------------------------------

class TestAddedOnly:
    def test_new_requirement_appears_in_result(self):
        canonical, delta = _load("added_only")
        result, errors = merge_spec_delta(canonical, delta)
        assert errors == []
        assert "New Added Requirement" in result

    def test_existing_requirement_retained(self):
        canonical, delta = _load("added_only")
        result, errors = merge_spec_delta(canonical, delta)
        assert errors == []
        assert "Existing Requirement" in result

    def test_result_differs_from_canonical(self):
        canonical, delta = _load("added_only")
        result, errors = merge_spec_delta(canonical, delta)
        assert errors == []
        assert result.strip() != canonical.strip()


# ---------------------------------------------------------------------------
# MODIFIED-only
# ---------------------------------------------------------------------------

class TestModifiedOnly:
    def test_requirement_block_is_replaced(self):
        canonical, delta = _load("modified_only")
        result, errors = merge_spec_delta(canonical, delta)
        assert errors == []
        # New content present
        assert "full audit trail" in result

    def test_old_content_not_retained(self):
        canonical, delta = _load("modified_only")
        result, errors = merge_spec_delta(canonical, delta)
        assert errors == []
        # Old scenario text should not survive
        assert "old approach" not in result

    def test_heading_appears_exactly_once(self):
        canonical, delta = _load("modified_only")
        result, errors = merge_spec_delta(canonical, delta)
        assert errors == []
        assert result.count("### Requirement: Reliable Archive Tracking") == 1

    def test_no_errors_returned(self):
        canonical, delta = _load("modified_only")
        _, errors = merge_spec_delta(canonical, delta)
        assert errors == []


# ---------------------------------------------------------------------------
# REMOVED-only
# ---------------------------------------------------------------------------

class TestRemovedOnly:
    def test_removed_requirement_absent_from_result(self):
        canonical, delta = _load("removed_only")
        result, errors = merge_spec_delta(canonical, delta)
        assert errors == []
        assert "Legacy Archive Behavior" not in result

    def test_retained_requirement_still_present(self):
        canonical, delta = _load("removed_only")
        result, errors = merge_spec_delta(canonical, delta)
        assert errors == []
        assert "Retained Requirement" in result

    def test_result_well_formed(self):
        canonical, delta = _load("removed_only")
        result, errors = merge_spec_delta(canonical, delta)
        assert errors == []
        assert result.strip() != ""

    def test_no_errors_returned(self):
        canonical, delta = _load("removed_only")
        _, errors = merge_spec_delta(canonical, delta)
        assert errors == []


# ---------------------------------------------------------------------------
# Mixed (ADDED + MODIFIED + REMOVED)
# ---------------------------------------------------------------------------

class TestMixed:
    def test_added_requirement_present(self):
        canonical, delta = _load("mixed")
        result, errors = merge_spec_delta(canonical, delta)
        assert errors == []
        assert "Newly Added Requirement" in result

    def test_modified_requirement_replaced(self):
        canonical, delta = _load("mixed")
        result, errors = merge_spec_delta(canonical, delta)
        assert errors == []
        assert "improved result" in result
        assert "old result" not in result

    def test_removed_requirement_gone(self):
        canonical, delta = _load("mixed")
        result, errors = merge_spec_delta(canonical, delta)
        assert errors == []
        assert "To Be Removed" not in result

    def test_no_errors_returned(self):
        canonical, delta = _load("mixed")
        _, errors = merge_spec_delta(canonical, delta)
        assert errors == []


# ---------------------------------------------------------------------------
# No-op and missing-target error cases
# ---------------------------------------------------------------------------

class TestNoOpOrMissingTarget:
    def test_modified_missing_target_returns_error(self):
        """MODIFIED delta targeting a non-existent canonical requirement must fail."""
        canonical = "## Requirements\n\n### Requirement: Existing\n\nSome content.\n\n#### Scenario: S\n- **GIVEN** x\n- **WHEN** y\n- **THEN** z\n"
        delta = "## MODIFIED Requirements\n\n### Requirement: Missing Requirement\n\nNew content.\n\n#### Scenario: S\n- **GIVEN** a\n- **WHEN** b\n- **THEN** c\n"
        result, errors = merge_spec_delta(canonical, delta)
        assert errors, "Expected errors for missing MODIFIED target"
        assert any("Missing Requirement" in e for e in errors)
        assert result == canonical  # canonical unchanged

    def test_removed_missing_target_returns_error(self):
        """REMOVED delta targeting a non-existent canonical requirement must fail."""
        canonical = "## Requirements\n\n### Requirement: Existing\n\nSome content.\n\n#### Scenario: S\n- **GIVEN** x\n- **WHEN** y\n- **THEN** z\n"
        delta = "## REMOVED Requirements\n\n### Requirement: Ghost Requirement\n\nGhost content.\n\n#### Scenario: S\n- **GIVEN** a\n- **WHEN** b\n- **THEN** c\n"
        result, errors = merge_spec_delta(canonical, delta)
        assert errors, "Expected errors for missing REMOVED target"
        assert any("Ghost Requirement" in e for e in errors)
        assert result == canonical  # canonical unchanged

    def test_canonical_unchanged_on_error(self):
        """Canonical must be returned unchanged when promotion errors occur."""
        canonical = "## Requirements\n\n### Requirement: Only One\n\nContent.\n\n#### Scenario: S\n- **GIVEN** x\n- **WHEN** y\n- **THEN** z\n"
        delta = "## MODIFIED Requirements\n\n### Requirement: Does Not Exist\n\nNew.\n\n#### Scenario: S\n- **GIVEN** a\n- **WHEN** b\n- **THEN** c\n"
        result, errors = merge_spec_delta(canonical, delta)
        assert result == canonical


# ---------------------------------------------------------------------------
# Spec-only no-op regression
# ---------------------------------------------------------------------------

class TestSpecOnlyNoOp:
    """
    Regression: a MODIFIED delta whose content is identical to canonical
    must fail with a no-op error rather than silently reporting success.

    This mirrors the session-analysis finding where the old append-only
    implementation silently ignored MODIFIED deltas.
    """

    def test_spec_only_no_op_returns_error(self):
        canonical, delta = _load("spec_only_no_op")
        result, errors = merge_spec_delta(canonical, delta)
        assert errors, "Expected no-op error for identical MODIFIED delta"
        assert any("no-op" in e.lower() or "no canonical diff" in e.lower() for e in errors)

    def test_spec_only_no_op_canonical_unchanged(self):
        canonical, delta = _load("spec_only_no_op")
        result, errors = merge_spec_delta(canonical, delta)
        assert result == canonical

    def test_simulate_promotion_no_op_errors(self):
        canonical, delta = _load("spec_only_no_op")
        _, errors = simulate_promotion(canonical, delta)
        assert errors, "simulate_promotion must surface no-op error"


# ---------------------------------------------------------------------------
# Standalone functions for -k no_op_or_missing_target filter
# ---------------------------------------------------------------------------

def test_no_op_or_missing_target_modified_fails():
    """MODIFIED delta targeting a non-existent requirement must return errors."""
    canonical = (
        "## Requirements\n\n"
        "### Requirement: Present\n\nContent.\n\n"
        "#### Scenario: S\n- **GIVEN** x\n- **WHEN** y\n- **THEN** z\n"
    )
    delta = (
        "## MODIFIED Requirements\n\n"
        "### Requirement: Not Present\n\nNew.\n\n"
        "#### Scenario: S\n- **GIVEN** a\n- **WHEN** b\n- **THEN** c\n"
    )
    result, errors = merge_spec_delta(canonical, delta)
    assert errors
    assert result == canonical


def test_no_op_or_missing_target_removed_fails():
    """REMOVED delta targeting a non-existent requirement must return errors."""
    canonical = (
        "## Requirements\n\n"
        "### Requirement: Present\n\nContent.\n\n"
        "#### Scenario: S\n- **GIVEN** x\n- **WHEN** y\n- **THEN** z\n"
    )
    delta = (
        "## REMOVED Requirements\n\n"
        "### Requirement: Absent\n\nOld content.\n\n"
        "#### Scenario: S\n- **GIVEN** a\n- **WHEN** b\n- **THEN** c\n"
    )
    result, errors = merge_spec_delta(canonical, delta)
    assert errors
    assert result == canonical


def test_no_op_or_missing_target_no_op_identical_modified_fails():
    """MODIFIED delta with identical content to canonical must return no-op error."""
    block = (
        "### Requirement: Same Requirement\n\n"
        "Identical content.\n\n"
        "#### Scenario: S\n- **GIVEN** x\n- **WHEN** y\n- **THEN** z\n"
    )
    canonical = f"## Requirements\n\n{block}"
    delta = f"## MODIFIED Requirements\n\n{block}"
    result, errors = merge_spec_delta(canonical, delta)
    assert errors
    assert "no-op" in errors[0].lower() or "no canonical diff" in errors[0].lower()


# ---------------------------------------------------------------------------
# delta_to_canonical (new spec creation)
# ---------------------------------------------------------------------------

class TestDeltaToCanonical:
    def test_added_only_produces_canonical(self):
        _, delta = _load("added_only")
        result = delta_to_canonical(delta)
        assert "New Added Requirement" in result
        assert "## ADDED Requirements" not in result

    def test_new_spec_via_simulate_promotion(self):
        _, delta = _load("added_only")
        result, errors = simulate_promotion(None, delta)
        assert errors == []
        assert result is not None
        assert "New Added Requirement" in result

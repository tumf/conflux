#!/usr/bin/env python3
"""Archive-preparation guard: promotion must not silently drop a canonical scenario.

An OpenSpec ``## MODIFIED Requirements`` block replaces a canonical requirement
wholesale. That is exactly what makes it dangerous: a delta that simply forgets
to carry a paragraph or a ``#### Scenario:`` deletes it at archive time, and the
loss is invisible in review because the diff only shows what the delta *says*.

This script compares, for every MODIFIED requirement in a change, the canonical
scenario set against the promoted one and fails when a canonical scenario would
disappear. A scenario a change means to replace is declared explicitly inside the
delta with an HTML comment, which is invisible to every OpenSpec parser::

    <!-- replaces-scenario: Marking during execution does not add to DynamicQueue -->

Declarations are checked in both directions. An undeclared drop fails, and so
does a declaration for a scenario that is not actually dropped, so the list
cannot rot into a blanket exemption.

Usage::

    python3 scripts/check-scenario-set.py [change-id ...] [--repo-root PATH]

With no change id every active change that carries spec deltas is checked, which
is what keeps the Make target useful after this change is archived.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

REQUIREMENT_RE = re.compile(r"^###\s+Requirement:\s*(.+?)\s*$")
SCENARIO_RE = re.compile(r"^####\s+Scenario:\s*(.+?)\s*$")
SECTION_RE = re.compile(r"^##\s+(ADDED|MODIFIED|REMOVED|RENAMED)\s+Requirements\s*$")
REPLACES_RE = re.compile(r"<!--\s*replaces-scenario:\s*(.+?)\s*-->")


def parse_requirements(lines: list[str], *, modified_only: bool) -> dict[str, dict]:
    """Map requirement name -> {"scenarios": [...], "replaces": [...]}.

    Canonical specs repeat requirement names (historical duplicates that archive
    never merged). Scenario sets are therefore *unioned* across every occurrence:
    a scenario that survives in either copy has not been dropped.
    """
    requirements: dict[str, dict] = {}
    in_scope = not modified_only
    current: str | None = None

    for line in lines:
        section = SECTION_RE.match(line)
        if section:
            in_scope = (not modified_only) or section.group(1) == "MODIFIED"
            current = None
            continue

        requirement = REQUIREMENT_RE.match(line)
        if requirement:
            current = requirement.group(1) if in_scope else None
            if current is not None:
                requirements.setdefault(current, {"scenarios": [], "replaces": []})
            continue

        if current is None:
            continue

        scenario = SCENARIO_RE.match(line)
        if scenario:
            entry = requirements[current]["scenarios"]
            if scenario.group(1) not in entry:
                entry.append(scenario.group(1))
            continue

        for replaced in REPLACES_RE.findall(line):
            entry = requirements[current]["replaces"]
            if replaced not in entry:
                entry.append(replaced)

    return requirements


def read(path: Path) -> list[str]:
    return path.read_text(encoding="utf-8").splitlines()


def check_capability(canonical_path: Path, delta_path: Path) -> list[str]:
    """Return one failure line per problem found for this capability."""
    failures: list[str] = []
    delta = parse_requirements(read(delta_path), modified_only=True)
    if not delta:
        return failures

    if not canonical_path.exists():
        # A MODIFIED requirement against a capability with no canonical spec is
        # itself a promotion hazard: there is nothing for it to modify.
        return [
            f"{delta_path}: MODIFIED requirements target missing canonical spec {canonical_path}"
        ]

    canonical = parse_requirements(read(canonical_path), modified_only=False)

    for name, entry in sorted(delta.items()):
        if name not in canonical:
            failures.append(
                f"{delta_path}: MODIFIED requirement '{name}' has no canonical counterpart "
                f"in {canonical_path}"
            )
            continue

        promoted = set(entry["scenarios"])
        declared = set(entry["replaces"])
        canonical_scenarios = canonical[name]["scenarios"]

        dropped = [s for s in canonical_scenarios if s not in promoted]
        undeclared = [s for s in dropped if s not in declared]
        for scenario in undeclared:
            failures.append(
                f"{delta_path}: requirement '{name}' drops canonical scenario "
                f"'{scenario}' without a `<!-- replaces-scenario: ... -->` declaration"
            )

        stale = sorted(declared - set(dropped))
        for scenario in stale:
            failures.append(
                f"{delta_path}: requirement '{name}' declares replacement of "
                f"'{scenario}', which is not a canonical scenario this delta drops"
            )

    return failures


def active_change_ids(root: Path) -> list[str]:
    """Every active change that carries spec deltas, archives excluded."""
    changes = root / "openspec" / "changes"
    if not changes.is_dir():
        return []
    return sorted(
        entry.name
        for entry in changes.iterdir()
        if entry.is_dir() and entry.name != "archive" and (entry / "specs").is_dir()
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("change_ids", nargs="*")
    parser.add_argument("--repo-root", default=".")
    args = parser.parse_args()

    root = Path(args.repo_root).resolve()
    change_ids = args.change_ids or active_change_ids(root)
    if not change_ids:
        print("no active change carries spec deltas; nothing to compare")
        return 0

    failures: list[str] = []
    checked = 0
    for change_id in change_ids:
        change_specs = root / "openspec" / "changes" / change_id / "specs"
        if not change_specs.is_dir():
            print(f"no spec deltas found for change '{change_id}'", file=sys.stderr)
            return 1
        for delta_path in sorted(change_specs.glob("*/spec.md")):
            capability = delta_path.parent.name
            canonical_path = root / "openspec" / "specs" / capability / "spec.md"
            checked += 1
            failures.extend(check_capability(canonical_path, delta_path))

    if failures:
        print("scenario-set check FAILED:", file=sys.stderr)
        for failure in failures:
            print(f"  - {failure}", file=sys.stderr)
        return 1

    print(
        f"scenario-set check passed for {', '.join(change_ids)} "
        f"({checked} capability delta(s) compared against canonical specs)"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())

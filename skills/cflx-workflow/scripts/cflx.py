#!/usr/bin/env python3
"""
CFLX - Conflux workflow management tool
A standalone Python implementation of essential OpenSpec operations.
"""

import argparse
import json
import os
import re
import shutil
import sys
from pathlib import Path
from typing import Dict, List, Optional, Tuple


class Colors:
    """ANSI color codes for terminal output."""

    RESET = "\033[0m"
    BOLD = "\033[1m"
    RED = "\033[91m"
    GREEN = "\033[92m"
    YELLOW = "\033[93m"
    BLUE = "\033[94m"
    CYAN = "\033[96m"


class OpenSpecManager:
    """Manage OpenSpec changes and specifications."""

    def __init__(self, root_dir: str = "."):
        self.root_dir = Path(root_dir).resolve()
        self.changes_dir = self.root_dir / "openspec" / "changes"
        self.archive_dir = self.changes_dir / "archive"
        self.specs_dir = self.root_dir / "openspec" / "specs"

    def list_changes(self, show_specs: bool = False) -> List[Dict]:
        """List all changes or specs."""
        if show_specs:
            return self._list_specs()

        changes = []
        if not self.changes_dir.exists():
            return changes

        for item in self.changes_dir.iterdir():
            if item.is_dir() and item.name != "archive":
                change_info = self._get_change_info(item)
                if change_info:
                    changes.append(change_info)

        # Also check archive
        if self.archive_dir.exists():
            for item in self.archive_dir.iterdir():
                if item.is_dir():
                    change_info = self._get_change_info(item, archived=True)
                    if change_info:
                        changes.append(change_info)

        return sorted(changes, key=lambda x: x.get("id", ""))

    def _list_specs(self) -> List[Dict]:
        """List all specs."""
        specs = []
        if not self.specs_dir.exists():
            return specs

        for item in self.specs_dir.iterdir():
            if item.is_dir():
                spec_file = item / "spec.md"
                if spec_file.exists():
                    specs.append(
                        {
                            "name": item.name,
                            "path": str(spec_file.relative_to(self.root_dir)),
                        }
                    )

        return sorted(specs, key=lambda x: x["name"])

    def _get_change_info(
        self, change_dir: Path, archived: bool = False
    ) -> Optional[Dict]:
        """Extract change information from directory."""
        proposal_file = change_dir / "proposal.md"
        tasks_file = change_dir / "tasks.md"

        info = {
            "id": change_dir.name,
            "path": str(change_dir.relative_to(self.root_dir)),
            "archived": archived,
        }

        # Extract title from proposal.md
        if proposal_file.exists():
            content = proposal_file.read_text(encoding="utf-8")
            # Look for first heading
            match = re.search(r"^#\s+(.+)$", content, re.MULTILINE)
            if match:
                info["title"] = match.group(1).strip()

        # Count tasks
        if tasks_file.exists():
            info.update(self._count_tasks(tasks_file))

        return info

    def _count_tasks(self, tasks_file: Path) -> Dict:
        """Count completed and total tasks."""
        content = tasks_file.read_text(encoding="utf-8")

        # Exclude Future Work, Out of Scope, Notes sections
        sections_to_exclude = ["future work", "out of scope", "notes"]
        lines = content.split("\n")

        in_excluded_section = False
        completed = 0
        total = 0

        for line in lines:
            # Check if we're entering an excluded section
            if line.startswith("##"):
                section_name = line.lstrip("#").strip().lower()
                in_excluded_section = any(
                    excluded in section_name for excluded in sections_to_exclude
                )
                continue

            if in_excluded_section:
                continue

            # Count tasks
            if re.match(r"^\s*[-*]\s*\[[ x]\]", line):
                total += 1
                if re.match(r"^\s*[-*]\s*\[x\]", line):
                    completed += 1

        return {"tasks_completed": completed, "tasks_total": total}

    def show_change(
        self, change_id: str, json_output: bool = False, deltas_only: bool = False
    ) -> Optional[Dict]:
        """Show detailed information about a change."""
        change_dir = self._find_change_dir(change_id)
        if not change_dir:
            return None

        info = {
            "id": change_id,
            "path": str(change_dir.relative_to(self.root_dir)),
            "archived": "archive" in change_dir.parts,
        }

        # Read proposal
        proposal_file = change_dir / "proposal.md"
        if proposal_file.exists():
            info["proposal"] = proposal_file.read_text(encoding="utf-8")

        # Read tasks
        tasks_file = change_dir / "tasks.md"
        if tasks_file.exists():
            info["tasks"] = tasks_file.read_text(encoding="utf-8")
            info.update(self._count_tasks(tasks_file))

        # Read design
        design_file = change_dir / "design.md"
        if design_file.exists():
            info["design"] = design_file.read_text(encoding="utf-8")

        # Read spec deltas
        specs_dir = change_dir / "specs"
        if specs_dir.exists():
            info["specs"] = {}
            for spec_dir in specs_dir.iterdir():
                if spec_dir.is_dir():
                    spec_file = spec_dir / "spec.md"
                    if spec_file.exists():
                        info["specs"][spec_dir.name] = spec_file.read_text(
                            encoding="utf-8"
                        )

        if deltas_only and "specs" in info:
            # Only return spec deltas
            return {"id": change_id, "specs": info["specs"]}

        return info

    def _find_change_dir(self, change_id: str) -> Optional[Path]:
        """Find the directory for a given change ID."""
        # Check active changes
        change_dir = self.changes_dir / change_id
        if change_dir.exists():
            return change_dir

        # Check archive
        archive_change_dir = self.archive_dir / change_id
        if archive_change_dir.exists():
            return archive_change_dir

        return None

    def validate_change(
        self, change_id: Optional[str] = None, strict: bool = False
    ) -> Tuple[bool, List[str]]:
        """Validate a change or all changes."""
        errors = []

        if change_id:
            change_dir = self._find_change_dir(change_id)
            if not change_dir:
                errors.append(f"Change '{change_id}' not found")
                return False, errors

            errors.extend(self._validate_change_dir(change_dir, strict))
        else:
            # Validate all changes
            if self.changes_dir.exists():
                for item in self.changes_dir.iterdir():
                    if item.is_dir() and item.name != "archive":
                        errors.extend(self._validate_change_dir(item, strict))

        return len(errors) == 0, errors

    def _validate_change_dir(self, change_dir: Path, strict: bool) -> List[str]:
        """Validate a single change directory."""
        errors = []
        change_id = change_dir.name

        # Check required files
        proposal_file = change_dir / "proposal.md"
        tasks_file = change_dir / "tasks.md"

        if not proposal_file.exists():
            errors.append(f"{change_id}: Missing proposal.md")

        if not tasks_file.exists():
            errors.append(f"{change_id}: Missing tasks.md")

        # Validate proposal structure
        if proposal_file.exists():
            content = proposal_file.read_text(encoding="utf-8")
            if not re.search(r"^#\s+.+$", content, re.MULTILINE):
                errors.append(f"{change_id}: proposal.md missing title heading")

        # Validate tasks format
        if tasks_file.exists():
            task_errors = self._validate_tasks_file(tasks_file, change_id)
            errors.extend(task_errors)

        # Validate spec deltas (strict mode)
        if strict:
            specs_dir = change_dir / "specs"
            if specs_dir.exists() and specs_dir.is_dir():
                spec_errors = self._validate_specs_dir(specs_dir, change_id)
                errors.extend(spec_errors)
            elif strict:
                errors.append(
                    f"{change_id}: No spec deltas found (required in strict mode)"
                )

        return errors

    def _validate_tasks_file(self, tasks_file: Path, change_id: str) -> List[str]:
        """Validate tasks.md file format."""
        errors = []
        content = tasks_file.read_text(encoding="utf-8")
        lines = content.split("\n")

        in_excluded_section = False
        sections_to_exclude = ["future work", "out of scope", "notes"]

        for i, line in enumerate(lines, 1):
            # Check section headers
            if line.startswith("##"):
                section_name = line.lstrip("#").strip().lower()
                in_excluded_section = any(
                    excluded in section_name for excluded in sections_to_exclude
                )
                continue

            # Check for checkboxes in excluded sections
            if in_excluded_section and re.match(r"^\s*[-*]\s*\[[ x]\]", line):
                errors.append(
                    f"{change_id}: tasks.md:{i}: Checkbox found in excluded section (should be removed)"
                )

            # Check for tasks without checkboxes in active sections
            if (
                not in_excluded_section
                and re.match(r"^\s*[-*]\s+[^[]", line)
                and line.strip()
            ):
                # Might be a task without checkbox
                if not line.strip().startswith(("##", "#", "---", "```")):
                    errors.append(
                        f"{change_id}: tasks.md:{i}: Possible task without checkbox: {line.strip()[:50]}"
                    )

        return errors

    def _validate_specs_dir(self, specs_dir: Path, change_id: str) -> List[str]:
        """Validate spec delta files."""
        errors = []

        for spec_dir in specs_dir.iterdir():
            if not spec_dir.is_dir():
                continue

            spec_file = spec_dir / "spec.md"
            if not spec_file.exists():
                errors.append(f"{change_id}: Missing spec.md in {spec_dir.name}")
                continue

            content = spec_file.read_text(encoding="utf-8")

            # Check for delta markers
            has_delta = False
            for marker in [
                "## ADDED Requirements",
                "## MODIFIED Requirements",
                "## REMOVED Requirements",
            ]:
                if marker in content:
                    has_delta = True
                    break

            if not has_delta:
                errors.append(
                    f"{change_id}: {spec_dir.name}/spec.md missing delta markers (ADDED/MODIFIED/REMOVED)"
                )

            # Check for scenarios
            requirements = re.findall(r"^### Requirement:", content, re.MULTILINE)
            scenarios = re.findall(r"^#### Scenario:", content, re.MULTILINE)

            if requirements and not scenarios:
                errors.append(
                    f"{change_id}: {spec_dir.name}/spec.md has requirements but no scenarios"
                )

        return errors

    def archive_change(
        self, change_id: str, skip_specs: bool = False
    ) -> Tuple[bool, str]:
        """Archive a deployed change."""
        change_dir = self.changes_dir / change_id

        if not change_dir.exists():
            return False, f"Change '{change_id}' not found"

        if "archive" in change_dir.parts:
            return False, f"Change '{change_id}' is already archived"

        # Validate before archiving
        is_valid, errors = self.validate_change(change_id, strict=True)
        if not is_valid:
            return False, f"Validation failed:\n" + "\n".join(errors)

        # Create archive directory if needed
        self.archive_dir.mkdir(parents=True, exist_ok=True)

        # Move to archive
        archive_dest = self.archive_dir / change_id
        if archive_dest.exists():
            return False, f"Archive destination already exists: {archive_dest}"

        shutil.move(str(change_dir), str(archive_dest))

        # Update specs (unless skip_specs)
        if not skip_specs:
            specs_updated = self._update_specs_from_change(archive_dest)
            return (
                True,
                f"Archived to {archive_dest.relative_to(self.root_dir)}\nSpecs updated: {specs_updated}",
            )

        return True, f"Archived to {archive_dest.relative_to(self.root_dir)}"

    def _update_specs_from_change(self, change_dir: Path) -> List[str]:
        """Update canonical specs from change deltas."""
        updated = []
        specs_dir = change_dir / "specs"

        if not specs_dir.exists():
            return updated

        for spec_dir in specs_dir.iterdir():
            if not spec_dir.is_dir():
                continue

            spec_file = spec_dir / "spec.md"
            if not spec_file.exists():
                continue

            # Update canonical spec
            canonical_spec = self.specs_dir / spec_dir.name / "spec.md"
            canonical_spec.parent.mkdir(parents=True, exist_ok=True)

            delta_content = spec_file.read_text(encoding="utf-8")

            # If canonical spec exists, merge; otherwise create
            if canonical_spec.exists():
                canonical_content = canonical_spec.read_text(encoding="utf-8")
                merged_content = self._merge_spec_delta(
                    canonical_content, delta_content
                )
                canonical_spec.write_text(merged_content, encoding="utf-8")
            else:
                # Extract requirements from delta
                canonical_spec.write_text(
                    self._delta_to_canonical(delta_content), encoding="utf-8"
                )

            updated.append(spec_dir.name)

        return updated

    def _merge_spec_delta(self, canonical: str, delta: str) -> str:
        """Merge delta into canonical spec."""
        # Simple implementation: append deltas
        # In a real implementation, this would be more sophisticated
        result = canonical

        # Extract ADDED requirements
        added_section = re.search(
            r"## ADDED Requirements(.+?)(?=## |$)", delta, re.DOTALL
        )
        if added_section:
            result += "\n\n" + added_section.group(1).strip()

        # Extract MODIFIED requirements (replace matching sections)
        # This is simplified - real implementation would need proper merging

        return result

    def _delta_to_canonical(self, delta: str) -> str:
        """Convert delta format to canonical spec format."""
        # Remove delta markers
        canonical = re.sub(
            r"^## (ADDED|MODIFIED|REMOVED) Requirements",
            "## Requirements",
            delta,
            flags=re.MULTILINE,
        )
        return canonical


def print_changes(changes: List[Dict], show_specs: bool = False):
    """Print changes or specs in a formatted way."""
    if show_specs:
        print(f"\n{Colors.BOLD}Specifications:{Colors.RESET}\n")
        for spec in changes:
            print(f"  {Colors.CYAN}{spec['name']}{Colors.RESET}")
            print(f"    Path: {spec['path']}")
            print()
    else:
        print(f"\n{Colors.BOLD}Changes:{Colors.RESET}\n")
        for change in changes:
            status = (
                f"{Colors.YELLOW}[ARCHIVED]{Colors.RESET}"
                if change.get("archived")
                else f"{Colors.GREEN}[ACTIVE]{Colors.RESET}"
            )
            print(f"  {status} {Colors.BOLD}{change['id']}{Colors.RESET}")
            if "title" in change:
                print(f"    Title: {change['title']}")
            if "tasks_total" in change:
                completed = change.get("tasks_completed", 0)
                total = change["tasks_total"]
                progress = f"{completed}/{total}"
                if completed == total and total > 0:
                    progress = f"{Colors.GREEN}{progress}{Colors.RESET}"
                print(f"    Tasks: {progress}")
            print(f"    Path: {change['path']}")
            print()


def print_change_detail(change: Dict, json_output: bool = False):
    """Print detailed change information."""
    if json_output:
        print(json.dumps(change, indent=2))
        return

    print(f"\n{Colors.BOLD}Change: {change['id']}{Colors.RESET}")
    print(f"Path: {change['path']}")
    print(f"Status: {'ARCHIVED' if change.get('archived') else 'ACTIVE'}")

    if "tasks_total" in change:
        print(f"Tasks: {change.get('tasks_completed', 0)}/{change['tasks_total']}")

    if "proposal" in change:
        print(f"\n{Colors.BOLD}Proposal:{Colors.RESET}")
        print(
            change["proposal"][:500] + "..."
            if len(change["proposal"]) > 500
            else change["proposal"]
        )

    if "specs" in change:
        print(f"\n{Colors.BOLD}Spec Deltas:{Colors.RESET}")
        for spec_name, spec_content in change["specs"].items():
            print(f"\n  {Colors.CYAN}{spec_name}:{Colors.RESET}")
            print(
                f"  {spec_content[:300]}..."
                if len(spec_content) > 300
                else f"  {spec_content}"
            )


def main():
    parser = argparse.ArgumentParser(
        description="CFLX - Conflux workflow management tool",
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )

    subparsers = parser.add_subparsers(dest="command", help="Commands")

    # list command
    list_parser = subparsers.add_parser("list", help="List changes or specs")
    list_parser.add_argument(
        "--specs", action="store_true", help="List specs instead of changes"
    )

    # show command
    show_parser = subparsers.add_parser("show", help="Show change details")
    show_parser.add_argument("change_id", help="Change ID to show")
    show_parser.add_argument("--json", action="store_true", help="Output as JSON")
    show_parser.add_argument(
        "--deltas-only", action="store_true", help="Show only spec deltas"
    )

    # validate command
    validate_parser = subparsers.add_parser("validate", help="Validate changes")
    validate_parser.add_argument(
        "change_id", nargs="?", help="Change ID to validate (omit for all)"
    )
    validate_parser.add_argument(
        "--strict", action="store_true", help="Strict validation mode"
    )

    # archive command
    archive_parser = subparsers.add_parser("archive", help="Archive a deployed change")
    archive_parser.add_argument("change_id", help="Change ID to archive")
    archive_parser.add_argument("--yes", action="store_true", help="Skip confirmation")
    archive_parser.add_argument(
        "--skip-specs", action="store_true", help="Skip spec updates"
    )

    args = parser.parse_args()

    if not args.command:
        parser.print_help()
        return 1

    manager = OpenSpecManager()

    try:
        if args.command == "list":
            changes = manager.list_changes(show_specs=args.specs)
            print_changes(changes, show_specs=args.specs)

        elif args.command == "show":
            change = manager.show_change(
                args.change_id, json_output=args.json, deltas_only=args.deltas_only
            )
            if not change:
                print(
                    f"{Colors.RED}Error: Change '{args.change_id}' not found{Colors.RESET}",
                    file=sys.stderr,
                )
                return 1
            print_change_detail(change, json_output=args.json)

        elif args.command == "validate":
            is_valid, errors = manager.validate_change(
                args.change_id, strict=args.strict
            )
            if is_valid:
                print(f"{Colors.GREEN}✓ Validation passed{Colors.RESET}")
                return 0
            else:
                print(
                    f"{Colors.RED}✗ Validation failed:{Colors.RESET}", file=sys.stderr
                )
                for error in errors:
                    print(f"  {error}", file=sys.stderr)
                return 1

        elif args.command == "archive":
            if not args.yes:
                response = input(f"Archive change '{args.change_id}'? [y/N] ")
                if response.lower() != "y":
                    print("Cancelled")
                    return 0

            success, message = manager.archive_change(
                args.change_id, skip_specs=args.skip_specs
            )
            if success:
                print(f"{Colors.GREEN}✓ {message}{Colors.RESET}")
                return 0
            else:
                print(f"{Colors.RED}✗ {message}{Colors.RESET}", file=sys.stderr)
                return 1

        return 0

    except Exception as e:
        print(f"{Colors.RED}Error: {e}{Colors.RESET}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())

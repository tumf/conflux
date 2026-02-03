#!/usr/bin/env python3
"""
Package and validate a skill into a distributable .skill file.
"""

import argparse
import re
import sys
import zipfile
from pathlib import Path
from typing import Optional


def validate_yaml_frontmatter(skill_md_path: Path) -> tuple[bool, list[str]]:
    """Validate YAML frontmatter in SKILL.md."""
    errors = []

    content = skill_md_path.read_text()

    # Check for frontmatter
    if not content.startswith("---\n"):
        errors.append("SKILL.md must start with YAML frontmatter (---)")
        return False, errors

    # Extract frontmatter
    parts = content.split("---\n", 2)
    if len(parts) < 3:
        errors.append("SKILL.md frontmatter must end with --- on its own line")
        return False, errors

    frontmatter = parts[1]

    # Check required fields
    has_name = bool(re.search(r"^name:\s*.+", frontmatter, re.MULTILINE))
    has_description = bool(re.search(r"^description:\s*.+", frontmatter, re.MULTILINE))

    if not has_name:
        errors.append("YAML frontmatter missing required field: name")
    if not has_description:
        errors.append("YAML frontmatter missing required field: description")

    # Check description quality
    desc_match = re.search(r"^description:\s*(.+)$", frontmatter, re.MULTILINE)
    if desc_match:
        description = desc_match.group(1).strip()
        if "TODO" in description:
            errors.append("Description contains TODO - please complete it")
        if len(description) < 20:
            errors.append("Description is too short (minimum 20 characters)")

    return len(errors) == 0, errors


def validate_skill_structure(skill_dir: Path) -> tuple[bool, list[str]]:
    """Validate skill directory structure."""
    errors = []

    # Check SKILL.md exists
    skill_md = skill_dir / "SKILL.md"
    if not skill_md.exists():
        errors.append(f"Required file not found: SKILL.md")
        return False, errors

    # Validate frontmatter
    valid_frontmatter, fm_errors = validate_yaml_frontmatter(skill_md)
    errors.extend(fm_errors)

    # Check for TODOs in SKILL.md body
    content = skill_md.read_text()
    body = content.split("---\n", 2)[2] if content.count("---\n") >= 2 else content
    if "TODO" in body:
        errors.append("SKILL.md body contains TODO items - please complete them")

    # Warn about empty resource directories
    for subdir in ["scripts", "references", "assets"]:
        dir_path = skill_dir / subdir
        if dir_path.exists():
            files = list(dir_path.rglob("*"))
            files = [f for f in files if f.is_file()]
            if not files:
                print(f"⚠️  Warning: {subdir}/ directory is empty", file=sys.stderr)

    return len(errors) == 0, errors


def package_skill(skill_dir: Path, output_dir: Optional[Path] = None) -> Path:
    """Package skill into a .skill file (zip with .skill extension)."""
    skill_name = skill_dir.name

    if output_dir is None:
        output_dir = skill_dir.parent

    output_dir.mkdir(parents=True, exist_ok=True)
    output_file = output_dir / f"{skill_name}.skill"

    # Remove existing package
    if output_file.exists():
        output_file.unlink()

    # Create zip file with .skill extension
    with zipfile.ZipFile(output_file, "w", zipfile.ZIP_DEFLATED) as zipf:
        for file_path in skill_dir.rglob("*"):
            if file_path.is_file():
                arcname = file_path.relative_to(skill_dir.parent)
                zipf.write(file_path, arcname)

    return output_file


def main():
    parser = argparse.ArgumentParser(
        description="Package and validate a skill into a .skill file"
    )
    parser.add_argument(
        "skill_path",
        type=Path,
        help="Path to skill directory",
    )
    parser.add_argument(
        "output_dir",
        type=Path,
        nargs="?",
        help="Output directory (default: parent of skill directory)",
    )

    args = parser.parse_args()

    skill_dir = args.skill_path.resolve()

    if not skill_dir.exists():
        print(f"Error: Skill directory not found: {skill_dir}", file=sys.stderr)
        sys.exit(1)

    if not skill_dir.is_dir():
        print(f"Error: Not a directory: {skill_dir}", file=sys.stderr)
        sys.exit(1)

    print(f"Validating skill: {skill_dir.name}")

    # Validate structure
    valid, errors = validate_skill_structure(skill_dir)

    if not valid:
        print("\n❌ Validation failed:", file=sys.stderr)
        for error in errors:
            print(f"  - {error}", file=sys.stderr)
        sys.exit(1)

    print("✅ Validation passed")

    # Package skill
    print("\nPackaging skill...")
    output_file = package_skill(skill_dir, args.output_dir)

    print(f"✅ Skill packaged: {output_file}")
    print(f"\nDistribute this file to users or install with:")
    print(f"  cp {output_file} ~/.config/Claude/skills/")


if __name__ == "__main__":
    main()

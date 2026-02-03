#!/usr/bin/env python3
"""
Initialize a new skill with proper structure and template files.
"""

import argparse
import sys
from pathlib import Path


def create_skill_structure(skill_name: str, output_path: Path) -> None:
    """Create the skill directory structure with template files."""
    skill_dir = output_path / skill_name

    if skill_dir.exists():
        print(f"Error: Skill directory already exists: {skill_dir}", file=sys.stderr)
        sys.exit(1)

    # Create directories
    skill_dir.mkdir(parents=True)
    (skill_dir / "scripts").mkdir()
    (skill_dir / "references").mkdir()
    (skill_dir / "assets").mkdir()

    # Create SKILL.md template
    skill_md = f"""---
name: {skill_name}
description: TODO: Describe what this skill does and when to use it. Include specific triggers and contexts.
---

# {skill_name.replace("-", " ").title()}

TODO: Add instructions for using this skill.

## Quick Start

TODO: Add quick start instructions or examples.

## Resources

### Scripts

- TODO: List and describe scripts in `scripts/` directory

### References

- TODO: List and describe reference documents in `references/` directory

### Assets

- TODO: List and describe assets in `assets/` directory
"""

    (skill_dir / "SKILL.md").write_text(skill_md)

    # Create example script
    example_script = """#!/usr/bin/env python3
\"\"\"
Example script - customize or delete as needed.
\"\"\"

def main():
    print("Example script executed")

if __name__ == "__main__":
    main()
"""

    script_path = skill_dir / "scripts" / "example.py"
    script_path.write_text(example_script)
    script_path.chmod(0o755)

    # Create example reference
    example_ref = f"""# Example Reference

This is an example reference document for {skill_name}.

Delete or customize this file as needed.
"""

    (skill_dir / "references" / "example.md").write_text(example_ref)

    # Create README for assets
    assets_readme = """# Assets

Place files used in output here (templates, images, etc.).

These files are not meant to be loaded into context, but rather used in the final output.
"""

    (skill_dir / "assets" / "README.md").write_text(assets_readme)

    print(f"✅ Skill initialized: {skill_dir}")
    print("\nNext steps:")
    print(f"1. Edit {skill_dir}/SKILL.md to add instructions")
    print(f"2. Customize or remove example files")
    print(f"3. Add scripts, references, and assets as needed")
    print(f"4. Package with: scripts/package_skill.py {skill_dir}")


def main():
    parser = argparse.ArgumentParser(
        description="Initialize a new skill with proper structure"
    )
    parser.add_argument("skill_name", help="Name of the skill (e.g., 'pdf-editor')")
    parser.add_argument(
        "--path",
        type=Path,
        default=Path("skills"),
        help="Output directory (default: skills/)",
    )

    args = parser.parse_args()

    # Validate skill name
    skill_name = args.skill_name.lower().strip()
    if not skill_name or " " in skill_name:
        print("Error: Skill name must not contain spaces", file=sys.stderr)
        sys.exit(1)

    create_skill_structure(skill_name, args.path)


if __name__ == "__main__":
    main()

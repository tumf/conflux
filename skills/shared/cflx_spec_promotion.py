"""
Shared spec promotion engine for Conflux archive workflow.

Provides requirement-block-aware merge of change deltas into canonical specs.
Handles ADDED (append), MODIFIED (replace in-place), and REMOVED (delete) operations,
and rejects promotions that target missing requirements or produce no canonical diff.
"""
import re
from typing import Dict, List, Optional, Tuple


def _split_spec(content: str) -> Tuple[str, List[Tuple[str, str]]]:
    """
    Split spec content into (preamble, [(normalized_key, full_block), ...]).

    full_block starts at '### Requirement:' and includes all content up to
    (but not including) the next '### Requirement:' heading or end of file.
    """
    parts = re.split(r"(?=^### Requirement:)", content, flags=re.MULTILINE)
    preamble = ""
    blocks: List[Tuple[str, str]] = []
    for part in parts:
        if part.startswith("### Requirement:"):
            heading_match = re.match(r"### Requirement:\s*(.+)", part)
            if heading_match:
                key = heading_match.group(1).strip()
                blocks.append((key, part))
        else:
            preamble = part
    return preamble, blocks


def parse_delta_sections(delta: str) -> Dict[str, Dict[str, str]]:
    """
    Parse delta content into {section_type: {normalized_key: full_block}}.

    section_type is one of ADDED, MODIFIED, REMOVED.
    """
    sections: Dict[str, Dict[str, str]] = {"ADDED": {}, "MODIFIED": {}, "REMOVED": {}}
    section_pattern = re.compile(
        r"^## (ADDED|MODIFIED|REMOVED) Requirements\s*$", re.MULTILINE
    )
    matches = list(section_pattern.finditer(delta))
    for i, match in enumerate(matches):
        section_type = match.group(1)
        start = match.end()
        end = matches[i + 1].start() if i + 1 < len(matches) else len(delta)
        section_content = delta[start:end]
        _, blocks = _split_spec(section_content)
        sections[section_type] = {key: block for key, block in blocks}
    return sections


def _blocks_equal(
    b1: List[Tuple[str, str]], b2: List[Tuple[str, str]]
) -> bool:
    """Return True if two block lists have identical keys and stripped content."""
    if len(b1) != len(b2):
        return False
    for (k1, v1), (k2, v2) in zip(b1, b2):
        if k1 != k2 or v1.strip() != v2.strip():
            return False
    return True


def _reconstruct(preamble: str, blocks: List[Tuple[str, str]]) -> str:
    """Reassemble a spec from its preamble and requirement blocks."""
    parts = []
    if preamble.strip():
        parts.append(preamble.rstrip("\n"))
    for _, block in blocks:
        parts.append(block.rstrip("\n"))
    result = "\n\n".join(parts)
    if result and not result.endswith("\n"):
        result += "\n"
    return result


def merge_spec_delta(canonical: str, delta: str) -> Tuple[str, List[str]]:
    """
    Merge a change delta into a canonical spec using requirement identity matching.

    - ADDED: appends new requirement blocks after existing ones.
    - MODIFIED: replaces matching blocks in their original positions.
    - REMOVED: deletes matching blocks.

    Returns (result_content, errors).
    errors is non-empty when a MODIFIED/REMOVED target is absent from the canonical
    spec, or when promotion would leave the canonical spec byte-for-byte unchanged.
    The canonical content is returned unchanged whenever errors are non-empty.
    """
    errors: List[str] = []
    sections = parse_delta_sections(delta)
    preamble, original_blocks = _split_spec(canonical)
    original_dict = {key: block for key, block in original_blocks}

    # Validate MODIFIED targets exist in canonical
    for key in sections["MODIFIED"]:
        if key not in original_dict:
            errors.append(
                f"MODIFIED target not found in canonical spec: '### Requirement: {key}'"
            )

    # Validate REMOVED targets exist in canonical
    for key in sections["REMOVED"]:
        if key not in original_dict:
            errors.append(
                f"REMOVED target not found in canonical spec: '### Requirement: {key}'"
            )

    if errors:
        return canonical, errors

    # Apply REMOVED and MODIFIED to original blocks (preserve order)
    removed_keys = set(sections["REMOVED"].keys())
    result_blocks: List[Tuple[str, str]] = []
    for key, block in original_blocks:
        if key in removed_keys:
            continue  # delete
        elif key in sections["MODIFIED"]:
            result_blocks.append((key, sections["MODIFIED"][key]))  # replace
        else:
            result_blocks.append((key, block))  # keep

    # Append ADDED blocks at the end
    for key, block in sections["ADDED"].items():
        result_blocks.append((key, block))

    # Reject no-op promotions
    if _blocks_equal(original_blocks, result_blocks):
        errors.append(
            "Archive promotion would produce no canonical diff (no-op archive)"
        )
        return canonical, errors

    return _reconstruct(preamble, result_blocks), []


def simulate_promotion(
    canonical: Optional[str], delta: str
) -> Tuple[Optional[str], List[str]]:
    """
    Simulate spec promotion without writing any files.

    Returns (result_content, errors).
    When canonical is None the spec is new; returns (delta_to_canonical(delta), []).
    """
    if canonical is None:
        return delta_to_canonical(delta), []
    return merge_spec_delta(canonical, delta)


def delta_to_canonical(delta: str) -> str:
    """Convert a delta-format spec to canonical format for brand-new specs."""
    sections = parse_delta_sections(delta)
    all_blocks: List[Tuple[str, str]] = []
    for section_type in ("ADDED", "MODIFIED", "REMOVED"):
        for key, block in sections[section_type].items():
            all_blocks.append((key, block))

    if not all_blocks:
        # Fallback: strip section markers when no requirement blocks were parsed
        return re.sub(
            r"^## (ADDED|MODIFIED|REMOVED) Requirements\s*\n",
            "## Requirements\n",
            delta,
            flags=re.MULTILINE,
        )

    return _reconstruct("", all_blocks)

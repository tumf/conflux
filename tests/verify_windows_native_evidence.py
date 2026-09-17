#!/usr/bin/env python3
"""Validate committed evidence from the native Windows Named Pipe test run."""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path
from typing import NoReturn


def fail(message: str) -> NoReturn:
    raise SystemExit(f"invalid Windows native evidence: {message}")


def main() -> None:
    if len(sys.argv) != 2:
        fail("expected one evidence path")

    path = Path(sys.argv[1])
    try:
        text = path.read_text(encoding="utf-8")
    except OSError as error:
        fail(str(error))

    required = {
        "HOSTNAME": r"(?m)^HOSTNAME:\s*(\S+)\s*$",
        "GIT_COMMIT": r"(?m)^GIT_COMMIT:\s*([0-9a-f]{40})\s*$",
        "RUSTC_HOST": r"(?m)^RUSTC_HOST:\s*(x86_64-pc-windows-msvc)\s*$",
        "TEST_EXIT_CODE": r"(?m)^TEST_EXIT_CODE:\s*(0)\s*$",
        "TEST_COUNT": r"(?m)^TEST_COUNT:\s*([1-9][0-9]*)\s*$",
    }
    values: dict[str, str] = {}
    for key, pattern in required.items():
        match = re.search(pattern, text)
        if match is None:
            fail(f"missing or invalid {key}")
        values[key] = match.group(1)

    commit = values["GIT_COMMIT"]
    checks = [
        ["git", "cat-file", "-e", f"{commit}^{{commit}}"],
        ["git", "merge-base", "--is-ancestor", commit, "HEAD"],
    ]
    for command in checks:
        if subprocess.run(command, check=False).returncode != 0:
            fail(f"tested commit {commit} is not an ancestor of HEAD")

    passed = re.search(r"test result:\s+ok\.\s+([1-9][0-9]*) passed", text)
    if passed is None or int(passed.group(1)) != int(values["TEST_COUNT"]):
        fail("TEST_COUNT does not match a non-zero cargo test result")


if __name__ == "__main__":
    main()

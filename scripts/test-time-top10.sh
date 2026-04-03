#!/usr/bin/env bash
set -euo pipefail

# Measure per-test execution time by running tests one-by-one,
# then print the slowest N tests.
#
# Usage:
#   scripts/test-time-top10.sh
#   scripts/test-time-top10.sh 20
#   scripts/test-time-top10.sh 10 60
#
# Notes:
# - Parses `cargo test -- --list` output and preserves the owning test binary
#   (`--lib`, `--test <name>`, `--bin <name>`), so exact matching works.
# - Heavy suites gated behind `heavy-tests` are excluded from default-path runs.
#
# Args:
#   $1: top N (default: 10)
#   $2: per-test timeout seconds (default: 120)

TOP_N="${1:-10}"
PER_TEST_TIMEOUT="${2:-120}"

if ! [[ "$TOP_N" =~ ^[0-9]+$ ]] || [[ "$TOP_N" -le 0 ]]; then
  echo "TOP_N must be a positive integer, got: $TOP_N" >&2
  exit 1
fi

if ! [[ "$PER_TEST_TIMEOUT" =~ ^[0-9]+$ ]] || [[ "$PER_TEST_TIMEOUT" -le 0 ]]; then
  echo "PER_TEST_TIMEOUT must be a positive integer, got: $PER_TEST_TIMEOUT" >&2
  exit 1
fi

LIST_FILE="$(mktemp)"
RESULTS_FILE="$(mktemp)"
trap 'rm -f "$LIST_FILE" "$RESULTS_FILE"' EXIT

echo "[1/3] Collecting test list..."
cargo test -- --list > "$LIST_FILE"

echo "[2/3] Measuring each test (timeout=${PER_TEST_TIMEOUT}s)..."
python3 - "$LIST_FILE" "$RESULTS_FILE" "$PER_TEST_TIMEOUT" <<'PY'
import re
import subprocess
import sys
import time
from pathlib import Path

list_file = Path(sys.argv[1])
out_file = Path(sys.argv[2])
timeout = int(sys.argv[3])

lines = list_file.read_text().splitlines()

# Parse cargo test -- --list output.
# Each section starts with "Running ..." which tells us the binary.
# Then test names follow as "<name>: test".
# We map each test name to its owning cargo target args.

current_target_args = ["--lib"]  # default fallback
entries = []  # (target_args, test_name)

running_re = re.compile(
    r"Running (?:unittests )?(?:tests/)?([\w/.-]+)"
)

for line in lines:
    stripped = line.strip()

    m = running_re.search(stripped)
    if m:
        raw = m.group(1)
        if "src/lib.rs" in stripped or raw.startswith("src/lib"):
            current_target_args = ["--lib"]
        elif "src/main.rs" in stripped or raw.startswith("src/main"):
            current_target_args = ["--lib"]
        elif "src/bin/" in stripped:
            bin_name = re.sub(r"\.rs$", "", raw.split("/")[-1])
            current_target_args = ["--bin", bin_name]
        elif stripped.startswith("Running tests/") or "/tests/" in stripped:
            test_name = re.sub(r"\.rs$", "", raw.split("/")[-1])
            test_name = re.sub(r"-[0-9a-f]+$", "", test_name)
            current_target_args = ["--test", test_name]
        elif "Doc-tests" in stripped:
            current_target_args = ["--doc"]
        continue

    if stripped.endswith(": test"):
        test_name = stripped[: -len(": test")]
        entries.append((list(current_target_args), test_name))

# deduplicate
seen = set()
unique = []
for target_args, test_name in entries:
    key = (tuple(target_args), test_name)
    if key not in seen:
        seen.add(key)
        unique.append((target_args, test_name))

with out_file.open("w") as f:
    for i, (target_args, test_name) in enumerate(unique, 1):
        display = test_name
        print(f"  - [{i}/{len(unique)}] {display}", flush=True)
        start = time.perf_counter()
        status = "ok"
        rc = 0
        cmd = ["cargo", "test"] + target_args + ["--", "--exact", test_name]
        try:
            proc = subprocess.run(
                cmd,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                timeout=timeout,
                check=False,
            )
            rc = proc.returncode
            if rc != 0:
                status = "fail"
        except subprocess.TimeoutExpired:
            rc = 124
            status = "timeout"

        elapsed = time.perf_counter() - start
        f.write(f"{elapsed:.6f}\t{status}\t{rc}\t{display}\n")
PY

echo "[3/3] Top ${TOP_N} slow tests:"
python3 - "$RESULTS_FILE" "$TOP_N" <<'PY'
import sys
from pathlib import Path

result_file = Path(sys.argv[1])
top_n = int(sys.argv[2])

rows = []
for line in result_file.read_text().splitlines():
    if not line.strip():
        continue
    sec_str, status, rc, name = line.split('\t', 3)
    rows.append((float(sec_str), status, int(rc), name))

rows.sort(key=lambda r: r[0], reverse=True)

top_rows = rows[:top_n]
if not top_rows:
    print("No test timing data found.")
    raise SystemExit(0)

for i, (sec, status, rc, name) in enumerate(top_rows, 1):
    extra = ""
    if status != "ok":
        extra = f" [{status}, rc={rc}]"
    print(f"{i:2d}. {sec:8.3f}s  {name}{extra}")
PY

echo
echo "Done."

#!/usr/bin/env python3
"""Mine Conflux runtime logs for actionable errors.

This script is intentionally stdlib-only so agents can run it from opencode
commands without setup. It reads ~/.local/state/cflx/logs by default, filters
logs newer than .last-checked, groups noisy warnings, and emits a compact JSON
report plus human-readable next-query hints.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
from collections import deque
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Iterable


LEVEL_RE = re.compile(
    r"^(?P<ts>\S+)\s+(?P<level>ERROR|WARN)\s+(?P<target>\S+):\s+(?P<loc>[^:]+:\d+):\s+(?P<msg>.*)$"
)
MANUAL_RE = re.compile(
    r"M key .*ResolveMerge|ResolveMerge\(|Scheduled merge-wait retry intent|notified existing scheduler|started scheduler for manual resolve",
    re.I,
)
ACTION_RE = re.compile(
    r"ResolveStarted|Resolving merge for|Merge resolution attempt|MergeStarted|MergeDeferred|MergeCompleted|ResolveCompleted|ResolveFailed|Retrying deferred merge|Detected MergeWait|Worktree branch .*pre-merge base",
    re.I,
)
ERROR_RE = re.compile(
    r"\b(ERROR|WARN)\b|\berror\b|\bfailed\b|\bpanic\b|\bexit status\b|\bexit code\b|\bNo progress made\b|\bhuman_action_required\b",
    re.I,
)
VOLATILE_REPLACEMENTS = [
    (re.compile(r"project_id=\"[^\"]+\""), "project_id=*"),
    (re.compile(r"branch=\"[^\"]+\""), "branch=*"),
    (re.compile(r"change_id=\"[^\"]+\""), "change_id=*"),
    (re.compile(r"/Users/[^\s;\"]+"), "/*"),
    (re.compile(r"after \d+ ms"), "after * ms"),
    (re.compile(r"pid=\d+"), "pid=*"),
    (re.compile(r"pgid=\d+"), "pgid=*"),
]


@dataclass
class Hit:
    file: str
    line: int
    text: str
    context: list[str]


@dataclass
class PendingHit:
    hit: Hit
    remaining_after: int


@dataclass
class Group:
    key: str
    level: str
    count: int = 0
    files: set[str] = field(default_factory=set)
    examples: list[Hit] = field(default_factory=list)

    def add(
        self, file: Path, line: int, text: str, context: list[str], max_examples: int
    ) -> bool:
        self.count += 1
        self.files.add(str(file))
        if len(self.examples) < max_examples:
            self.examples.append(Hit(str(file), line, text, context))
            return True
        return False


def normalize(text: str) -> str:
    normalized = text.strip()
    for pattern, replacement in VOLATILE_REPLACEMENTS:
        normalized = pattern.sub(replacement, normalized)
    normalized = re.sub(r"\s+", " ", normalized)
    return normalized[:260]


def marker_mtime(log_root: Path, marker_name: str) -> float:
    marker = log_root / marker_name
    if marker.exists():
        return marker.stat().st_mtime
    return 0.0


def iter_log_files(log_root: Path, since_mtime: float) -> Iterable[Path]:
    for path in sorted(log_root.rglob("*.log")):
        try:
            if path.stat().st_mtime >= since_mtime:
                yield path
        except OSError:
            continue


def iter_lines(path: Path) -> Iterable[tuple[int, str]]:
    try:
        with path.open("r", encoding="utf-8", errors="replace") as handle:
            for line_no, line in enumerate(handle, start=1):
                yield line_no, line.rstrip("\n\r")
    except OSError:
        return


def append_after_context(pending_hits: list[PendingHit], context_line: str) -> None:
    for pending in pending_hits:
        if pending.remaining_after > 0:
            pending.hit.context.append(context_line)
            pending.remaining_after -= 1


def retain_pending_hits(pending_hits: list[PendingHit]) -> list[PendingHit]:
    return [pending for pending in pending_hits if pending.remaining_after > 0]


def make_hit(
    file: Path,
    line_no: int,
    text: str,
    previous_context: Iterable[str],
    context_line: str,
    context_radius: int,
) -> PendingHit:
    hit = Hit(str(file), line_no, text[:1000], [*previous_context, context_line])
    return PendingHit(hit=hit, remaining_after=context_radius)


def classify(level: str, target: str, message: str) -> str:
    msg = message.lower()
    if "failed to refresh" in msg and "snapshot" in msg:
        return "stale-refresh-snapshot"
    if "ls-remote" in msg or "git fetch" in msg or "could not resolve host" in msg:
        return "remote-git-network"
    if "human_action_required" in msg or "acceptance must confirm rejection" in msg:
        return "human-gated-rejection"
    if "no progress made" in msg:
        return "agent-progress-stall"
    if "worktree branch" in msg and "pre-merge base" in msg:
        return "merge-presync-verification"
    if "failed to execute git" in msg and "no such file or directory" in msg:
        return "missing-working-directory"
    return f"{level.lower()}:{target}:{normalize(message)}"


def mine(
    log_root: Path, since_mtime: float, max_examples: int, context_radius: int
) -> dict:
    groups: dict[str, Group] = {}
    manual_events: list[Hit] = []
    action_events: list[Hit] = []
    files_seen: list[str] = []
    total_lines = 0

    for path in iter_log_files(log_root, since_mtime):
        file_seen = False
        previous_context: deque[str] = deque(maxlen=context_radius)
        pending_hits: list[PendingHit] = []

        for line_no, line in iter_lines(path):
            if not file_seen:
                files_seen.append(str(path))
                file_seen = True
            total_lines += 1
            context_line = f"{line_no}: {line[:1000]}"
            append_after_context(pending_hits, context_line)
            pending_hits = retain_pending_hits(pending_hits)

            manual = MANUAL_RE.search(line)
            action = ACTION_RE.search(line)
            if manual and len(manual_events) < max_examples * 10:
                pending = make_hit(
                    path,
                    line_no,
                    line,
                    previous_context,
                    context_line,
                    context_radius,
                )
                manual_events.append(pending.hit)
                pending_hits.append(pending)
            if action and len(action_events) < max_examples * 20:
                pending = make_hit(
                    path,
                    line_no,
                    line,
                    previous_context,
                    context_line,
                    context_radius,
                )
                action_events.append(pending.hit)
                pending_hits.append(pending)

            match = LEVEL_RE.match(line)
            if match:
                level = match.group("level")
                target = match.group("target")
                message = match.group("msg")
                cls = classify(level, target, message)
                key = (
                    f"{cls}|{normalize(message)}"
                    if cls.startswith(("warn:", "error:"))
                    else cls
                )
                pending = make_hit(
                    path,
                    line_no,
                    line,
                    previous_context,
                    context_line,
                    context_radius,
                )
                added_example = groups.setdefault(key, Group(key=key, level=level)).add(
                    path,
                    line_no,
                    line[:1000],
                    pending.hit.context,
                    max_examples,
                )
                if added_example:
                    pending_hits.append(pending)
            elif ERROR_RE.search(line):
                key = f"unstructured|{normalize(line)}"
                pending = make_hit(
                    path,
                    line_no,
                    line,
                    previous_context,
                    context_line,
                    context_radius,
                )
                added_example = groups.setdefault(key, Group(key=key, level="UNSTRUCTURED")).add(
                    path,
                    line_no,
                    line[:1000],
                    pending.hit.context,
                    max_examples,
                )
                if added_example:
                    pending_hits.append(pending)

            previous_context.append(context_line)

    sorted_groups = sorted(groups.values(), key=lambda group: group.count, reverse=True)
    return {
        "log_root": str(log_root),
        "since_mtime": since_mtime,
        "since_iso": datetime.fromtimestamp(since_mtime, timezone.utc).isoformat()
        if since_mtime
        else None,
        "files_seen": files_seen,
        "total_lines_scanned": total_lines,
        "groups": [
            {
                "key": group.key,
                "level": group.level,
                "count": group.count,
                "files": sorted(group.files),
                "examples": [hit.__dict__ for hit in group.examples],
            }
            for group in sorted_groups
        ],
        "manual_events": [hit.__dict__ for hit in manual_events],
        "action_events": [hit.__dict__ for hit in action_events],
    }


def format_text(report: dict, top: int) -> str:
    lines: list[str] = []
    lines.append(f"Log root: {report['log_root']}")
    lines.append(f"Since: {report['since_iso'] or 'beginning'}")
    lines.append(f"Files scanned: {len(report['files_seen'])}")
    lines.append(f"Lines scanned: {report['total_lines_scanned']}")
    lines.append("")
    lines.append("Top error/warning groups:")
    for group in report["groups"][:top]:
        lines.append(
            f"- [{group['level']}] {group['key']} count={group['count']} files={len(group['files'])}"
        )
        for example in group["examples"][:2]:
            lines.append(f"  - {example['file']}:{example['line']}: {example['text']}")
    lines.append("")
    lines.append("Manual operation markers:")
    if report["manual_events"]:
        for hit in report["manual_events"][:top]:
            lines.append(f"- {hit['file']}:{hit['line']}: {hit['text']}")
    else:
        lines.append("- none found in scanned logs")
    lines.append("")
    lines.append("Action timeline markers:")
    for hit in report["action_events"][:top]:
        lines.append(f"- {hit['file']}:{hit['line']}: {hit['text']}")
    lines.append("")
    lines.append("Recommended follow-up queries:")
    lines.append("- Search a suspected change_id with: --change-id <id>")
    lines.append(
        "- For manual retry causality, inspect manual_events before ResolveStarted / Merge resolution attempt lines."
    )
    lines.append(
        "- Treat high-count stale-refresh-snapshot groups as log noise unless investigating TUI refresh itself."
    )
    return "\n".join(lines)


def filter_change_id(report: dict, change_id: str) -> dict:
    def keep_hit(hit: dict) -> bool:
        blob = "\n".join([hit.get("text", ""), *hit.get("context", [])])
        return change_id in blob

    report = dict(report)
    report["groups"] = [
        {
            **group,
            "examples": [hit for hit in group["examples"] if keep_hit(hit)],
        }
        for group in report["groups"]
    ]
    report["groups"] = [
        group
        for group in report["groups"]
        if group["examples"] or change_id in group["key"]
    ]
    report["manual_events"] = [hit for hit in report["manual_events"] if keep_hit(hit)]
    report["action_events"] = [hit for hit in report["action_events"] if keep_hit(hit)]
    return report


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--log-root", default=os.path.expanduser("~/.local/state/cflx/logs")
    )
    parser.add_argument("--marker", default=".last-checked")
    parser.add_argument(
        "--since", type=float, default=None, help="Unix timestamp override"
    )
    parser.add_argument(
        "--change-id", default=None, help="Filter examples/timeline to a change id"
    )
    parser.add_argument("--format", choices=["text", "json"], default="text")
    parser.add_argument("--top", type=int, default=20)
    parser.add_argument("--max-examples", type=int, default=3)
    parser.add_argument("--context", type=int, default=2)
    args = parser.parse_args()

    log_root = Path(args.log_root).expanduser()
    since = (
        args.since if args.since is not None else marker_mtime(log_root, args.marker)
    )
    report = mine(log_root, since, args.max_examples, args.context)
    if args.change_id:
        report = filter_change_id(report, args.change_id)

    if args.format == "json":
        print(json.dumps(report, ensure_ascii=False, indent=2))
    else:
        print(format_text(report, args.top))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

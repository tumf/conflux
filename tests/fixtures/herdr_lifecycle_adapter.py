#!/usr/bin/env python3
"""Reference external lifecycle adapter for Herdr.

Conflux starts this script as a child process and writes newline-delimited JSON
lifecycle messages to its stdin (see `docs/guides/CONFIG.md`). The adapter is
observability-only: Conflux ignores its output and its exit status, so this
script must never try to influence the cflx workflow.

Herdr context is read from the **inherited process environment**, which is why
no `cflx` wrapper or PATH shim is needed:

  HERDR_ENV          "1" when running inside a Herdr pane
  HERDR_SOCKET_PATH  unix socket to report agent state to
  HERDR_PANE_ID      pane the cflx process is running in

Outside Herdr (`HERDR_ENV != "1"`) the adapter exits successfully without side
effects, so the same configuration is safe in every terminal.

NOTE: the translated payloads below are the integration *shape*, not a certified
Herdr wire format. Match the field names to the Herdr version you run before
using this in production; Herdr also still needs to recognize the foreground
`cflx` process in order to create and remove the Agent entry.
"""

import json
import os
import socket
import sys

# Protocol versions this adapter understands. Unknown versions are ignored
# rather than guessed at.
SUPPORTED_PROTOCOL_VERSIONS = {1}

# cflx semantic state -> Herdr agent status
STATE_TRANSLATION = {
    "idle": "idle",
    "working": "working",
    "blocked": "blocked",
}


def translate(message, pane_id):
    """Translate one cflx lifecycle message into a Herdr agent report."""
    if message.get("protocol_version") not in SUPPORTED_PROTOCOL_VERSIONS:
        return None

    kind = message.get("kind")

    if kind == "process_started":
        return {
            "type": "agent_attach",
            "pane_id": pane_id,
            "pid": message.get("pid"),
            "mode": message.get("mode"),
        }

    if kind == "state_changed":
        status = STATE_TRANSLATION.get(message.get("state"))
        if status is None:
            return None
        report = {"type": "agent_status", "pane_id": pane_id, "status": status}
        change_id = (message.get("context") or {}).get("change_id")
        if change_id:
            report["change_id"] = change_id
        return report

    if kind == "process_stopping":
        return {"type": "agent_detach", "pane_id": pane_id}

    # session_identified and future kinds carry no Herdr agent status.
    return None


def main():
    if os.environ.get("HERDR_ENV") != "1":
        # Not inside a Herdr pane: no-op.
        return 0

    socket_path = os.environ.get("HERDR_SOCKET_PATH")
    pane_id = os.environ.get("HERDR_PANE_ID")
    if not socket_path or not pane_id:
        # Herdr context is incomplete; stay a no-op rather than guessing.
        return 0

    try:
        connection = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        connection.connect(socket_path)
    except OSError:
        # Reporting is best effort; never fail the cflx process.
        return 0

    with connection:
        for line in sys.stdin:
            line = line.strip()
            if not line:
                continue
            try:
                message = json.loads(line)
            except json.JSONDecodeError:
                continue

            report = translate(message, pane_id)
            if report is None:
                continue

            try:
                connection.sendall((json.dumps(report) + "\n").encode("utf-8"))
            except OSError:
                break

    return 0


if __name__ == "__main__":
    sys.exit(main())

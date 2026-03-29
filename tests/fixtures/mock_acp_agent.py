#!/usr/bin/env python3
import json
import sys

session_created = False
pending_elicitation = {}


def send(message):
    sys.stdout.write(json.dumps(message) + "\n")
    sys.stdout.flush()


for raw in sys.stdin:
    raw = raw.strip()
    if not raw:
        continue

    try:
        msg = json.loads(raw)
    except json.JSONDecodeError:
        continue

    method = msg.get("method")
    msg_id = msg.get("id")

    if method == "initialize" and msg_id is not None:
        send(
            {
                "jsonrpc": "2.0",
                "id": msg_id,
                "result": {
                    "protocolVersion": 1,
                    "agentInfo": {"name": "mock-acp", "version": "1.0.0"},
                    "agentCapabilities": {"elicitation": {"form": {}}},
                },
            }
        )
        continue

    if method == "session/new" and msg_id is not None:
        session_created = True
        send(
            {"jsonrpc": "2.0", "id": msg_id, "result": {"sessionId": "mock-session-1"}}
        )
        continue

    if method == "session/prompt":
        params = msg.get("params") or {}
        session_id = params.get("sessionId", "mock-session-1")
        content = ((params.get("message") or {}).get("content")) or ""

        if not session_created:
            continue

        if content == "trigger-elicitation":
            request_id = "elicitation-1"
            pending_elicitation[request_id] = session_id
            send(
                {
                    "jsonrpc": "2.0",
                    "method": "session/elicitation",
                    "params": {
                        "sessionId": session_id,
                        "requestId": request_id,
                        "mode": "form",
                        "message": "Need approval",
                        "schema": {
                            "type": "object",
                            "properties": {
                                "answer": {"type": "string", "title": "Answer"}
                            },
                            "required": ["answer"],
                        },
                    },
                }
            )
            continue

        send(
            {
                "jsonrpc": "2.0",
                "method": "session/update",
                "params": {
                    "type": "agent_message_chunk",
                    "text": f"echo:{content}",
                },
            }
        )
        send(
            {
                "jsonrpc": "2.0",
                "method": "session/update",
                "params": {
                    "type": "turn_complete",
                    "stop_reason": "completed",
                },
            }
        )
        continue

    if msg_id is not None and isinstance(msg_id, str) and msg_id in pending_elicitation:
        result = msg.get("result") or {}
        content = result.get("content") or {}
        answer = content.get("answer", "")
        send(
            {
                "jsonrpc": "2.0",
                "method": "session/update",
                "params": {
                    "type": "agent_message_chunk",
                    "text": f"elicitation-accepted:{answer}",
                },
            }
        )
        send(
            {
                "jsonrpc": "2.0",
                "method": "session/update",
                "params": {
                    "type": "turn_complete",
                    "stop_reason": "completed",
                },
            }
        )
        del pending_elicitation[msg_id]
        continue

    if method == "session/cancel":
        send(
            {
                "jsonrpc": "2.0",
                "method": "session/update",
                "params": {
                    "type": "turn_complete",
                    "stop_reason": "cancelled",
                },
            }
        )

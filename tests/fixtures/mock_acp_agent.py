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

    if method == "session/prompt" and msg_id is not None:
        params = msg.get("params") or {}
        session_id = params.get("sessionId", "mock-session-1")
        prompt_blocks = params.get("prompt") or []
        content = ""
        for block in prompt_blocks:
            if block.get("type") == "text":
                content = block.get("text", "")
                break
        if not content:
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
                    "sessionId": session_id,
                    "update": {
                        "sessionUpdate": "agent_message_chunk",
                        "content": {"type": "text", "text": f"echo:{content}"},
                    },
                },
            }
        )
        send(
            {
                "jsonrpc": "2.0",
                "method": "session/update",
                "params": {
                    "sessionId": session_id,
                    "update": {
                        "sessionUpdate": "turn_complete",
                        "stopReason": "end_turn",
                    },
                },
            }
        )
        send({"jsonrpc": "2.0", "id": msg_id, "result": {"stopReason": "end_turn"}})
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
                    "sessionId": pending_elicitation[msg_id],
                    "update": {
                        "sessionUpdate": "agent_message_chunk",
                        "content": {
                            "type": "text",
                            "text": f"elicitation-accepted:{answer}",
                        },
                    },
                },
            }
        )
        send(
            {
                "jsonrpc": "2.0",
                "method": "session/update",
                "params": {
                    "sessionId": pending_elicitation[msg_id],
                    "update": {
                        "sessionUpdate": "turn_complete",
                        "stopReason": "end_turn",
                    },
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
                    "sessionId": "mock-session-1",
                    "update": {
                        "sessionUpdate": "turn_complete",
                        "stopReason": "cancelled",
                    },
                },
            }
        )

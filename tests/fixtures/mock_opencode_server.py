#!/usr/bin/env python3
import argparse
import json
import queue
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

STATE = {
    "sessions": {},
    "events": [],
    "event_cond": threading.Condition(),
}


def push_event(event_type, data):
    with STATE["event_cond"]:
        STATE["events"].append((event_type, data))
        STATE["event_cond"].notify_all()


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def _read_json(self):
        length = int(self.headers.get("Content-Length", "0"))
        raw = self.rfile.read(length) if length > 0 else b"{}"
        return json.loads(raw.decode("utf-8"))

    def _send_json(self, code, payload):
        body = json.dumps(payload).encode("utf-8")
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        if self.path == "/global/health":
            return self._send_json(200, {"healthy": True})

        if self.path.startswith("/session/") and self.path.endswith("/message"):
            session_id = self.path.split("/")[2]
            session = STATE["sessions"].get(session_id, {"messages": []})
            return self._send_json(200, session.get("messages", []))

        if self.path == "/event":
            self.send_response(200)
            self.send_header("Content-Type", "text/event-stream")
            self.send_header("Cache-Control", "no-cache")
            self.send_header("Connection", "keep-alive")
            self.end_headers()

            idx = 0
            while True:
                with STATE["event_cond"]:
                    while idx >= len(STATE["events"]):
                        STATE["event_cond"].wait(timeout=15)
                        if idx >= len(STATE["events"]):
                            # heartbeat comment
                            try:
                                self.wfile.write(b": keepalive\n\n")
                                self.wfile.flush()
                            except Exception:
                                return
                    event_type, data = STATE["events"][idx]
                    idx += 1

                payload = json.dumps(data)
                frame = f"event: {event_type}\ndata: {payload}\n\n".encode("utf-8")
                try:
                    self.wfile.write(frame)
                    self.wfile.flush()
                except Exception:
                    return
            return

        self.send_error(404)

    def do_POST(self):
        if self.path == "/session":
            body = self._read_json()
            sid = f"mock-session-{len(STATE['sessions']) + 1}"
            STATE["sessions"][sid] = {
                "id": sid,
                "title": body.get("title"),
                "messages": [],
                "status": "idle",
            }
            return self._send_json(200, {"id": sid, "title": body.get("title")})

        if self.path.startswith("/session/") and self.path.endswith("/prompt_async"):
            session_id = self.path.split("/")[2]
            body = self._read_json()
            text = body.get("text", "")
            session = STATE["sessions"].setdefault(session_id, {"messages": []})

            if text == "trigger-elicitation":
                push_event(
                    "message.part.updated",
                    {
                        "session_id": session_id,
                        "part": {
                            "type": "text",
                            "text": "elicitation-accepted:approved",
                        },
                    },
                )
                push_event(
                    "session.status",
                    {
                        "session_id": session_id,
                        "status": "completed",
                        "stop_reason": "end_turn",
                    },
                )
            else:
                push_event(
                    "message.part.updated",
                    {
                        "session_id": session_id,
                        "part": {
                            "type": "text",
                            "text": f"echo:{text}",
                        },
                    },
                )
                push_event(
                    "session.status",
                    {
                        "session_id": session_id,
                        "status": "completed",
                        "stop_reason": "end_turn",
                    },
                )

            session.setdefault("messages", []).append(
                {
                    "id": f"msg-{len(session['messages']) + 1}",
                    "role": "assistant",
                    "parts": [{"type": "text", "text": f"echo:{text}"}],
                }
            )
            self.send_response(204)
            self.send_header("Content-Length", "0")
            self.end_headers()
            return

        if self.path.startswith("/session/") and self.path.endswith("/abort"):
            session_id = self.path.split("/")[2]
            push_event(
                "session.status",
                {
                    "session_id": session_id,
                    "status": "cancelled",
                    "stop_reason": "cancelled",
                },
            )
            self.send_response(204)
            self.send_header("Content-Length", "0")
            self.end_headers()
            return

        self.send_error(404)

    def log_message(self, fmt, *args):
        pass


def main():
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="cmd")
    serve = sub.add_parser("serve")
    serve.add_argument("--port", default="0")
    serve.add_argument("--hostname", default="127.0.0.1")
    serve.add_argument("--print-logs", action="store_true")
    args = parser.parse_args()

    if args.cmd != "serve":
        raise SystemExit(2)

    port = int(args.port)
    httpd = ThreadingHTTPServer((args.hostname, port), Handler)
    actual_port = httpd.server_address[1]
    print(f"listening on http://{args.hostname}:{actual_port}", flush=True)

    try:
        httpd.serve_forever()
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    main()

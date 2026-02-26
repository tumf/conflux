//! Stream-JSON output textification for Claude Code `--output-format stream-json`.
//!
//! Converts NDJSON event lines emitted by Claude Code into human-readable text
//! with line-oriented buffering.  Each JSON event that carries text is decoded;
//! events that carry no text (tool calls, system init messages, …) are suppressed
//! when textify mode is active.  Plain non-JSON lines are passed through unchanged.

/// Returns `true` if `line` is a parseable JSON object that contains a `type`
/// field — i.e., it is a stream-json event line rather than plain text output.
///
/// Used by [`process_stdout_line`] to distinguish "recognisable but non-text
/// stream-json events" (which should be suppressed in textify mode) from
/// ordinary non-JSON output lines (which should pass through unchanged).
pub fn is_stream_json_event(line: &str) -> bool {
    let trimmed = line.trim();
    if !trimmed.starts_with('{') {
        return false;
    }
    let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) else {
        return false;
    };
    value.get("type").and_then(|t| t.as_str()).is_some()
}

/// Extract human-readable text from a single stream-json (NDJSON) line.
///
/// Returns `Some(text)` when the line is a recognized stream-json event that
/// carries human-readable text.  Returns `None` for:
/// - Non-JSON lines (plain text, empty strings, …)
/// - JSON objects whose `type` is not handled (e.g., `system`, `tool_use`)
/// - Recognized events that contain no text (e.g., empty text delta)
///
/// Supported event types
/// - `stream_event` with `event.delta.type = "text_delta"`: streaming text chunk
/// - `assistant` with `message.content[].type = "text"`: full assistant text block
/// - `result` with a non-empty, non-error `result` field: final result text
pub fn extract_text_from_stream_json(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if !trimmed.starts_with('{') {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(trimmed).ok()?;
    let event_type = value.get("type")?.as_str()?;

    match event_type {
        "stream_event" => extract_from_stream_event(&value),
        "assistant" => extract_from_assistant(&value),
        "result" => extract_from_result(&value),
        _ => None,
    }
}

fn extract_from_stream_event(value: &serde_json::Value) -> Option<String> {
    // {"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"..."}}}
    let delta = value.get("event")?.get("delta")?;
    if delta.get("type")?.as_str()? != "text_delta" {
        return None;
    }
    let text = delta.get("text")?.as_str()?;
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

fn extract_from_assistant(value: &serde_json::Value) -> Option<String> {
    // {"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"..."}]}}
    let content = value.get("message")?.get("content")?.as_array()?;
    let mut parts = Vec::new();
    for block in content {
        if block.get("type").and_then(|t| t.as_str()) == Some("text") {
            if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                if !text.is_empty() {
                    parts.push(text.to_string());
                }
            }
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(""))
    }
}

fn extract_from_result(value: &serde_json::Value) -> Option<String> {
    // {"type":"result","subtype":"success","result":"...","is_error":false}
    let is_error = value
        .get("is_error")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if is_error {
        return None;
    }
    let result = value.get("result")?.as_str()?;
    if result.is_empty() {
        None
    } else {
        Some(result.to_string())
    }
}

/// Line-oriented buffer for stream-json text output.
///
/// Text chunks emitted by streaming events are typically not newline-terminated.
/// This buffer accumulates chunks and emits complete lines only when a `\n` is
/// observed, retaining any incomplete trailing fragment for the next call.
#[derive(Default)]
pub struct StreamJsonTextBuffer {
    partial: String,
}

impl StreamJsonTextBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed a raw text chunk into the buffer.
    ///
    /// Returns zero or more complete lines (newline stripped).
    /// An incomplete trailing fragment is held in the buffer.
    pub fn feed(&mut self, text: &str) -> Vec<String> {
        self.partial.push_str(text);
        self.flush_lines()
    }

    /// Flush any remaining buffered text as a final (possibly incomplete) line.
    ///
    /// Should be called when the stream ends to emit any trailing fragment.
    /// Returns `None` if the buffer is empty.
    pub fn finalize(&mut self) -> Option<String> {
        if self.partial.is_empty() {
            None
        } else {
            Some(std::mem::take(&mut self.partial))
        }
    }

    fn flush_lines(&mut self) -> Vec<String> {
        let mut lines = Vec::new();
        while let Some(pos) = self.partial.find('\n') {
            let line = self.partial[..pos].to_string();
            self.partial = self.partial[pos + 1..].to_string();
            lines.push(line);
        }
        lines
    }
}

/// Process a single raw stdout line with stream-json textification.
///
/// - If the line is a recognized stream-json event that carries text, the text
///   is extracted and fed into `buffer`, returning any complete lines.
/// - If the line is a parseable stream-json event but carries no human-readable
///   text (e.g., `thinking`, `tool_use`, `system`), it is **suppressed**
///   (empty vec returned).
/// - If the line is not a stream-json event at all (plain text, non-JSON output),
///   it is returned unchanged as a single-element vec.
///
/// Call [`StreamJsonTextBuffer::finalize`] at stream end to flush any trailing
/// partial line remaining in the buffer.
pub fn process_stdout_line(line: &str, buffer: &mut StreamJsonTextBuffer) -> Vec<String> {
    match extract_text_from_stream_json(line) {
        Some(text) => buffer.feed(&text),
        None => {
            // Suppress parseable stream-json events that have no human-readable text.
            // Pass through non-JSON lines (plain command output, log lines, …).
            if is_stream_json_event(line) {
                vec![]
            } else {
                vec![line.to_string()]
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── extract_text_from_stream_json ─────────────────────────────────────────

    #[test]
    fn test_extract_stream_event_text_delta() {
        let line = r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"Hello "}}}"#;
        assert_eq!(
            extract_text_from_stream_json(line),
            Some("Hello ".to_string())
        );
    }

    #[test]
    fn test_extract_stream_event_empty_text() {
        let line = r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":""}}}"#;
        assert_eq!(extract_text_from_stream_json(line), None);
    }

    #[test]
    fn test_extract_stream_event_non_text_delta() {
        let line = r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"input_json_delta","partial_json":"{"}}}"#;
        assert_eq!(extract_text_from_stream_json(line), None);
    }

    #[test]
    fn test_extract_assistant_text_block() {
        let line = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Hello world"}]}}"#;
        assert_eq!(
            extract_text_from_stream_json(line),
            Some("Hello world".to_string())
        );
    }

    #[test]
    fn test_extract_assistant_multiple_text_blocks() {
        let line = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Hello "},{"type":"text","text":"world"}]}}"#;
        assert_eq!(
            extract_text_from_stream_json(line),
            Some("Hello world".to_string())
        );
    }

    #[test]
    fn test_extract_assistant_no_text_blocks() {
        let line = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","name":"bash","input":{}}]}}"#;
        assert_eq!(extract_text_from_stream_json(line), None);
    }

    #[test]
    fn test_extract_result_success() {
        let line = r#"{"type":"result","subtype":"success","result":"Done successfully","is_error":false}"#;
        assert_eq!(
            extract_text_from_stream_json(line),
            Some("Done successfully".to_string())
        );
    }

    #[test]
    fn test_extract_result_error_suppressed() {
        let line =
            r#"{"type":"result","subtype":"error","result":"Something failed","is_error":true}"#;
        assert_eq!(extract_text_from_stream_json(line), None);
    }

    #[test]
    fn test_extract_result_empty_suppressed() {
        let line = r#"{"type":"result","subtype":"success","result":"","is_error":false}"#;
        assert_eq!(extract_text_from_stream_json(line), None);
    }

    #[test]
    fn test_non_json_returns_none() {
        assert_eq!(extract_text_from_stream_json("plain text"), None);
        assert_eq!(extract_text_from_stream_json(""), None);
        assert_eq!(extract_text_from_stream_json("not json"), None);
    }

    #[test]
    fn test_unknown_type_returns_none() {
        let line = r#"{"type":"system","subtype":"init","session_id":"abc"}"#;
        assert_eq!(extract_text_from_stream_json(line), None);
    }

    // ── StreamJsonTextBuffer ──────────────────────────────────────────────────

    #[test]
    fn test_buffer_single_complete_line() {
        let mut buf = StreamJsonTextBuffer::new();
        let lines = buf.feed("Hello world\n");
        assert_eq!(lines, vec!["Hello world".to_string()]);
        assert_eq!(buf.finalize(), None);
    }

    #[test]
    fn test_buffer_partial_then_complete() {
        let mut buf = StreamJsonTextBuffer::new();
        assert!(buf.feed("Hello ").is_empty());
        assert_eq!(buf.feed("world\n"), vec!["Hello world".to_string()]);
    }

    #[test]
    fn test_buffer_multiple_lines_in_one_chunk() {
        let mut buf = StreamJsonTextBuffer::new();
        let lines = buf.feed("line1\nline2\nline3\n");
        assert_eq!(
            lines,
            vec![
                "line1".to_string(),
                "line2".to_string(),
                "line3".to_string()
            ]
        );
    }

    #[test]
    fn test_buffer_partial_retained_until_finalize() {
        let mut buf = StreamJsonTextBuffer::new();
        let lines = buf.feed("line1\npartial");
        assert_eq!(lines, vec!["line1".to_string()]);
        assert_eq!(buf.finalize(), Some("partial".to_string()));
    }

    #[test]
    fn test_buffer_finalize_empty() {
        let mut buf = StreamJsonTextBuffer::new();
        assert_eq!(buf.finalize(), None);
    }

    // ── process_stdout_line ───────────────────────────────────────────────────

    #[test]
    fn test_process_plain_text_passthrough() {
        let mut buf = StreamJsonTextBuffer::new();
        let result = process_stdout_line("plain text output", &mut buf);
        assert_eq!(result, vec!["plain text output".to_string()]);
    }

    #[test]
    fn test_process_stream_event_buffers_partial() {
        let mut buf = StreamJsonTextBuffer::new();
        let line = r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"Hello "}}}"#;
        assert!(process_stdout_line(line, &mut buf).is_empty());

        let line2 = r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"world\n"}}}"#;
        let result = process_stdout_line(line2, &mut buf);
        assert_eq!(result, vec!["Hello world".to_string()]);
    }

    #[test]
    fn test_process_assistant_multiline_split() {
        let mut buf = StreamJsonTextBuffer::new();
        let line = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"line1\nline2\nline3"}]}}"#;
        let result = process_stdout_line(line, &mut buf);
        // "line3" has no trailing newline → stays in buffer
        assert_eq!(result, vec!["line1".to_string(), "line2".to_string()]);
        assert_eq!(buf.finalize(), Some("line3".to_string()));
    }

    #[test]
    fn test_process_unrecognized_json_suppressed() {
        // Parseable stream-json events without text content must be suppressed.
        let mut buf = StreamJsonTextBuffer::new();
        let line = r#"{"type":"system","subtype":"init"}"#;
        let result = process_stdout_line(line, &mut buf);
        assert!(result.is_empty());
    }

    #[test]
    fn test_process_thinking_event_suppressed() {
        let mut buf = StreamJsonTextBuffer::new();
        let line = r#"{"type":"thinking","thinking":"some internal reasoning"}"#;
        assert!(process_stdout_line(line, &mut buf).is_empty());
    }

    #[test]
    fn test_process_tool_use_event_suppressed() {
        let mut buf = StreamJsonTextBuffer::new();
        let line = r#"{"type":"tool_use","name":"bash","input":{"command":"ls"}}"#;
        assert!(process_stdout_line(line, &mut buf).is_empty());
    }

    #[test]
    fn test_process_empty_line_passthrough() {
        let mut buf = StreamJsonTextBuffer::new();
        let result = process_stdout_line("", &mut buf);
        assert_eq!(result, vec!["".to_string()]);
    }

    // ── is_stream_json_event ──────────────────────────────────────────────────

    #[test]
    fn test_is_stream_json_event_true() {
        assert!(is_stream_json_event(r#"{"type":"system","subtype":"init"}"#));
        assert!(is_stream_json_event(r#"{"type":"tool_use","name":"bash"}"#));
        assert!(is_stream_json_event(r#"{"type":"thinking","thinking":"..."}"#));
        assert!(is_stream_json_event(r#"{"type":"stream_event","event":{}}"#));
    }

    #[test]
    fn test_is_stream_json_event_false() {
        assert!(!is_stream_json_event("plain text"));
        assert!(!is_stream_json_event(""));
        assert!(!is_stream_json_event(r#"{"no_type": true}"#));
        assert!(!is_stream_json_event("not json {"));
    }

    // ── multi-line log emission ───────────────────────────────────────────────

    /// Multi-line assistant content must be emitted as separate log lines.
    /// Each `\n` in the extracted text produces one output line; the last
    /// fragment (no trailing newline) is flushed via `finalize()`.
    #[test]
    fn test_multiline_assistant_emits_separate_log_lines() {
        let mut buf = StreamJsonTextBuffer::new();
        let event = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"First line\nSecond line\nThird line"}]}}"#;

        let mut all_lines = process_stdout_line(event, &mut buf);
        // "Third line" has no trailing newline → held in buffer
        assert_eq!(
            all_lines,
            vec!["First line".to_string(), "Second line".to_string()]
        );

        // Finalize flushes the remaining fragment as the third log line
        if let Some(tail) = buf.finalize() {
            all_lines.push(tail);
        }
        assert_eq!(
            all_lines,
            vec![
                "First line".to_string(),
                "Second line".to_string(),
                "Third line".to_string(),
            ]
        );
    }

    /// Streaming text_delta events that together form a multi-line message
    /// must each be emitted as a separate log line.
    #[test]
    fn test_streaming_deltas_multiline_emit_separate_log_lines() {
        let mut buf = StreamJsonTextBuffer::new();

        let delta1 = r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"Hello\n"}}}"#;
        let delta2 = r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"World\n"}}}"#;

        let lines1 = process_stdout_line(delta1, &mut buf);
        let lines2 = process_stdout_line(delta2, &mut buf);

        assert_eq!(lines1, vec!["Hello".to_string()]);
        assert_eq!(lines2, vec!["World".to_string()]);
        // Buffer should be empty after all newlines consumed
        assert_eq!(buf.finalize(), None);
    }
}

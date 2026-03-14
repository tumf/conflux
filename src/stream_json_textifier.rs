//! Stream-JSON output textification for Claude Code `--output-format stream-json`.
//!
//! Converts NDJSON event lines emitted by Claude Code into human-readable text
//! with line-oriented buffering.  Each JSON event that carries text is decoded;
//! tool-related events are converted to one-line summaries;
//! other non-text events (system init messages, thinking, …) are suppressed
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

/// Extract a one-line summary from tool-related stream-json events.
///
/// Returns `Some(summary)` for `tool_use` and `tool_result` events.
/// Returns `None` for all other event types.
///
/// Format:
/// - `tool_use`: `[tool_use:<name>] key=value ...`
/// - `tool_result`: `[tool_result:<name>] content (truncated if too long)`
pub fn extract_tool_summary_from_stream_json(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if !trimmed.starts_with('{') {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(trimmed).ok()?;
    let event_type = value.get("type")?.as_str()?;

    match event_type {
        "tool_use" => extract_tool_use_summary(&value),
        "tool_result" => extract_tool_result_summary(&value),
        "assistant" => extract_assistant_tool_summary(&value),
        _ => None,
    }
}

fn extract_tool_use_summary(value: &serde_json::Value) -> Option<String> {
    // {"type":"tool_use","name":"bash","id":"...","input":{...}}
    let name = value.get("name")?.as_str()?;
    let input = value.get("input")?;
    // Normalize tool name to lowercase for case-insensitive matching.
    let name_lower = name.to_ascii_lowercase();

    let mut parts = Vec::new();
    parts.push(format!("[tool_use:{}]", name));

    if let Some(obj) = input.as_object() {
        match name_lower.as_str() {
            "read" => {
                // File path (prefer filePath, fall back to path).
                let file_path = obj
                    .get("filePath")
                    .or_else(|| obj.get("file_path"))
                    .or_else(|| obj.get("path"))
                    .and_then(|v| v.as_str());
                if let Some(fp) = file_path {
                    parts.push(format!("filePath={}", truncate_string(fp, 100)));
                }
                // Optional offset/limit for partial reads.
                if let Some(offset) = obj.get("offset").and_then(|v| v.as_i64()) {
                    parts.push(format!("offset={}", offset));
                }
                if let Some(limit) = obj.get("limit").and_then(|v| v.as_i64()) {
                    parts.push(format!("limit={}", limit));
                }
            }
            "write" | "edit" | "multiedit" => {
                // File path.
                let file_path = obj
                    .get("filePath")
                    .or_else(|| obj.get("file_path"))
                    .or_else(|| obj.get("path"))
                    .and_then(|v| v.as_str());
                if let Some(fp) = file_path {
                    parts.push(format!("filePath={}", truncate_string(fp, 100)));
                }
                // Do NOT include raw body content (text/content/old_string/new_string).
                // Emit safe metadata: chars=<n> lines=<n> derived from body fields.
                let body_text = obj
                    .get("text")
                    .or_else(|| obj.get("content"))
                    .or_else(|| obj.get("new_string"))
                    .and_then(|v| v.as_str());
                if let Some(body) = body_text {
                    parts.push(format!("chars={}", body.len()));
                    parts.push(format!("lines={}", body.lines().count()));
                }
            }
            "grep" => {
                if let Some(pattern) = obj.get("pattern").and_then(|v| v.as_str()) {
                    parts.push(format!("pattern={}", truncate_string(pattern, 80)));
                }
                if let Some(path) = obj.get("path").and_then(|v| v.as_str()) {
                    parts.push(format!("path={}", truncate_string(path, 80)));
                }
                if let Some(glob) = obj.get("glob").and_then(|v| v.as_str()) {
                    parts.push(format!("glob={}", truncate_string(glob, 80)));
                }
                if let Some(mode) = obj.get("output_mode").and_then(|v| v.as_str()) {
                    parts.push(format!("output_mode={}", mode));
                }
            }
            "glob" => {
                if let Some(pattern) = obj.get("pattern").and_then(|v| v.as_str()) {
                    parts.push(format!("pattern={}", truncate_string(pattern, 80)));
                }
                if let Some(path) = obj.get("path").and_then(|v| v.as_str()) {
                    parts.push(format!("path={}", truncate_string(path, 80)));
                }
            }
            "todowrite" => {
                if let Some(todos) = obj.get("todos").and_then(|v| v.as_array()) {
                    let total = todos.len();
                    let done = todos
                        .iter()
                        .filter(|t| {
                            t.get("status").and_then(|s| s.as_str()) == Some("completed")
                        })
                        .count();
                    let in_progress = todos
                        .iter()
                        .filter(|t| {
                            t.get("status").and_then(|s| s.as_str()) == Some("in_progress")
                        })
                        .count();
                    parts.push(format!("todos={}", total));
                    if done > 0 {
                        parts.push(format!("completed={}", done));
                    }
                    if in_progress > 0 {
                        parts.push(format!("in_progress={}", in_progress));
                    }
                }
            }
            "webfetch" => {
                if let Some(url) = obj.get("url").and_then(|v| v.as_str()) {
                    parts.push(format!("url={}", truncate_string(url, 100)));
                }
                if let Some(prompt) = obj.get("prompt").and_then(|v| v.as_str()) {
                    parts.push(format!("prompt={}", truncate_string(prompt, 60)));
                }
            }
            _ => {
                // Generic bounded fallback for unknown tools: extract a fixed set of
                // safe scalar fields so logs remain informative without leaking payloads.
                let key_fields = [
                    "command",
                    "url",
                    "path",
                    "query",
                    "selector",
                    "text",
                    "filePath",
                    "pattern",
                    "description",
                ];
                for key in &key_fields {
                    if let Some(val) = obj.get(*key) {
                        let val_str = match val {
                            serde_json::Value::String(s) => s.clone(),
                            _ => val.to_string(),
                        };
                        let truncated = truncate_string(&val_str, 100);
                        parts.push(format!("{}={}", key, truncated));
                    }
                }
            }
        }
    }

    Some(parts.join(" "))
}

fn extract_tool_result_summary(value: &serde_json::Value) -> Option<String> {
    // {"type":"tool_result","tool_use_id":"...","content":"..."}
    let tool_use_id = value.get("tool_use_id").and_then(|v| v.as_str());
    let content = value.get("content");

    let mut parts = Vec::new();
    if let Some(id) = tool_use_id {
        parts.push(format!("[tool_result:{}]", id));
    } else {
        parts.push("[tool_result]".to_string());
    }

    if let Some(content_val) = content {
        let content_str = match content_val {
            serde_json::Value::String(s) => s.clone(),
            _ => content_val.to_string(),
        };
        let truncated = truncate_string(&content_str, 200);
        parts.push(truncated);
    }

    Some(parts.join(" "))
}

fn extract_assistant_tool_summary(value: &serde_json::Value) -> Option<String> {
    // {"type":"assistant","message":{"content":[{"type":"tool_use",...}]}}
    let content = value.get("message")?.get("content")?.as_array()?;

    for block in content {
        if block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
            return extract_tool_use_summary(block);
        }
    }
    None
}

fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
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
/// - If the line is a tool-related event (`tool_use`, `tool_result`), a one-line
///   summary is returned instead of the raw JSON.
/// - If the line is a parseable stream-json event but carries no human-readable
///   text or summary (e.g., `thinking`, `system`), it is **suppressed**
///   (empty vec returned).
/// - If the line is not a stream-json event at all (plain text, non-JSON output),
///   it is returned unchanged as a single-element vec.
///
/// Call [`StreamJsonTextBuffer::finalize`] at stream end to flush any trailing
/// partial line remaining in the buffer.
pub fn process_stdout_line(line: &str, buffer: &mut StreamJsonTextBuffer) -> Vec<String> {
    // First, try to extract text (for text_delta, assistant text, result text)
    if let Some(text) = extract_text_from_stream_json(line) {
        return buffer.feed(&text);
    }

    // Second, try to extract tool summary (for tool_use, tool_result)
    if let Some(summary) = extract_tool_summary_from_stream_json(line) {
        return vec![summary];
    }

    // Finally, suppress other stream-json events or pass through non-JSON lines
    if is_stream_json_event(line) {
        vec![]
    } else {
        vec![line.to_string()]
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
    fn test_process_tool_use_event_emits_summary() {
        let mut buf = StreamJsonTextBuffer::new();
        let line = r#"{"type":"tool_use","name":"bash","input":{"command":"ls"}}"#;
        let result = process_stdout_line(line, &mut buf);
        assert_eq!(result.len(), 1);
        assert!(result[0].starts_with("[tool_use:bash]"));
        assert!(result[0].contains("command=ls"));
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
        assert!(is_stream_json_event(
            r#"{"type":"system","subtype":"init"}"#
        ));
        assert!(is_stream_json_event(r#"{"type":"tool_use","name":"bash"}"#));
        assert!(is_stream_json_event(
            r#"{"type":"thinking","thinking":"..."}"#
        ));
        assert!(is_stream_json_event(
            r#"{"type":"stream_event","event":{}}"#
        ));
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

    // ── extract_tool_summary tests ────────────────────────────────────────────

    #[test]
    fn test_extract_tool_use_summary_bash() {
        let line =
            r#"{"type":"tool_use","name":"bash","id":"tool_123","input":{"command":"ls -la"}}"#;
        let summary = extract_tool_summary_from_stream_json(line);
        assert!(summary.is_some());
        let s = summary.unwrap();
        assert!(s.starts_with("[tool_use:bash]"));
        assert!(s.contains("command=ls -la"));
    }

    #[test]
    fn test_extract_tool_use_summary_read() {
        let line = r#"{"type":"tool_use","name":"read","input":{"filePath":"/path/to/file.txt"}}"#;
        let summary = extract_tool_summary_from_stream_json(line);
        assert!(summary.is_some());
        let s = summary.unwrap();
        assert!(s.starts_with("[tool_use:read]"));
        assert!(s.contains("filePath=/path/to/file.txt"));
    }

    #[test]
    fn test_extract_tool_use_summary_truncates_long_values() {
        let long_cmd = "a".repeat(150);
        let line = format!(
            r#"{{"type":"tool_use","name":"bash","input":{{"command":"{}"}}}}"#,
            long_cmd
        );
        let summary = extract_tool_summary_from_stream_json(&line);
        assert!(summary.is_some());
        let s = summary.unwrap();
        assert!(s.len() < line.len());
        assert!(s.contains("..."));
    }

    #[test]
    fn test_extract_tool_result_summary() {
        let line =
            r#"{"type":"tool_result","tool_use_id":"tool_123","content":"Success: file created"}"#;
        let summary = extract_tool_summary_from_stream_json(line);
        assert!(summary.is_some());
        let s = summary.unwrap();
        assert!(s.starts_with("[tool_result:tool_123]"));
        assert!(s.contains("Success: file created"));
    }

    #[test]
    fn test_extract_tool_result_truncates_long_content() {
        let long_content = "x".repeat(300);
        let line = format!(
            r#"{{"type":"tool_result","tool_use_id":"tool_456","content":"{}"}}"#,
            long_content
        );
        let summary = extract_tool_summary_from_stream_json(&line);
        assert!(summary.is_some());
        let s = summary.unwrap();
        assert!(s.len() < line.len());
        assert!(s.contains("..."));
    }

    #[test]
    fn test_extract_assistant_tool_use_summary() {
        let line = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","name":"grep","input":{"pattern":"error"}}]}}"#;
        let summary = extract_tool_summary_from_stream_json(line);
        assert!(summary.is_some());
        let s = summary.unwrap();
        assert!(s.starts_with("[tool_use:grep]"));
        assert!(s.contains("pattern=error"));
    }

    #[test]
    fn test_extract_tool_summary_non_tool_event_returns_none() {
        let line = r#"{"type":"thinking","thinking":"some reasoning"}"#;
        assert_eq!(extract_tool_summary_from_stream_json(line), None);

        let line2 = r#"{"type":"system","subtype":"init"}"#;
        assert_eq!(extract_tool_summary_from_stream_json(line2), None);
    }

    // ── file tool (read/write/edit) path and body rules ───────────────────────

    #[test]
    fn test_read_tool_includes_filepath() {
        let line = r#"{"type":"tool_use","name":"read","input":{"filePath":"/src/main.rs"}}"#;
        let summary = extract_tool_summary_from_stream_json(line).unwrap();
        assert!(summary.starts_with("[tool_use:read]"));
        assert!(summary.contains("filePath=/src/main.rs"));
    }

    #[test]
    fn test_read_tool_uses_path_alias_when_no_filepath() {
        let line = r#"{"type":"tool_use","name":"read","input":{"path":"/etc/config.toml"}}"#;
        let summary = extract_tool_summary_from_stream_json(line).unwrap();
        assert!(summary.contains("filePath=/etc/config.toml"));
    }

    #[test]
    fn test_write_tool_includes_filepath_no_body() {
        // Use serde_json to build valid JSON with arbitrary body text.
        let body = "fn main() { println!(\"hello\"); }";
        let json_val = serde_json::json!({
            "type": "tool_use",
            "name": "write",
            "input": {
                "filePath": "/out/foo.rs",
                "text": body
            }
        });
        let line = json_val.to_string();
        let summary = extract_tool_summary_from_stream_json(&line).unwrap();
        assert!(summary.starts_with("[tool_use:write]"));
        assert!(summary.contains("filePath=/out/foo.rs"));
        // Body text must NOT appear in the summary
        assert!(!summary.contains("fn main"));
        assert!(!summary.contains("text="));
    }

    #[test]
    fn test_write_tool_includes_chars_and_lines_metadata() {
        let body = "line one\nline two\nline three";
        let json_val = serde_json::json!({
            "type": "tool_use",
            "name": "write",
            "input": {
                "filePath": "/out/foo.rs",
                "text": body
            }
        });
        let line = json_val.to_string();
        let summary = extract_tool_summary_from_stream_json(&line).unwrap();
        assert!(summary.contains(&format!("chars={}", body.len())));
        assert!(summary.contains("lines=3"));
    }

    #[test]
    fn test_edit_tool_includes_filepath_no_old_new_string() {
        let line = r#"{"type":"tool_use","name":"edit","input":{"filePath":"/src/lib.rs","old_string":"foo","new_string":"bar baz qux"}}"#;
        let summary = extract_tool_summary_from_stream_json(line).unwrap();
        assert!(summary.starts_with("[tool_use:edit]"));
        assert!(summary.contains("filePath=/src/lib.rs"));
        // Raw body strings must not appear
        assert!(!summary.contains("old_string="));
        assert!(!summary.contains("new_string="));
        assert!(!summary.contains("bar baz qux"));
    }

    #[test]
    fn test_edit_tool_new_string_body_shows_metadata() {
        let new_string = "replacement content here";
        let line = format!(
            r#"{{"type":"tool_use","name":"edit","input":{{"filePath":"/f.rs","old_string":"x","new_string":"{}"}}}}"#,
            new_string
        );
        let summary = extract_tool_summary_from_stream_json(&line).unwrap();
        assert!(summary.contains(&format!("chars={}", new_string.len())));
    }

    #[test]
    fn test_non_file_tool_still_includes_text_field() {
        // Ensure non-file tools (e.g. a hypothetical "search" tool) still show text=...
        let line =
            r#"{"type":"tool_use","name":"bash","input":{"command":"echo hi","text":"some info"}}"#;
        let summary = extract_tool_summary_from_stream_json(line).unwrap();
        assert!(summary.contains("text=some info"));
    }

    #[test]
    fn test_process_tool_use_summary_does_not_use_buffer() {
        // Tool summaries are returned immediately, not buffered
        let mut buf = StreamJsonTextBuffer::new();
        let line = r#"{"type":"tool_use","name":"bash","input":{"command":"echo hello"}}"#;
        let result = process_stdout_line(line, &mut buf);
        assert_eq!(result.len(), 1);
        assert!(result[0].starts_with("[tool_use:bash]"));
        // Buffer should remain empty
        assert_eq!(buf.finalize(), None);
    }

    #[test]
    fn test_process_tool_result_summary() {
        let mut buf = StreamJsonTextBuffer::new();
        let line = r#"{"type":"tool_result","tool_use_id":"tool_789","content":"File not found"}"#;
        let result = process_stdout_line(line, &mut buf);
        assert_eq!(result.len(), 1);
        assert!(result[0].starts_with("[tool_result:tool_789]"));
        assert!(result[0].contains("File not found"));
    }

    // ── non-Bash dedicated tool formatters ────────────────────────────────────

    #[test]
    fn test_glob_tool_shows_pattern_and_path() {
        let line =
            r#"{"type":"tool_use","name":"glob","input":{"pattern":"**/*.rs","path":"/src"}}"#;
        let summary = extract_tool_summary_from_stream_json(line).unwrap();
        assert!(summary.starts_with("[tool_use:glob]"));
        assert!(summary.contains("pattern=**/*.rs"));
        assert!(summary.contains("path=/src"));
    }

    #[test]
    fn test_glob_tool_without_path() {
        let line = r#"{"type":"tool_use","name":"glob","input":{"pattern":"*.toml"}}"#;
        let summary = extract_tool_summary_from_stream_json(line).unwrap();
        assert!(summary.starts_with("[tool_use:glob]"));
        assert!(summary.contains("pattern=*.toml"));
        assert!(!summary.contains("path="));
    }

    #[test]
    fn test_grep_tool_shows_pattern_path_and_glob() {
        let line = r#"{"type":"tool_use","name":"grep","input":{"pattern":"fn main","path":"/src","glob":"*.rs","output_mode":"files_with_matches"}}"#;
        let summary = extract_tool_summary_from_stream_json(line).unwrap();
        assert!(summary.starts_with("[tool_use:grep]"));
        assert!(summary.contains("pattern=fn main"));
        assert!(summary.contains("path=/src"));
        assert!(summary.contains("glob=*.rs"));
        assert!(summary.contains("output_mode=files_with_matches"));
    }

    #[test]
    fn test_todowrite_tool_shows_todo_counts() {
        let line = r#"{"type":"tool_use","name":"todowrite","input":{"todos":[{"content":"Task A","status":"completed"},{"content":"Task B","status":"in_progress"},{"content":"Task C","status":"pending"}]}}"#;
        let summary = extract_tool_summary_from_stream_json(line).unwrap();
        assert!(summary.starts_with("[tool_use:todowrite]"));
        assert!(summary.contains("todos=3"));
        assert!(summary.contains("completed=1"));
        assert!(summary.contains("in_progress=1"));
    }

    #[test]
    fn test_todowrite_no_completed_or_inprogress_omits_those_fields() {
        let line = r#"{"type":"tool_use","name":"todowrite","input":{"todos":[{"content":"Task A","status":"pending"},{"content":"Task B","status":"pending"}]}}"#;
        let summary = extract_tool_summary_from_stream_json(line).unwrap();
        assert!(summary.contains("todos=2"));
        assert!(!summary.contains("completed="));
        assert!(!summary.contains("in_progress="));
    }

    #[test]
    fn test_webfetch_tool_shows_url_and_prompt() {
        let line = r#"{"type":"tool_use","name":"webfetch","input":{"url":"https://example.com/docs","prompt":"Extract API endpoints"}}"#;
        let summary = extract_tool_summary_from_stream_json(line).unwrap();
        assert!(summary.starts_with("[tool_use:webfetch]"));
        assert!(summary.contains("url=https://example.com/docs"));
        assert!(summary.contains("prompt=Extract API endpoints"));
    }

    #[test]
    fn test_read_tool_includes_offset_and_limit() {
        let line = r#"{"type":"tool_use","name":"read","input":{"filePath":"/src/main.rs","offset":100,"limit":50}}"#;
        let summary = extract_tool_summary_from_stream_json(line).unwrap();
        assert!(summary.starts_with("[tool_use:read]"));
        assert!(summary.contains("filePath=/src/main.rs"));
        assert!(summary.contains("offset=100"));
        assert!(summary.contains("limit=50"));
    }

    // ── case-insensitive tool name matching ───────────────────────────────────

    #[test]
    fn test_read_tool_mixed_case_matches_dedicated_formatter() {
        let line = r#"{"type":"tool_use","name":"Read","input":{"filePath":"/src/lib.rs"}}"#;
        let summary = extract_tool_summary_from_stream_json(line).unwrap();
        // Header preserves original casing; formatter still applies.
        assert!(summary.starts_with("[tool_use:Read]"));
        assert!(summary.contains("filePath=/src/lib.rs"));
        // Must not leak any raw body (nothing to leak here, but ensure path shows up).
        assert!(summary.contains("filePath="));
    }

    #[test]
    fn test_grep_tool_uppercase_matches_dedicated_formatter() {
        let line = r#"{"type":"tool_use","name":"Grep","input":{"pattern":"TODO","path":"/src"}}"#;
        let summary = extract_tool_summary_from_stream_json(line).unwrap();
        assert!(summary.starts_with("[tool_use:Grep]"));
        assert!(summary.contains("pattern=TODO"));
        assert!(summary.contains("path=/src"));
    }

    #[test]
    fn test_todowrite_mixed_case_matches_dedicated_formatter() {
        let line = r#"{"type":"tool_use","name":"TodoWrite","input":{"todos":[{"content":"X","status":"completed"}]}}"#;
        let summary = extract_tool_summary_from_stream_json(line).unwrap();
        assert!(summary.starts_with("[tool_use:TodoWrite]"));
        assert!(summary.contains("todos=1"));
        assert!(summary.contains("completed=1"));
    }

    #[test]
    fn test_bash_uppercase_uses_generic_fallback() {
        let line = r#"{"type":"tool_use","name":"Bash","input":{"command":"ls -la"}}"#;
        let summary = extract_tool_summary_from_stream_json(line).unwrap();
        assert!(summary.starts_with("[tool_use:Bash]"));
        assert!(summary.contains("command=ls -la"));
    }

    #[test]
    fn test_unknown_tool_uses_generic_fallback_with_safe_fields() {
        let line = r#"{"type":"tool_use","name":"UnknownTool","input":{"query":"search term","description":"some tool","secret_body":"sensitive data"}}"#;
        let summary = extract_tool_summary_from_stream_json(line).unwrap();
        assert!(summary.starts_with("[tool_use:UnknownTool]"));
        assert!(summary.contains("query=search term"));
        assert!(summary.contains("description=some tool"));
        // Fields not in the safe list must not appear.
        assert!(!summary.contains("secret_body"));
        assert!(!summary.contains("sensitive data"));
    }

    #[test]
    fn test_write_tool_body_safety_omits_raw_content() {
        let line = r#"{"type":"tool_use","name":"write","input":{"filePath":"/out/file.txt","content":"secret content here"}}"#;
        let summary = extract_tool_summary_from_stream_json(line).unwrap();
        assert!(summary.starts_with("[tool_use:write]"));
        assert!(summary.contains("filePath=/out/file.txt"));
        // Raw body must not appear; only safe metadata.
        assert!(!summary.contains("secret content here"));
        assert!(!summary.contains("content="));
        assert!(summary.contains("chars="));
        assert!(summary.contains("lines="));
    }
}

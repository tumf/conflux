//! User-level TUI configuration.
//!
//! This module intentionally keeps UI preferences separate from
//! `OrchestratorConfig`: TUI keybindings are client-local input mappings, not
//! authoritative workflow-control state.

use crate::config::jsonc;
use crate::error::{OrchestratorError, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::Deserialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tracing::{debug, info};

const TUI_CONFIG_FILE: &str = "tui.jsonc";
const GLOBAL_CONFIG_DIR: &str = "cflx";
const DEFAULT_START_KEYS: &[&str] = &["F5", "!"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TuiConfig {
    pub keybindings: TuiKeybindings,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TuiKeybindings {
    pub start: Vec<TuiKeyBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TuiKeyBinding {
    code: KeyCode,
    modifiers: KeyModifiers,
    label: String,
    normalized: String,
}

impl TuiKeyBinding {
    pub fn label(&self) -> &str {
        &self.label
    }

    fn matches(&self, key: &KeyEvent) -> bool {
        self.code == key.code && self.modifiers == key.modifiers
    }
}

impl TuiKeybindings {
    pub fn default_start() -> Self {
        Self {
            start: DEFAULT_START_KEYS
                .iter()
                .enumerate()
                .map(|(index, key)| {
                    parse_key_binding(key, &format!("keybindings.start[{index}]"))
                        .expect("built-in default start keys must remain valid TUI key bindings")
                })
                .collect(),
        }
    }

    pub fn start_label(&self) -> String {
        self.start
            .iter()
            .map(|binding| binding.label())
            .collect::<Vec<_>>()
            .join("/")
    }

    pub fn matches_start_key(&self, key: &KeyEvent) -> bool {
        self.start.iter().any(|binding| binding.matches(key))
    }
}

impl Default for TuiConfig {
    fn default() -> Self {
        Self {
            keybindings: TuiKeybindings::default_start(),
        }
    }
}

impl TuiConfig {
    pub fn start_key_label(&self) -> String {
        self.keybindings.start_label()
    }

    pub fn matches_start_key(&self, key: &KeyEvent) -> bool {
        self.keybindings.matches_start_key(key)
    }

    pub fn parse_jsonc(content: &str, source: &Path) -> Result<Self> {
        let raw: RawTuiConfig = jsonc::parse(content).map_err(|err| match err {
            OrchestratorError::ConfigParse(msg) => OrchestratorError::ConfigParse(format!(
                "Failed to parse TUI config file {}: {}",
                source.display(),
                msg
            )),
            other => other,
        })?;
        Self::from_raw(raw, source)
    }

    pub fn load_from_file(path: &Path) -> Result<Self> {
        debug!(path = %path.display(), "Loading TUI config file");
        let content = std::fs::read_to_string(path).map_err(|err| {
            OrchestratorError::ConfigLoad(format!(
                "Failed to read TUI config file {}: {}",
                path.display(),
                err
            ))
        })?;
        Self::parse_jsonc(&content, path)
    }

    /// Load and merge user-level TUI config candidates in low → high priority.
    ///
    /// Missing files are ignored. Existing files are parsed and merged over the
    /// default config. Project `.cflx.jsonc` is intentionally never consulted.
    pub fn load_user_config() -> Result<Self> {
        let mut config = Self::default();
        let mut loaded_paths = Vec::new();

        for path in tui_config_candidate_paths() {
            if path.exists() {
                let loaded = Self::load_from_file(&path)?;
                config.merge(loaded);
                loaded_paths.push(path);
            }
        }

        info!(
            loaded_count = loaded_paths.len(),
            start_keys = %config.start_key_label(),
            "Resolved TUI user configuration"
        );
        Ok(config)
    }

    fn from_raw(raw: RawTuiConfig, source: &Path) -> Result<Self> {
        let mut config = Self::default();
        if let Some(keybindings) = raw.keybindings {
            config.keybindings = TuiKeybindings::from_raw(keybindings, source)?;
        }
        Ok(config)
    }

    fn merge(&mut self, other: Self) {
        self.keybindings = other.keybindings;
    }
}

#[derive(Debug, Deserialize)]
struct RawTuiConfig {
    keybindings: Option<RawTuiKeybindings>,
}

#[derive(Debug, Deserialize)]
struct RawTuiKeybindings {
    start: Option<Vec<String>>,
}

impl TuiKeybindings {
    fn from_raw(raw: RawTuiKeybindings, source: &Path) -> Result<Self> {
        let Some(start) = raw.start else {
            return Ok(Self::default_start());
        };

        if start.is_empty() {
            return Err(tui_config_error(
                source,
                "keybindings.start must contain at least one key binding",
            ));
        }

        let mut seen = HashSet::new();
        let mut parsed = Vec::with_capacity(start.len());
        for (index, key_name) in start.iter().enumerate() {
            let field = format!("keybindings.start[{index}]");
            let binding = parse_key_binding(key_name, &field)
                .map_err(|message| tui_config_error(source, message))?;
            if !seen.insert(binding.normalized.clone()) {
                return Err(tui_config_error(
                    source,
                    format!(
                        "keybindings.start contains duplicate key binding '{}' normalized as '{}'",
                        key_name, binding.label
                    ),
                ));
            }
            parsed.push(binding);
        }

        Ok(Self { start: parsed })
    }
}

fn tui_config_error(source: &Path, message: impl Into<String>) -> OrchestratorError {
    OrchestratorError::ConfigParse(format!(
        "Invalid TUI config {}: {}",
        source.display(),
        message.into()
    ))
}

fn parse_key_binding(input: &str, field: &str) -> std::result::Result<TuiKeyBinding, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(format!("{field} must not be empty"));
    }

    if trimmed.contains('+') {
        return Err(format!(
            "{field} uses unsupported modifier syntax '{}'; modifier chords such as Ctrl+R are not supported in keybindings.start",
            input
        ));
    }

    let normalized = trimmed.to_ascii_lowercase();
    let (code, label, normalized) = match normalized.as_str() {
        "esc" | "escape" => (KeyCode::Esc, "Esc".to_string(), "esc".to_string()),
        "enter" | "return" => (KeyCode::Enter, "Enter".to_string(), "enter".to_string()),
        "space" => (KeyCode::Char(' '), "Space".to_string(), "space".to_string()),
        "tab" => (KeyCode::Tab, "Tab".to_string(), "tab".to_string()),
        "backspace" => (
            KeyCode::Backspace,
            "Backspace".to_string(),
            "backspace".to_string(),
        ),
        "delete" | "del" => (KeyCode::Delete, "Delete".to_string(), "delete".to_string()),
        "insert" | "ins" => (KeyCode::Insert, "Insert".to_string(), "insert".to_string()),
        "pageup" | "page_up" | "pgup" => {
            (KeyCode::PageUp, "PageUp".to_string(), "pageup".to_string())
        }
        "pagedown" | "page_down" | "pgdn" => (
            KeyCode::PageDown,
            "PageDown".to_string(),
            "pagedown".to_string(),
        ),
        "home" => (KeyCode::Home, "Home".to_string(), "home".to_string()),
        "end" => (KeyCode::End, "End".to_string(), "end".to_string()),
        "up" | "arrowup" => (KeyCode::Up, "Up".to_string(), "up".to_string()),
        "down" | "arrowdown" => (KeyCode::Down, "Down".to_string(), "down".to_string()),
        "left" | "arrowleft" => (KeyCode::Left, "Left".to_string(), "left".to_string()),
        "right" | "arrowright" => (KeyCode::Right, "Right".to_string(), "right".to_string()),
        name if name.len() >= 2 && name.starts_with('f') => {
            let number = name[1..]
                .parse::<u8>()
                .map_err(|_| unknown_key_message(field, input))?;
            if !(1..=12).contains(&number) {
                return Err(format!(
                    "{field} has unsupported function key '{}'; supported range is F1-F12",
                    input
                ));
            }
            (
                KeyCode::F(number),
                format!("F{number}"),
                format!("f{number}"),
            )
        }
        _ => {
            let mut chars = trimmed.chars();
            match (chars.next(), chars.next()) {
                (Some(ch), None) => {
                    let normalized_char = ch.to_ascii_lowercase();
                    (
                        KeyCode::Char(normalized_char),
                        normalized_char.to_string(),
                        format!("char:{normalized_char}"),
                    )
                }
                _ => return Err(unknown_key_message(field, input)),
            }
        }
    };

    Ok(TuiKeyBinding {
        code,
        modifiers: KeyModifiers::NONE,
        label,
        normalized,
    })
}

fn unknown_key_message(field: &str, input: &str) -> String {
    format!(
        "{field} has unknown key '{}'; supported keys include F1-F12, Esc, Enter, Space, Tab, PageUp, PageDown, Home, End, arrows, and single-character keys",
        input
    )
}

pub fn tui_config_candidate_paths() -> Vec<PathBuf> {
    candidate_paths_from_parts(
        dirs::config_dir(),
        dirs::home_dir(),
        std::env::var_os("XDG_CONFIG_HOME").filter(|value| !value.is_empty()),
    )
}

fn candidate_paths_from_parts(
    platform_config_dir: Option<PathBuf>,
    home_dir: Option<PathBuf>,
    xdg_config_home: Option<std::ffi::OsString>,
) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let mut seen = HashSet::new();

    let mut push_unique = |path: PathBuf| {
        if seen.insert(path.clone()) {
            paths.push(path);
        }
    };

    if let Some(config_dir) = platform_config_dir {
        push_unique(config_dir.join(GLOBAL_CONFIG_DIR).join(TUI_CONFIG_FILE));
    }

    if let Some(home) = home_dir {
        push_unique(
            home.join(".config")
                .join(GLOBAL_CONFIG_DIR)
                .join(TUI_CONFIG_FILE),
        );
    }

    if let Some(xdg) = xdg_config_home {
        push_unique(
            PathBuf::from(xdg)
                .join(GLOBAL_CONFIG_DIR)
                .join(TUI_CONFIG_FILE),
        );
    }

    paths
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEvent;

    fn source() -> PathBuf {
        PathBuf::from("/tmp/cflx-test/tui.jsonc")
    }

    fn parse_config(content: &str) -> Result<TuiConfig> {
        TuiConfig::parse_jsonc(content, &source())
    }

    #[test]
    fn default_config_uses_f5() {
        let config = TuiConfig::default();
        assert_eq!(config.start_key_label(), "F5/!");
        assert!(config.matches_start_key(&KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE)));
        assert!(config.matches_start_key(&KeyEvent::new(KeyCode::Char('!'), KeyModifiers::NONE)));
    }

    #[test]
    fn parses_jsonc_start_bindings() {
        let config = parse_config(
            r#"{
              // user preference
              "keybindings": { "start": ["F5", "!", "Space",], }
            }"#,
        )
        .unwrap();

        assert_eq!(config.start_key_label(), "F5/!/Space");
        assert!(config.matches_start_key(&KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE)));
        assert!(config.matches_start_key(&KeyEvent::new(KeyCode::Char('!'), KeyModifiers::NONE)));
        assert!(config.matches_start_key(&KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE)));
    }

    #[test]
    fn parses_supported_key_names() {
        let config = parse_config(
            r#"{"keybindings":{"start":["F1","F12","Esc","Enter","Space","Tab","PageUp","PageDown","Home","End","Up","Down","Left","Right","a"]}}"#,
        )
        .unwrap();

        assert_eq!(
            config.start_key_label(),
            "F1/F12/Esc/Enter/Space/Tab/PageUp/PageDown/Home/End/Up/Down/Left/Right/a"
        );
    }

    #[test]
    fn rejects_unknown_key_names() {
        let err = parse_config(r#"{"keybindings":{"start":["Hyper"]}}"#).unwrap_err();
        let message = err.to_string();
        assert!(message.contains("Invalid TUI config"));
        assert!(message.contains("keybindings.start[0]"));
        assert!(message.contains("unknown key"));
        assert!(message.contains("/tmp/cflx-test/tui.jsonc"));
    }

    #[test]
    fn rejects_empty_start_array() {
        let err = parse_config(r#"{"keybindings":{"start":[]}}"#).unwrap_err();
        assert!(err
            .to_string()
            .contains("keybindings.start must contain at least one"));
    }

    #[test]
    fn rejects_duplicate_start_entries_after_normalization() {
        let err = parse_config(r#"{"keybindings":{"start":["F5","f5"]}}"#).unwrap_err();
        assert!(err.to_string().contains("duplicate key binding"));
    }

    #[test]
    fn rejects_unsupported_modifier_strings() {
        let err = parse_config(r#"{"keybindings":{"start":["Ctrl+R"]}}"#).unwrap_err();
        assert!(err.to_string().contains("unsupported modifier syntax"));
    }

    #[test]
    fn parse_errors_include_config_path() {
        let err = parse_config(r#"{"keybindings": "#).unwrap_err();
        let message = err.to_string();
        assert!(message.contains("Failed to parse TUI config file"));
        assert!(message.contains("/tmp/cflx-test/tui.jsonc"));
    }

    #[test]
    fn candidate_paths_follow_low_to_high_priority_and_deduplicate() {
        let paths = candidate_paths_from_parts(
            Some(PathBuf::from("/platform")),
            Some(PathBuf::from("/home/alice")),
            Some(std::ffi::OsString::from("/xdg")),
        );
        assert_eq!(
            paths,
            vec![
                PathBuf::from("/platform/cflx/tui.jsonc"),
                PathBuf::from("/home/alice/.config/cflx/tui.jsonc"),
                PathBuf::from("/xdg/cflx/tui.jsonc"),
            ]
        );

        let deduped = candidate_paths_from_parts(
            Some(PathBuf::from("/same/.config")),
            Some(PathBuf::from("/same")),
            Some(std::ffi::OsString::from("/same/.config")),
        );
        assert_eq!(deduped, vec![PathBuf::from("/same/.config/cflx/tui.jsonc")]);
    }

    #[test]
    fn load_from_file_does_not_require_orchestration_commands() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tui.jsonc");
        std::fs::write(&path, r#"{"keybindings":{"start":["F5","!"]}}"#).unwrap();

        let config = TuiConfig::load_from_file(&path).unwrap();

        assert_eq!(config.start_key_label(), "F5/!");
    }
}

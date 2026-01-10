# Design: Open Editor in Change Directory

## Architecture Overview

TUIからエディタを起動する際の主要な課題：

1. **TUIの一時停止と復帰**: crosstermのraw modeを適切に解除・復帰する必要がある
2. **子プロセスの制御**: エディタプロセスをフォアグラウンドで実行し、終了を待機
3. **環境変数の取得**: `$EDITOR` の取得とフォールバック処理

## Technical Approach

### Terminal State Management

```
TUI Active → Disable Raw Mode → Restore Terminal → Launch Editor → Wait → Re-enable Raw Mode → TUI Resume
```

1. `crossterm::terminal::disable_raw_mode()` でraw modeを解除
2. `crossterm::execute!(stdout, LeaveAlternateScreen)` で代替画面を離れる
3. エディタプロセスを起動し、終了を待機
4. `crossterm::execute!(stdout, EnterAlternateScreen)` で代替画面に戻る
5. `crossterm::terminal::enable_raw_mode()` でraw modeを再有効化
6. 画面を再描画

### Editor Launch

```rust
fn launch_editor(change_id: &str) -> Result<()> {
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
    let change_dir = Path::new("openspec/changes").join(change_id);

    let status = Command::new(&editor)
        .arg(".")
        .current_dir(&change_dir)
        .status()?;

    Ok(())
}
```

### Supported Editors

`$EDITOR` の値によって異なる動作：

| Editor | Command | Notes |
|--------|---------|-------|
| vim/nvim | `$EDITOR .` | ディレクトリを開く |
| code | `$EDITOR . --wait` | VSCodeの場合は`--wait`が必要 |
| emacs | `$EDITOR .` | diredモードで開く |

基本的には `$EDITOR .` で統一し、VSCodeなど特殊なエディタは利用者側で `EDITOR="code --wait"` のように設定してもらう。

## Implementation Details

### Key Event Handler

`tui.rs` の key event handler に `KeyCode::Char('e')` を追加：

```rust
(KeyCode::Char('e'), _) => {
    if app.mode == AppMode::Select {
        if let Some(change) = app.get_current_change() {
            // Terminal state restoration
            disable_raw_mode()?;
            execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;

            // Launch editor
            launch_editor_for_change(&change.id)?;

            // Restore terminal state
            enable_raw_mode()?;
            execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
            terminal.clear()?;
        }
    }
}
```

### Helper Function Location

新しいヘルパー関数 `launch_editor_for_change` を `tui.rs` に追加：

```rust
fn launch_editor_for_change(change_id: &str) -> Result<()> {
    let editor = std::env::var("EDITOR")
        .unwrap_or_else(|_| "vi".to_string());

    let change_dir = Path::new("openspec/changes").join(change_id);

    if !change_dir.exists() {
        return Err(OrchestratorError::ChangeNotFound(change_id.to_string()));
    }

    Command::new(&editor)
        .arg(".")
        .current_dir(&change_dir)
        .status()
        .map_err(|e| OrchestratorError::EditorLaunchFailed(e.to_string()))?;

    Ok(())
}
```

## Error Handling

| Error | Message | Action |
|-------|---------|--------|
| EDITOR未設定 | "EDITOR environment variable not set" | 警告表示、vi使用 |
| ディレクトリなし | "Change directory not found" | 警告表示 |
| エディタ起動失敗 | "Failed to launch editor: {error}" | エラーログ |

## Considerations

### Running Mode での無効化

実行モードではエディタ起動を無効化する理由：
- プロセス実行中にファイル編集するとデータ不整合の可能性
- TUIの状態復帰が複雑になる

### Screen Clear Issue

エディタ終了後、TUIの再描画で画面がちらつく可能性。
`terminal.clear()` + 即座の再描画で対応。

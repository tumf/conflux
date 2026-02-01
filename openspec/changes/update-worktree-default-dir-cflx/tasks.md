## 1. 実装
- [ ] 1.1 `default_workspace_base_dir()` のデフォルトパスを `cflx` に統一し、macOS のフォールバックを `~/.local/share` に変更する
  - 検証: `src/config/defaults.rs` の `default_workspace_base_dir()` を確認し、macOS/Linux/Windows の分岐が以下になることを確認する
    - `XDG_DATA_HOME` がある場合: `${XDG_DATA_HOME}/cflx/worktrees/<project_slug>`
    - macOS/Linux のフォールバック: `~/.local/share/cflx/worktrees/<project_slug>`
    - Windows: `%APPDATA%/cflx/worktrees/<project_slug>`
    - その他: `${TMPDIR}/cflx-workspaces-fallback/<project_slug>`

- [ ] 1.2 `default_workspace_base_dir()` のテストとコメントを新しいパスに合わせて更新する
  - 検証: `src/config/defaults.rs` 内のテストが `cflx` と新しい macOS フォールバックを期待するよう更新されていることを確認する

## 2. 検証
- [ ] 2.1 既存テストが通ることを確認する
  - 検証: `cargo test default_workspace_base_dir` が成功する

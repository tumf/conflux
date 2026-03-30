---
change_type: implementation
priority: medium
dependencies: []
references:
  - src/service/mod.rs
  - src/config/defaults.rs
---

# Change: サーバログパスを /tmp から XDG_STATE_HOME 準拠に移動

**Change Type**: implementation

## Problem / Context

`cflx server` の launchd plist テンプレート (`src/service/mod.rs`) で、`StandardOutPath` / `StandardErrorPath` が `/tmp/cflx-server.log` にハードコードされている。

- `/tmp` は OS 再起動やクリーンアップで消去される可能性がある
- orchestrator ログは既に `get_log_file_path()` で XDG_STATE_HOME 準拠 (`~/.local/state/cflx/logs/`) に配置されているが、サーバログだけ `/tmp` に残っている
- ログ配置ポリシーが不統一

## Proposed Solution

サーバログパスを XDG_STATE_HOME 準拠に統一する。

- `config/defaults.rs` に `get_server_log_path() -> PathBuf` を追加
- フォールバック順: `$XDG_STATE_HOME/cflx/server.log` → `~/.local/state/cflx/server.log` → `{temp_dir}/cflx-server.log`
- `generate_plist()` のシグネチャを拡張し、ログパスを動的に埋め込む
- `install()` でログディレクトリを事前に `create_dir_all` する

## Acceptance Criteria

1. `cflx service install` で生成される plist の `StandardOutPath` / `StandardErrorPath` が `~/.local/state/cflx/server.log` を指す（XDG_STATE_HOME 未設定時）
2. `XDG_STATE_HOME` が設定されている場合、そのパス配下を使用する
3. ホームディレクトリが取得できない場合、`/tmp/cflx-server.log` にフォールバックする
4. ログディレクトリが存在しない場合、install 時に自動作成される
5. 既存の orchestrator ログパス (`get_log_file_path`) に影響しない

## Out of Scope

- 既存の `/tmp/cflx-server.log` からの自動マイグレーション
- ログローテーション（サーバログは単一ファイル）
- Windows / Linux の launchd 以外のサービスマネージャ対応

# Change: on_merged フック実行の信頼性向上

**Change Type**: implementation

## Why

parallel mode で `on_merged` フック（`make bump-patch` → `cargo release`）が 2 種類の原因で失敗し、バージョンバンプが適用されない。

1. **`.git/index.lock` 競合** — resolve コマンド（AI エージェント）のプロセスクリーンアップが完了する前に hook が走り、`cargo release` の git commit が lock ファイルに衝突する
2. **uncommitted changes 検出** — 他の parallel workspace の apply が同時実行中で、メインリポジトリの staging area に未コミットファイルが残っており、`cargo release` がクリーンな working tree を要求して中断する

どちらの場合も `continue_on_failure: true`（デフォルト）により警告ログのみで続行し、ユーザーが気づきにくい。

## What Changes

- `HookConfig` に `max_retries` と `retry_delay_secs` フィールドを追加し、フック失敗時にリトライ可能にする
- `HookRunner` に `repo_root` フィールドを追加し、フック実行時の作業ディレクトリを明示化する
- `on_merged` フック実行前に `.git/index.lock` の解放を待つロジックを追加する
- `HooksConfig` に `index_lock_wait_secs` を追加し、index.lock 待機の最大秒数を設定可能にする

## Acceptance Criteria

- `HookConfig` に `max_retries`（デフォルト 0）と `retry_delay_secs`（デフォルト 3）が追加されている
- `HookRunner` がフック実行時に `cmd.current_dir(repo_root)` を設定する
- `on_merged` フック実行前に `.git/index.lock` の存在をポーリングし、解放を待つ
- 全リトライ失敗後に `continue_on_failure` 判定を行う
- 既存のフック設定（文字列形式・オブジェクト形式）との後方互換性を維持する
- 新フィールドは `#[serde(default)]` でデフォルト値を持ち、未指定時は既存動作と同一

## Out of Scope

- `bump.sh` や `cargo release` 側の修正（`--allow-dirty` 等）
- 全 merge 完了後に on_merged をまとめて 1 回実行するバッチモード
- `web` 設定キーの `.cflx.jsonc` からの読み込み対応

## Impact

- Affected specs: hooks, configuration
- Affected code: `src/hooks.rs`, `src/config/types.rs`, `src/orchestrator.rs`, `src/parallel_run_service.rs`, `src/tui/orchestrator.rs`, `src/tui/command_handlers.rs`

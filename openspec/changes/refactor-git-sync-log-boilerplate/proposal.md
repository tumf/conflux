# Change: Git同期APIのログ送出重複を整理する

## Why
`src/server/api/git_sync.rs` の `run_resolve_command` では、開始・失敗・stdout/stderr・終了の各ケースで `RemoteLogEntry` を繰り返し組み立てています。ログ項目の大半が同一で、将来フィールド追加やレベル変更が入るたびに複数箇所を同時修正する必要があり、サーバーAPIの保守性を下げています。

## What Changes
- `run_resolve_command` 内のログ生成を、共通の補助関数または小さなビルダーへ寄せる
- `resolve` 操作で共通な `project_id` / `operation` / `timestamp` 付与を一元化する
- 既存のログ内容・レベル・送出タイミングを固定するキャラクタリゼーションテストを先に用意する

## Evidence
- `src/server/api/git_sync.rs:24` `emit_log_entry()` が低レベル送出のみ担当している
- `src/server/api/git_sync.rs:60` 開始ログで `RemoteLogEntry` を直接組み立てている
- `src/server/api/git_sync.rs:87` 起動失敗ログでも同様の構造を繰り返している
- `src/server/api/git_sync.rs:121` stdout 行ごとに同じフィールドを再構築している
- `src/server/api/git_sync.rs:139` stderr 行ごとに同じフィールドを再構築している
- `src/server/api/git_sync.rs:160` 終了ログでも同一フィールド群を再度設定している

## Impact
- Affected specs: `code-maintenance`, `git-sync`, `observability`
- Affected code: `src/server/api/git_sync.rs`, 関連テスト
- API/CLI互換性: 変更なし

## Acceptance Criteria
- `resolve_command` 実行時のログメッセージ本文、レベル、送出順序が回帰しない
- `git/sync` のHTTPレスポンス形式とCLI/サーバー公開挙動に変更がない
- ログエントリ生成の重複が減り、共通フィールドの設定箇所が一元化されている

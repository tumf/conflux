## 1. Implementation

- [x] 1.1 `hooks` 設定構造（短縮形/オブジェクト形式）の追加
- [x] 1.2 フック実行（timeout, continue_on_failure）の共通実装
- [x] 1.3 プレースホルダー展開（`{change_id}` 等）と環境変数注入
- [x] 1.4 `run`（非TUI）フローに各フックポイントを統合
- [x] 1.5 TUI 実行フローに各フックポイントを統合
- [x] 1.6 エラーハンドリング（hook失敗時の継続/停止）を実装

## 2. Validation

- [x] 2.1 `cargo test`
- [x] 2.2 `cargo fmt --check`
- [x] 2.3 `cargo clippy -- -D warnings`

## 3. Documentation

- [x] 3.1 `README.md` に `hooks` の設定例と一覧を追記

## 4. OpenSpec

- [x] 4.1 `openspec validate add-orchestrator-hooks --strict`

## Implementation Tasks

- [x] `src/server/api/ws.rs` の `build_changes_payload` (L327-341) を、Reducer (`OrchestratorState`) が利用可能な場合は `ChangeRuntimeState.display_status()` から per-change ステータスを取得するように変更する (verification: `cargo test --lib server::api::ws`)
- [x] Reducer が不在の場合のフォールバック経路で `WorkspaceState::Applied` → `"applied"` (not `"archiving"`) に修正する (verification: WebSocket テスト)
- [x] Reducer 不在フォールバック経路で `WorkspaceState::Created` → `"created"` (not `"queued"`) に修正する (verification: WebSocket テスト)
- [x] base branch `REJECTED.md` による二重判定 (L327-328) を削除し、Reducer の `TerminalState::Rejected` に一元化する (verification: rejected change の WebSocket 表示テスト)
- [x] WebSocket payload に `accepting`, `resolving`, `merge wait`, `resolve pending`, `blocked` ステータスが正しく含まれることを確認する統合テストを追加する (verification: `cargo test --lib server::api::ws`)
- [x] TUI の `display_status()` と WebSocket 表示の一致を検証する回帰テストを追加する (verification: `cargo test --lib server`)

## Future Work

- ダッシュボード UI コンポーネントの色・ラベル対応 (accepting 等の新ステータスに対する視覚的対応)

## Acceptance #1 Failure Follow-up

- [x] `src/server/api/control.rs` と `src/server/api/ws.rs` の未コミット差分を整理し、`git status --porcelain` が空になる状態にする
- [x] `src/server/api/ws.rs` の `handle_ws` の引数数を 7 以下へリファクタリングし、`cargo clippy -- -D warnings` を通す
- [x] pre-commit hook（rustfmt/clippy）を通常コミット相当で再実行し、hook が自動修正や失敗なしで完了することを確認する

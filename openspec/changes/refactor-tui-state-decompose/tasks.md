## 1. キャラクタリゼーション
- [x] 1.1 選択切替・並列実行・resolve queue・ワークツリー操作の主要状態遷移を固定するテストを整理または追加する（確認: `cargo test --lib tui::state:: -- --nocapture` 成功）
- [x] 1.2 ログ操作とマージガードの既存挙動を固定するテストを補強する（確認: `src/tui/state/log_logic.rs` / `src/tui/state/worktree_logic.rs` の追加テストと既存テスト成功）

## 2. リファクタリング
- [x] 2.1 `state.rs` 内のロジックを責務別サブモジュールへ段階的に分割できる境界に整理する（確認: `selection_logic.rs` / `log_logic.rs` / `worktree_logic.rs` を追加し責務を分離）
- [x] 2.2 `AppState` の公開メソッド契約を維持しつつ、内部依存を縮小する（確認: `AppState` 公開メソッドのシグネチャ変更なし）
- [x] 2.3 必要に応じて分割方針と責務境界を `design.md` に記録する（確認: `Implementation Notes` を追記）

## 3. 回帰確認
- [x] 3.1 TUI state / runner 関連テストを実行し、主要フローに回帰がないことを確認する（確認: `cargo test --lib tui::state:: -- --nocapture` / `cargo test --lib tui::runner:: -- --nocapture` 成功）
- [x] 3.2 画面操作・キュー操作・マージ操作の公開挙動に変更がないことを確認する（確認: 公開 API/CLI 変更なし、既存 TUI state テスト 99 件成功）

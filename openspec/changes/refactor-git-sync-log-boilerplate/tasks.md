## 1. キャラクタリゼーション
- [x] 1.1 `run_resolve_command` の開始・失敗・stdout・stderr・終了ログを固定するテストを追加する（確認: ログ件数・レベル・順序が期待値と一致）
- [x] 1.2 `git/sync` の公開レスポンスとログ送出が現行どおりであることを確認する（確認: API互換性維持）

## 2. リファクタリング
- [x] 2.1 `RemoteLogEntry` の共通フィールドを組み立てる補助関数またはビルダーを導入する（確認: `run_resolve_command` 内の重複初期化が減る）
- [x] 2.2 `resolve` 操作向けログ生成を一元化し、メッセージとレベルだけを差し替える構造に整理する（確認: フィールド追加時の修正箇所が限定される）

## 3. 回帰確認
- [x] 3.1 git sync / resolve 関連テストを実行し、ログとレスポンスに回帰がないことを確認する（確認: 関連テスト成功）
- [x] 3.2 サーバーAPIの公開仕様に変更がないことを確認する（確認: API/CLI変更なし）

## Acceptance #4 Failure Follow-up
- [x] `.cflx/` 配下の生成物（`acceptance-state.json`）を整理し、不要な未追跡ファイルを解消する
- [x] `cargo fmt --all` を実行して `src/server/api/git_sync.rs` のテストコード整形差分を解消する
- [x] `parallel::tests::executor::test_idle_queue_addition_marks_reanalysis_and_enqueues_change` の前提change_idを現行リポジトリ構成に合わせて修正し、`cargo test` を通す

## Acceptance #7 Failure Follow-up
- [x] `.cflx/` 配下の acceptance 生成物を ignore または削除して、`git status --porcelain` を空にする
- [x] `tasks.md` の `.cflx/` 整理完了チェックを、実際に working tree が clean である状態と一致するよう確認・更新する


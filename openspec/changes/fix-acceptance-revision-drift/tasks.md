## Implementation Tasks

- [x] `src/parallel/executor.rs` の acceptance 実行で開始時 revision と終了時 revision を区別し、durable acceptance state 更新に終了時 HEAD を使う（verification: integration - acceptance 実行テストで保存 revision が command 完了後 HEAD と一致すること）
- [x] PASS / FAIL / CONTINUE / BLOCKED / command failure の各分岐で revision 保存基準を終了時 HEAD に統一する（verification: integration - `src/parallel/tests/executor.rs` の acceptance result 別テストで state revision が終了時 HEAD と一致すること）
- [x] acceptance 中に HEAD が変化した場合の診断ログを追加し、archive guard mismatch のトリアージ情報を残す（verification: integration - ログ期待 or state-based test で start/end revision 差分が観測可能であること）
- [x] acceptance 中に HEAD が変化するケースの回帰テストを追加し、PASS 後 archive guard が stale mismatch で失敗しないことを検証する（verification: integration - `cargo test parallel::tests::executor -- --nocapture`）
- [x] lint / typecheck / test を通し、並列実行の既存 durable acceptance guard 回帰がないことを確認する（verification: manual - `cargo fmt --check && cargo clippy -- -D warnings && cargo test`）

## Future Work

- acceptance history に start/end revision の両方を永続化して UI から観測できるようにするかの別 proposal
- acceptance を read-only に制限する運用ポリシーの再検討

## Acceptance #1 Failure Follow-up

- [x] `openspec/changes/fix-acceptance-revision-drift/tasks.md` の verification ownership を実態に合わせて見直し、`src/parallel/tests/executor.rs` の実 Git / filesystem / process 依存テストは integration として再分類する
- [x] durable acceptance revision 判定を純粋ロジックとして切り出し、実 Git / filesystem / process に依存しない unit test を追加して checklist の `verification: unit` を真実にする

## Acceptance #2 Failure Follow-up

- [x] acceptance attempt history に保存する `commit_hash` を開始時 revision ではなく終了時 revision に更新し、次回 acceptance diff context が revision drift しないようにする
- [x] `src/parallel/executor.rs` の acceptance diff context が直前 acceptance 後の revision を基準に changed files を計算することを、HEAD 変化ありケースで回帰テスト化する

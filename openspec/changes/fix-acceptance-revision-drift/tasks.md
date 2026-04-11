## Implementation Tasks

- [ ] `src/parallel/executor.rs` の acceptance 実行で開始時 revision と終了時 revision を区別し、durable acceptance state 更新に終了時 HEAD を使う（verification: unit - acceptance 実行テストで保存 revision が command 完了後 HEAD と一致すること）
- [ ] PASS / FAIL / CONTINUE / BLOCKED / command failure の各分岐で revision 保存基準を終了時 HEAD に統一する（verification: unit - `src/parallel/tests/executor.rs` の acceptance result 別テストで state revision が終了時 HEAD と一致すること）
- [ ] acceptance 中に HEAD が変化した場合の診断ログを追加し、archive guard mismatch のトリアージ情報を残す（verification: unit - ログ期待 or state-based test で start/end revision 差分が観測可能であること）
- [ ] acceptance 中に HEAD が変化するケースの回帰テストを追加し、PASS 後 archive guard が stale mismatch で失敗しないことを検証する（verification: unit - `cargo test parallel::tests::executor -- --nocapture`）
- [ ] lint / typecheck / test を通し、並列実行の既存 durable acceptance guard 回帰がないことを確認する（verification: manual - `cargo fmt --check && cargo clippy -- -D warnings && cargo test`）

## Future Work

- acceptance history に start/end revision の両方を永続化して UI から観測できるようにするかの別 proposal
- acceptance を read-only に制限する運用ポリシーの再検討

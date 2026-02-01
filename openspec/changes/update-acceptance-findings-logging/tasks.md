## 1. Implementation
- [ ] 1.1 parallel acceptance の FAIL ログから "findings 件数" 表現を削除し、必要なら "tail 行数" と明示する（検証: `src/parallel/executor.rs` のログ文言）
- [ ] 1.2 serial acceptance の FAIL ログも同方針に統一する（検証: `src/serial_run_service.rs` のログ文言）
- [ ] 1.3 acceptance の findings として使う tail から `ACCEPTANCE:` マーカーと `FINDINGS:` 行を除外し、`- ` 箇条書きの構造解析は行わない（検証: `src/orchestration/acceptance.rs` の `build_acceptance_tail_findings`）
- [ ] 1.4 acceptance FAIL の findings には stdout/stderr tail を使うことを確認し、parse 結果を直接使わない（検証: `src/parallel/executor.rs` / `src/serial_run_service.rs` の FAIL ハンドリング）
- [ ] 1.5 tail フィルタのユニットテストを追加または更新する（検証: `src/orchestration/acceptance.rs` のテスト）

## 2. Validation
- [ ] 2.1 `cargo test acceptance -- --nocapture` を実行し、tail フィルタのテストが通ることを確認する

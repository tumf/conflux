# Tasks: Acceptance failure follow-up authoring

## 1. Acceptance prompt update
- [x] 1.1 `src/config/defaults.rs` の `ACCEPTANCE_SYSTEM_PROMPT` に、FAIL 時に `openspec/changes/{change_id}/tasks.md` を直接更新する指示を追加する。`## Acceptance #<n> Failure Follow-up` と `- [ ] <finding>` 形式、`ACCEPTANCE:`/`FINDINGS:` 行を追加しないこと、既存セクションがある場合は同じセクションを更新することを明記する。確認: プロンプト定数に新しい指示が含まれていること。

## 2. Orchestrator follow-up handling
- [x] 2.1 `src/orchestration/acceptance.rs` で acceptance FAIL 時の `update_tasks_on_acceptance_failure` 呼び出しを削除し、tasks.md を自動更新しないようにする。履歴記録は維持する。確認: FAIL 分岐から tasks 更新処理がなくなっていること。
- [x] 2.2 `src/parallel/executor.rs` と `src/serial_run_service.rs` の acceptance FAIL 処理を見直し、tasks.md を自動更新しないよう整理する。確認: FAIL 分岐に tasks 更新処理が存在しないこと。

## 3. Tests
- [x] 3.1 `src/orchestration/acceptance.rs` のテストを更新し、FAIL 時に tasks.md を自動追記しない仕様に合わせて期待値を修正する。確認: `cargo test acceptance` が通ること。

## 4. Validation
- [x] 4.1 `cargo test acceptance` を実行し、acceptance 関連のテストが通ることを確認する。

---
change_type: implementation
priority: high
dependencies: []
references:
  - src/parallel/executor.rs
  - src/parallel/acceptance_state.rs
  - src/parallel/dispatch.rs
  - openspec/specs/parallel-execution/spec.md
---

# Fix acceptance revision drift before archive

**Change Type**: implementation

## Problem / Context

- このセッションでは `rename-to-hermes-manager` の実行中に、acceptance が PASS した直後にもかかわらず archive guard が `durable acceptance-pass state missing for revision ...` で失敗した。
- 調査したログでは acceptance PASS は `revision=f9afe5cf007735e177231a93bf9ebef8ec19794b` として durable state に保存されていた一方、archive guard は直後の `HEAD=1cbc1af6abebfdb8d463ae982bd877944041f52d` を参照して stale 判定していた。
- `src/parallel/executor.rs` の acceptance 実装は acceptance 開始時に取得した revision を `revision_for_attempt` として固定し、pass/fail 保存にそのまま再利用している。
- 実運用では acceptance エージェントが `git add ... && git commit ...` を実行しうるため、acceptance 中に HEAD が進んだ場合、保存された durable acceptance state と archive guard の current revision が不一致になりうる。
- その結果、同一 acceptance サイクルで PASS → archive 進行 → stale guard fail という不整合が起き、正常完了できない。

## Proposed Solution

- acceptance 実行では開始時 revision と終了時 revision を区別し、durable acceptance state の保存には acceptance コマンド終了後に再取得した終了時 HEAD を用いる。
- acceptance 中に HEAD が変化した場合は、その事実を明示的にログへ記録し、archive guard と同じ revision 基準で durable pass/fail を扱う。
- archive guard の厳格な照合は維持しつつ、同一 acceptance サイクル内で作られた commit に対して PASS 記録が古くならないようにする。
- acceptance failure / blocked / continue など non-pass の durable state も同様に終了時 revision 基準へ揃え、resume 判定と archive guard の整合性を保つ。
- 回帰テストを追加し、acceptance 中に HEAD が変化するケースで PASS 後 archive-ready になることと、non-pass 時に終了時 revision の state が保存されることを検証する。

## Acceptance Criteria

- acceptance 中に commit が作られて HEAD が変化した場合、PASS 後に保存される durable acceptance state の revision は acceptance 終了時の HEAD と一致する。
- archive guard は同一サイクル直後に stale mismatch で失敗せず、終了時 HEAD に対する durable pass を認識できる。
- acceptance 中に HEAD が変化しなかった場合の既存挙動は維持される。
- FAIL / CONTINUE / BLOCKED / command failure の durable acceptance state も終了時 revision と整合する。
- 回帰テストが、acceptance 中の HEAD 変更あり/なしの両ケースを保護する。

## Out of Scope

- acceptance を read-only に制限するポリシー変更
- acceptance 履歴 UI / dashboard 表示の全面的な再設計
- archive / merge 全体のワークフロー変更

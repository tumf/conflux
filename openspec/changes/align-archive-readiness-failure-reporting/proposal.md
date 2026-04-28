---
change_type: hybrid
priority: high
dependencies: []
references:
  - src/openspec_cmd.rs
  - src/parallel/executor.rs
  - src/orchestration/archive.rs
  - src/execution/archive.rs
  - src/agent/prompt.rs
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/agent-prompts/spec.md
  - skills/cflx-archive/SKILL.md
---

# Change: archive readiness定義とarchive失敗原因表示を整合させる

**Change Type**: hybrid

## Premise / Context

- 現行の並列 archive 実行では、agent 側が `cflx openspec archive <id> --yes` の前提条件エラーを自然文で説明しても、プロセス自体が exit code 0 で終了すると orchestrator は archive command 成功として扱う。
- その後の `verify_archive_completion` はファイル状態のみを見て `openspec/changes/<change_id>` 残留を検出し、最終的に「archive command succeeded but not actually archived」という二次エラーへ潰してしまう。
- 実ログでは `refactor-split-tui-state-appstate` の archive 失敗時に、真の原因が `tasks.md: Runtime behavior is claimed without implementation-facing tasks` だったにもかかわらず、最終ユーザー表示は未アーカイブ一般論になっていた。
- acceptance / archive-readiness 側の canonical spec は final archive commit を阻害する commit-path blocker を acceptance で先に露出することを要求しているが、archive skill と archive 実行結果の扱いはその契約を十分に表現できていない。

## Problem / Context

Conflux の archive フローには、archive readiness の契約と archive failure の報告粒度にズレがある。

第1に、archive 本番は `validate_change(..., strict=true, evidence="error")` を使って前提条件を評価するが、proposal / skill / user-facing guidance は `--strict` や `--evidence warn` でも archive-ready であるかのように読める場面が残る。その結果、acceptance では通ったように見える change が archive で初めて落ちる。

第2に、archive agent が「前提条件エラーのため archive 未実施」と自然文で報告しても、Conflux core は exit code 成功だけを見て archive command success とみなし、最終的に file-state verification failure に再分類してしまう。そのためユーザーや後続の retry ロジックからは真の blocker が見えにくい。

この状態では、archive の再試行ポリシー、acceptance の archive-readiness、運用時の障害切り分けが一貫せず、誤った修正箇所へ誘導されやすい。

## Proposed Solution

archive readiness の定義・archive skill の期待・orchestrator の失敗分類を同じ契約に揃える。

- archive readiness を「最終 archive commit を成立させる実 blocker がないこと」と定義し、archive 実行時の validation/evidence policy と整合する形で spec と skill を明文化する。
- archive 実行では、exit code だけでなく archive CLI の実施有無・前提条件 failure の報告も踏まえて失敗分類し、`openspec/changes/<change_id>` 残留だけの一般論に潰さない。
- parallel / serial 両方で、verification failure の最終エラーに直前 attempt の root cause を保持・表示できるようにする。
- archive agent / skill guidance では、`cflx openspec archive ...` が archive-readiness blocker を返した場合に、それを archive 未実施として明示し、修正対象を tasks/proposal/verification に正しく向ける。

## Acceptance Criteria

- archive readiness に関する canonical spec と skill guidance は、archive 実行時に必要な validation/evidence 契約をユーザー・agent・orchestrator の間で矛盾なく表現する。
- archive agent が前提条件 failure を報告して archive を実施していないケースでは、Conflux は単なる「not actually archived」ではなく root cause を含む archive failure として扱う。
- parallel / serial archive の最終失敗メッセージには、直前 attempt で観測された validation failure や commit-path blocker の要約が含まれ、修正対象が分かる。
- archive verification は引き続き file-state を最終判定に使うが、root cause を上書きせず補助情報として扱う。
- `refactor-split-tui-state-appstate` のような tasks validation blocker が再現した場合、ユーザー表示・履歴・retry 文脈のいずれからも blocker の内容を追跡できる。

## Explicit Completion Conditions

- archive readiness / failure-reporting 契約を更新する spec delta が追加され、`parallel-execution` または `agent-prompts` の canonical requirement に反映される。
- `src/parallel/executor.rs` と `src/orchestration/archive.rs` の archive 失敗処理に、verification failure 時の root-cause 保持または伝播ロジックが追加される。
- archive agent / prompt / skill 側で、archive 未実施の前提条件 failure を success 扱いしない guidance か、少なくとも downstream に root cause を渡す guidance が追加される。
- Rust テストで「exit code 0 だが archive 前提条件 failure を報告する」ケースの archive failure 分類または error message 伝播が検証される。
- `cflx openspec validate align-archive-readiness-failure-reporting --strict --evidence warn` が成功する。

## Out of Scope

- `tasks.md` validator の behavior-task heuristics 自体を今回の proposal で緩和・変更すること。
- archive の再試行回数や backoff 方針そのものを大きく再設計すること。
- apply / acceptance / resolve 全体の exit-code contract を archive 以外まで一括で作り直すこと。

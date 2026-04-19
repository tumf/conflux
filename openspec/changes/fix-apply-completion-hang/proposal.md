---
change_type: implementation
priority: high
dependencies: []
references:
  - src/execution/apply.rs
  - src/parallel/executor.rs
  - src/orchestration/apply.rs
  - openspec/specs/parallel-execution/spec.md
---

# Change: apply 完了後に agent process が居残っても acceptance handoff へ進める

**Change Type**: implementation

## Problem / Context

parallel apply は agent command の出力チャネルが閉じるまで `rx.recv()` を待ち続け、その後でしか `child.wait()` と task progress 再評価に進みません。
そのため tasks.md 上では完了していても、agent process や配下の子プロセスが stdout/stderr を保持したまま居残るケースでは apply handoff が止まり、`Apply completed` や `Acceptance started` に到達できません。

今回の症状は間欠的であり、agent process が自然終了した run は先へ進み、自然終了しない run だけが apply 段階で停止します。
acceptance には canonical verdict 検知後の grace period と早期 terminate が既にありますが、apply には同等の救済がありません。

## Proposed Solution

- apply 実行中の出力ストリーム処理に、workspace 状態ベースの早期完了判定を追加する
- 完了条件（tasks 完了または apply-blocked handoff）を観測したら grace period を開始し、自然終了しない child を terminate する
- 上記 terminate 後の非0 exit は、完了条件を観測済みの run に限り成功相当として扱う
- inactivity timeout/retry 経路でも workspace 状態を再評価し、既に完了済みなら不要な retry を避ける
- この挙動を parallel-execution spec に明記し、回帰テストで間欠ハングを固定化する

## Acceptance Criteria

- tasks.md が完了条件を満たした後に apply agent process が出力チャネルを閉じなくても、orchestrator は有限時間内に apply handoff を確定し acceptance へ進む
- `REJECTED.md` による apply-blocked handoff が検知済みの run でも、居残り child によって apply loop が無限待機しない
- 完了条件未達の run は従来どおり成功扱いされず、誤って acceptance へ進まない
- 回帰テストで「完了条件達成後に sleep/居残りする apply command」が handoff に成功することを確認できる

## Out of Scope

- acceptance verdict contract の再設計
- `cflx-apply` skill 自体への新しい machine-readable verdict 契約追加
- serial mode の大規模な実行モデル変更

---
change_type: implementation
priority: high
dependencies: []
references:
  - src/acceptance.rs
  - src/parallel/executor.rs
  - src/orchestration/acceptance.rs
  - .opencode/commands/cflx-accept.md
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/agent-prompts/spec.md
  - ~/.local/state/cflx/logs/llm_hosting-197e010c/2026-04-18.log
---

# Change: Complete acceptance on standalone verdict

**Change Type**: implementation

## Premise / Context

- 現在の acceptance 実装は `src/acceptance.rs` で `starts_with("ACCEPTANCE: PASS")` を使っており、`ACCEPTANCE: PASSAll...` や `ACCEPTANCE: PASS## ...` も PASS 扱いしている。
- 2026-04-18 の実ログでは `rebuild-gpu-hosting-framework` に対して `ACCEPTANCE: PASSAll...`、`ACCEPTANCE: PASS`、`ACCEPTANCE: PASS## Acceptance Review Summary` の複数パターンが観測されている。
- 同ログでは acceptance プロセスが verdict 相当の出力後も 900 秒無出力で生存し続け、inactivity timeout により retry されている。
- 既存 canonical spec は acceptance verdict を「unwrapped standalone line」と定義しているが、runtime は trailing text 連結を許容しており、prompt 契約と実装挙動がずれている。

## Problem / Context

acceptance は archive の直前に実行される最終判定フェーズだが、現状は「standalone verdict を出したかどうか」ではなく「`ACCEPTANCE: PASS` で始まる文字列がどこかにあるか」で PASS を確定している。そのため、壊れた verdict 形式でも PASS と見なされ、さらに verdict 相当の出力後も acceptance コマンド本体が終了しない場合、runtime は 900 秒待って inactivity timeout retry を起こしてしまう。

この挙動は以下の 2 点で不安定である。

1. 出力契約違反 (`PASSAll`, `PASS## ...`) が success 扱いされる。
2. machine-readable verdict がすでに出ているのに orchestrator がプロセス終了を待ち続け、acceptance retry や duplicate archive handoff の原因になる。

## Proposed Solution

acceptance の成功確定条件を「canonical standalone verdict line の検出」に揃え、standalone verdict が確定した時点で acceptance operation を完了扱いにできるようにする。

具体的には:

1. runtime parser は canonical verdict を **単独行完全一致** で優先判定し、`PASSAll...` / `PASS## ...` のような trailing text 連結を PASS として扱わない。
2. acceptance 実行ランナーは stdout streaming 中に canonical standalone verdict を検出したら、その verdict を最終結果として確定し、残りの process lifetime に依存せず acceptance phase を完了できるようにする。
3. verdict 確定後は process group を明示的に終了させるか、少なくとも archive handoff 判定は process exit ではなく verdict detection を基準に進める。
4. command template / prompt 側では「単独行 verdict」が canonical contract であることを維持しつつ、その責務がテンプレート側にあることを明示する。
5. runtime と prompt contract の整合を regression test で固定する。

## Acceptance Criteria

- `ACCEPTANCE: PASSAll ...` や `ACCEPTANCE: PASS## ...` は canonical PASS として受理されない。
- `ACCEPTANCE: PASS` が単独行で一度検出された場合、acceptance は process exit を待たずに PASS を確定できる。
- verdict 確定後に acceptance 子プロセスがぶら下がっていても、900 秒 inactivity timeout retry には入らない。
- `.opencode/commands/cflx-accept.md` と canonical spec は、acceptance verdict が単独行完全一致であることを引き続き定義する。
- regression tests で `standalone PASS`, `PASSAll`, `PASS## heading`, `FAIL with findings`, `CONTINUE`, `BLOCKED` の parsing / handoff が検証される。

## Out of Scope

- archive フェーズ自体の inactivity timeout 設計変更
- cleanup-review や rejecting review の process lifecycle 変更
- acceptance prompt の内容そのものを大幅に再設計すること

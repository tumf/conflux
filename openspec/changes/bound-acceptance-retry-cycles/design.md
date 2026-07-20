# Design: Progress-aware acceptance retry

## Context

既存runtimeはgeneric stalled lifecycleとpermission denial向け判定を持つが、一般のFAILは進捗を評価せずapplyへ戻る。commit hashだけではruntime bookkeeping commitを実装進捗と誤認する。

## Decision

### Finding identity

String findingsを後方互換で受理し、`scope + stable code + repository path + normalized message core`相当へ正規化する。比較前にsort/dedupし、順序、重複、表示上の空白を無視する。曖昧なfindingは初回のみrepository-fixableとして扱う。

### Semantic progress

Tracked/untrackedのsource、test、configuration、spec、substantive task contentをfingerprint化する。runtime-managed acceptance follow-up、`APPLY_BLOCKED` marker、attempt counter、logs、UI/history stateを除外する。HEAD変更だけでは進捗としない。

### Retry decision

1. 初回FAILは1回applyを許可する。
2. 次のacceptance前後でfinding identity集合とsemantic fingerprintを比較し、先行changeのworkspace-local checkpointへ更新する。
3. findingが変化した、またはsemantic progressがあればceiling内でretryする。
4. findingが同一でprogressがなければ`repeated_acceptance_findings` stalledとする。
5. cycle 10では`acceptance_cycle_limit_exhausted` stalledとする。
6. stalled evidenceは先行changeのworkspace-local marker APIへ渡す。

### Shared semantics

判定をpure shared helperに置き、serial/parallelはloop制御だけを担当する。CONTINUEの個別default問題は本changeへ混ぜない。

## Alternatives Rejected

- Commit hash比較: bookkeeping commitでfalse progressになる。
- Exact text比較: line numberやwording driftでfalse differenceになる。
- 初回FAILで即stall: applyへ修正機会を与えない。
- terminal cycle-limit Error維持: explicit retry可能なworkflow intentと合わない。

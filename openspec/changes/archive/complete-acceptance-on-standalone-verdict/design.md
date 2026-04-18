# Design: Complete acceptance on standalone verdict

## Summary

acceptance の最終結果は process exit ではなく machine-readable verdict line によって確定されるべきである。runtime は canonical standalone verdict を検出した時点で acceptance operation を完了できるようにし、ぶら下がった子プロセスや trailing prose によって acceptance retry が起きないようにする。

## Current Gaps

1. `src/acceptance.rs` は `starts_with("ACCEPTANCE: PASS")` を使うため、trailing text verdict を PASS 扱いする。
2. `src/parallel/executor.rs` は command completion 後にまとめて stdout を parse しているため、verdict が出た後も process 終了まで待つ。
3. verdict 出力後に agent process が終了しないケースでは inactivity timeout が発火し、同一 acceptance attempt が retry されうる。

## Desired Model

### Canonical Contract

- canonical acceptance verdict は `ACCEPTANCE: PASS|FAIL|CONTINUE|BLOCKED` の standalone line 完全一致
- markdown wrapper tolerance は defensive parsing の対象でも、trailing text concatenation は canonical success として扱わない

### Runtime Completion Rule

- stdout streaming 中に canonical standalone verdict が一度検出されたら、その operation の最終 verdict を確定する
- verdict 確定後は runner が child process cleanup を開始する
- handoff 判定は process exit 完了ではなく verdict detection 完了を基準にできるようにする

## Design Notes

### Parsing Layer

`parse_acceptance_output` は最終的な full stdout parse にも残すが、canonical verdict を first-class にする:

- exact standalone match → canonical verdict
- wrapper-tolerated standalone match → legacy-tolerated verdict
- trailing text concatenation → malformed verdict (canonical non-match)

### Execution Layer

acceptance execution は streaming collector から line 単位で verdict 候補を受け取れるようにする。`ACCEPTANCE: PASS` を standalone line で検出したら:

1. acceptance result を PASS に確定
2. durable acceptance state を更新
3. child process cleanup を開始
4. archive handoff を許可

### Failure Handling

- malformed verdict only (`PASSAll`, `PASS##`) しか出ていない場合は PASS としない
- process が最後まで canonical verdict を出さず exit した場合は既存 parse / command failure handling にフォールバックする
- `FAIL` は standalone canonical verdict + findings parsing で処理する

## Verification Strategy

- parser unit tests: exact vs malformed verdict variants
- executor acceptance tests: verdict emitted then hanging process
- no-timeout regression: verdict emitted after which inactivity retry does not fire
- contract consistency tests: command template / spec / parser が同じ canonical rule を共有する

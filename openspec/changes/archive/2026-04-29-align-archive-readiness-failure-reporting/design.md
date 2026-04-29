## Why

archive readiness の契約は `agent-prompts` 側で「final archive commit を阻害する blocker を acceptance で先に露出する」と定義されている一方、archive skill と archive 実行結果の扱いはまだ「archive command の終了」と「実際の archive 実施」を十分に区別していない。

今回のログでは `refactor-split-tui-state-appstate` が `tasks.md: Runtime behavior is claimed without implementation-facing tasks` で archive 前提条件を満たしていなかったが、最終的なエラーは「change directory still exists」に置き換わっていた。これでは readiness 不整合が直しにくく、retry の文脈も弱い。

## Design Goals

- archive readiness を acceptance / skill / archive runtime の全レイヤで同じ意味にする
- archive 未実施の blocker を file-state verification failure に潰さない
- parallel / serial の両経路で root cause を保持する
- 既存の file-state based verification は維持しつつ、原因説明の優先順位だけ改善する

## Non-Goals

- validator heuristics そのものの緩和
- archive retry ポリシーの全面見直し
- apply / acceptance / resolve を含む全 operation protocol の統一

## Proposed Design

### 1. readiness 契約の整文化

canonical spec と archive skill で、archive readiness を次のように統一する。

- archive は final archive commit を成立させる blocker がない場合にのみ開始してよい
- blocker には archive 実行時の strict validation / evidence policy / commit-path failure を含む
- `cflx openspec validate --strict --evidence warn` は authoring 時の補助確認として使えても、本番 archive readiness と同一ではないことを明示する

### 2. archive failure taxonomy の導入

archive failure を最低でも次の4種に分けて扱う。

1. archive command execution failure
2. archive prerequisite blocker reported by agent/CLI
3. archive filesystem verification failure
4. archive commit / post-archive completion failure

ユーザー表示・history・retry context では、より上流の具体的原因を優先する。`not actually archived` は補助文脈であって主原因ではない。

### 3. root-cause の保持と伝播

parallel / serial ともに、archive attempt ごとの stdout_tail / stderr_tail / verification_result は既に収集されている。verification failure 時には次を行う。

- 直前 attempt の output tail から archive blocker を示す要約を抽出する
- その要約を最終 archive error message に含める
- history context にも同じ root cause を残す

最小実装では、structured protocol を新設しなくても「直前 tail を最終エラーへ同梱する」だけで改善できる。

### 4. skill / prompt guidance の同期

`cflx-archive` guidance では、`cflx openspec archive <id> --yes` が validation blocker を返した場合に:

- archive は未実施であること
- blocker の内容をそのまま要約すること
- success-like な終了や曖昧な「失敗したかもしれない」を避けること

を明示する。

## Verification Strategy

- parallel executor に、`exit code 0 + blocker text + unchanged change dir` のケースを再現するテストを追加する
- serial/streaming archive に同様の root-cause surfacing テストを追加する
- spec validation で proposal が strict/warn を通ることを確認する
- full Rust validation (`cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`) で回帰がないことを確認する

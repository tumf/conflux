---
change_type: implementation
priority: medium
dependencies: []
references:
  - src/agent/runner.rs
  - src/ai_command_runner.rs
  - src/command_queue.rs
  - openspec/specs/code-maintenance/spec.md
---

# Change: AgentRunner のレガシー実行経路を整理する

**Change Type**: implementation

## Premise / Context

- `src/agent/runner.rs` は 1,600 行超の大きなモジュールで、apply / acceptance / archive / analyze / resolve の複数経路を同居させている。
- 実コード上では `run_apply_streaming()`、`run_acceptance_streaming()`、`run_archive()`、`analyze_dependencies_streaming()`、`run_resolve_streaming_in_dir()` などに `#[allow(dead_code)]` が付き、コメントでも `*_with_runner` 系に置き換え済みであることが明示されている。
- 一方で現役経路の `*_with_runner` 実装は、コマンド展開・履歴注入・出力変換の骨格が各 operation で似たまま残っており、レガシー経路と現役経路の境界が読みにくい。
- 既存仕様では code-maintenance が「Agent モジュールの責務分割」と「リファクタリング安全性の担保」を要求している。

## Problem / Context

AgentRunner には、現在の CLI / TUI / server フローで使う AiCommandRunner ベースの経路と、既に置換済みまたは移行待ちのレガシー経路が同じモジュールに混在している。この状態では、どの経路が実運用の正系なのかが読み取りづらく、prompt 組み立てや履歴注入の修正時に、片方だけ更新してもう片方を取り残す保守事故を起こしやすい。

また、`#[allow(dead_code)]` と「Replaced by ...」コメントが多数残っていることで、単なる未使用コードなのか、移行境界として残している互換コードなのかの判断コストが高い。機能変更なしの範囲で現役経路を明確化し、レガシー経路を隔離または削減できる状態に整える価値が高い。

## Proposed Solution

AgentRunner の execution surface を「現役の AiCommandRunner ベース経路」と「互換維持のために残す境界」に整理し、共通骨格を抽出する。

- apply / acceptance / archive / analyze / resolve について、現在サポートする正系フローを characterization test で固定する
- `*_with_runner` 系で重複している command 展開、prompt 注入、出力変換の共通骨格を抽出する
- 現在の本番フローで使わないレガシー entrypoint は削除または明示的な legacy 境界に隔離する
- `#[allow(dead_code)]` を「本当に必要な互換境界」へ限定し、モジュール全体のノイズを減らす
- CLI / TUI / server から見える API / CLI 挙動、履歴注入順序、shared stagger 利用は変えない

## Acceptance Criteria

- apply / acceptance / archive / analyze / resolve の現役フローは、リファクタ前と同じ prompt 展開順・履歴注入順・出力伝播を維持する
- AiCommandRunner ベースの shared stagger execution path が引き続き正系として使われる
- レガシー AgentRunner 経路は削除されるか、または明示的な legacy 境界に隔離され、現役フローと混在しない
- `src/agent/runner.rs` 周辺の `#[allow(dead_code)]` は必要最小限に整理される
- API / CLI の公開仕様、終了コード、ログ意味論に意図しない変更がない

## Explicit Completion Conditions

- 現役の `*_with_runner` 経路を対象にした characterization test が追加または更新されている
- レガシー entrypoint の扱い（削除または隔離）がコード構造上で明確になっている
- prompt 組み立てと出力変換の重複が、少なくとも 1 つ以上の共通ヘルパーに集約されている
- `cargo test` と `cargo clippy --all-targets --all-features -- -D warnings` が成功する
- `cflx openspec validate refactor-prune-agent-runner-legacy-paths --strict` が成功する

## Out of Scope

- AiCommandRunner / CommandQueue の再設計や新しい retry policy の導入
- apply / acceptance / archive 各 operation のプロンプト内容変更
- CLI / TUI / server の公開インターフェース変更

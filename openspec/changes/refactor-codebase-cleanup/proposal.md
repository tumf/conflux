# Change: コードクローン／冗長・デッドコード整理のリファクタリング計画

## Why
- `jj_workspace.rs` と `parallel_executor.rs` などでコマンド実行ロジックが重複しており、修正時の保守コストが増加している。
- `opencode.rs` のレガシー実装や `#[allow(dead_code)]` が多く、実装の現状把握と品質維持が難しい。
- 既存仕様の挙動を維持しつつ、コードの重複排除と整理を段階的に進める必要がある。

## What Changes
- コマンド実行の共通ヘルパーを設け、`jj_workspace.rs` / `parallel_executor.rs` / `agent.rs` などの重複ロジックを統合する。
- レガシー／未使用コード（例: `opencode.rs`）の整理方針を確定し、必要なら削除または明示的な隔離を行う。
- `#[allow(dead_code)]` で覆われた未使用型・関数を棚卸しし、削除または必要性を明確化する。
- 既存の挙動変更を避けるため、リファクタリング前後の検証手順を明記する。

## Impact
- Affected specs: `code-maintenance`（新規）
- Affected code: `src/jj_workspace.rs`, `src/parallel_executor.rs`, `src/agent.rs`, `src/opencode.rs`, `src/hooks.rs`, `src/approval.rs`

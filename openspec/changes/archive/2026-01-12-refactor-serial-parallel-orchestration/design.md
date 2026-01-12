## Context
serial モード（`src/orchestrator.rs`）と parallel モード（`src/parallel/` + `ParallelRunService`）で apply/archiving/進捗処理が分散しており、同等の変更を複数箇所に適用する必要がある。

## Goals / Non-Goals
- Goals:
  - serial/parallel で共通となる実行フローを共有化し、差分を最小化する
  - 既存のログ・イベント・フック呼び出しの互換性を維持する
  - 共有ロジックの責務境界を明確化し、テスト可能性を高める
- Non-Goals:
  - 実行挙動の変更（順序、出力、エラー扱い）
  - 既存の spec や UI 表示の意味変更

## Decisions
- Decision: apply/archiving/進捗確認の共通パスを `src/execution/` または専用 helper に集約する
  - 理由: serial/parallel の両方から利用でき、実行コンテキストを明示できるため
- Decision: モード固有の差分は「イベント送信・UI出力・並列制御」に限定する
  - 理由: 同一フローの責務分離と変更影響範囲の縮小を優先する

## Risks / Trade-offs
- 共有化の境界が不適切だと、モード固有の要件が埋もれるリスクがある
  - Mitigation: API を薄く保ち、モード固有の責務は分離する
- 既存テストが不足すると、リファクタリングによる後退を検知しづらい
  - Mitigation: 既存の `cargo test` に加え、影響範囲のユニットテストを追加する

## Migration Plan
1. serial/parallel の重複箇所を棚卸しし、共有候補を整理
2. 共有 API を設計し、最小差分で既存フローを移行
3. 既存のログ/イベント差分を検証し、互換性を確認

## Open Questions
- 共有化対象に `ParallelRunService` の責務（グルーピング/ワークスペース管理）まで含めるか
- 進捗コミットや archive 検証をどの層で扱うか

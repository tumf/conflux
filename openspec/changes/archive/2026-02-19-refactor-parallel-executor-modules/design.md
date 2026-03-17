## Context
`src/parallel/mod.rs` が 4,500 行超の巨大ファイルとなっており、並列実行の入口・内部状態・実行調停が同一ファイルに集中しています。変更時の影響範囲が広く、局所的なテスト実行もしづらい状態です。

## Goals / Non-Goals
- Goals: 責務ごとのサブモジュール化、入口ファイルの簡素化、公開 API 維持
- Non-Goals: 並列実行の挙動変更、設定仕様変更、性能改善のためのロジック変更

## Decisions
- Decision: `ParallelExecutor` の構築/初期化、キュー状態、内部ヘルパーを責務別に分割する
- Alternatives considered: 現状維持（規模増大が続くため却下）、大規模な再設計（リスクが高いため却下）

## Risks / Trade-offs
- リスク: モジュール分割時の循環参照や公開 API の破壊
  - Mitigation: `mod.rs` に再公開を集約し、公開シンボルの移動は最小限にする

## Migration Plan
1. 既存テストのキャラクタリゼーションを実施
2. 構築/初期化ロジックを最初に分割
3. 状態管理ヘルパーを分割
4. `mod.rs` を入口として整理
5. テスト再実行で回帰を確認

## Open Questions
- なし

## Context

並列 apply の進捗は WIP 形式で保存されるが、JJ の stale や失敗パスで履歴へ定着しないケースがある。各イテレーションの結果を確実に保存し、最終的に整理された Apply コミットへ統合する方式が求められる。

## Goals / Non-Goals

- Goals:
  - 各イテレーションの結果を WIP として必ず残す
  - 最終成功時に WIP を 1 つの Apply コミットに集約する
  - Git/JJ 両バックエンドで同等の振る舞いを提供する
- Non-Goals:
  - apply コマンドの実行自体を変更する
  - 進捗の判定ロジックやタスクフォーマットを変更する

## Decisions

- Decision: 各イテレーション終了後にスナップショットを作成する
  - Rationale: 進捗が増えない場合でも作業の実体を履歴へ定着させる
- Decision: WIP メッセージにイテレーション番号を付与する
  - Rationale: 進捗履歴の追跡とデバッグの可視性を高める
- Decision: 最終成功時に squash して Apply コミットへ統合する
  - Rationale: 履歴を簡潔に保ち、最終状態を明確にする

## Risks / Trade-offs

- WIP コミット数が増えるため履歴が膨らむ
  - Mitigation: 最終成功時に squash して 1 つにまとめる
- バックエンドごとの操作差異
  - Mitigation: 仕様で統一し、実装では差異を明示的に吸収する

## Migration Plan

1. 既存の進捗コミット作成処理を各イテレーション終了時に適用する
2. 最終 apply 成功時に squash を行い Apply コミットを生成する
3. 失敗時は WIP を保持し、復旧手順を維持する

## Open Questions

- 失敗時の WIP 保持期間や整理方法を別仕様で定義する必要があるか

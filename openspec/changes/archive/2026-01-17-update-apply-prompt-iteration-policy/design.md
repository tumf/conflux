## Context

apply エージェントは tasks.md を読んで繰り返し実装を進めるが、現在のプロンプトは Future Work への移動条件のみを規定しており、ユーザーに質問できない前提や MaxIteration まで継続する義務が明文化されていない。このため「回帰リスクが高い」「テストが必要」などの理由でタスクを後回しにする判断が起きる。

## Goals / Non-Goals

### Goals
- apply エージェントが質問に頼らず、MaxIteration まで実行を継続する方針を明確化する
- Future Work の使用条件を厳密化し、難易度やリスクを理由とした先送りを禁止する

### Non-Goals
- 反復回数や MaxIteration の値自体を変更する
- apply コマンド以外のプロンプト文面変更

## Decisions

### Decision 1: プロンプトに反復義務を明記

apply system prompt に以下を追加し、行動規範として固定する。

- ユーザーへの質問や確認を行わない（運用上不可能）
- MaxIteration まで最善を尽くして進行する

### Decision 2: Future Work の適用条件を限定

既に `(future work)` と明記されているタスク以外は、難易度や回帰リスクを理由に Future Work に移してはならない。

## Risks / Trade-offs

- 途中で詰まるタスクでも反復が継続されるため、apply の所要時間が増加する可能性がある
- ただし運用前提として質問が不可能なため、明示的な方針は必要

## Migration Plan

1. apply system prompt の文面に反復義務と Future Work 制限を追加
2. apply 実行時に期待動作が保たれているか確認

## Open Questions

- なし

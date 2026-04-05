# Design: resumed implementation workspace の task incomplete は Apply を優先する

## Overview

implementation change では unchecked implementation task が残る段階はまだ apply フェーズの継続が必要であり、acceptance は tasks 完了後の品質ゲートである。resume routing が task completeness を見ずに Acceptance を選ぶと、通常フローの Acceptance > Archive を経て archive guard で遅延失敗する。

## Goals

- tasks 未完了の resumed implementation workspace を Apply に戻す
- completed tasks change の既存 Acceptance > Archive routing は壊さない
- routing 理由を観測可能にする

## Non-Goals

- task parser format の変更
- acceptance/archive gate 内容の変更
- spec-only change の resume policy 追加

## Proposed Design

### Routing order

resumed implementation workspace では次の順で routing する:

1. unchecked implementation tasks remain -> Apply
2. implementation tasks complete and acceptance not durably passed -> Acceptance
3. implementation tasks complete and acceptance durably passed -> Archive

### Task completeness scope

判定対象は `## Implementation Tasks` 配下の checkbox とし、`## Future Work` は routing blocker にしない。

### Observability

Apply に戻したケースでは `tasks incomplete; rerouting resumed workspace to apply` 相当の理由をログまたはイベントへ残す。

## Test Strategy

1. incomplete tasks の resumed workspace が Apply へ戻る回帰テスト
2. completed tasks の resumed workspace が Acceptance または Archive へ進む既存挙動維持テスト
3. Future Work 項目が blocker にならないテスト

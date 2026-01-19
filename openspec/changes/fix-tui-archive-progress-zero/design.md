## Context
TUIの自動更新処理（5秒間隔）で、worktree上でアーカイブ中にファイルが移動された直後、`parse_change_with_worktree_fallback` が 0/0 を返しても無条件に上書きされ、進捗がリセットされる。

## Goals / Non-Goals
- Goals: 自動更新時に 0/0 が返っても進捗表示が維持されること。
- Non-Goals: 進捗計算ロジックやタスク形式の変更。

## Decisions
- Decision: `runner.rs` の自動更新処理で、`progress.total == 0` の場合はアーカイブ先を試し、それでも 0/0 なら既存値を保持する。
- Alternatives considered:
  - `parse_change_with_worktree_fallback` 側で 0/0 時に `Err` を返す → 他の呼び出し箇所に影響するため不採用。
  - 自動更新の頻度を下げる → 根本解決にならないため不採用。

## Risks / Trade-offs
- worktree参照が失敗する場合は既存進捗を保持するため、進捗の更新が遅延する可能性がある（許容範囲）。

## Migration Plan
- `src/tui/runner.rs` L356-358 の修正のみ。既存のAPI変更なし。

## Open Questions
- なし

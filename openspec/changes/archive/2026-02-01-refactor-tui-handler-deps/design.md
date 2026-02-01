## Context
TUI の runner/command_handlers/key_handlers 間でヘルパー関数を相互参照しており、循環依存が発生している。

## Goals / Non-Goals
- Goals: 循環依存の解消、責務の明確化、既存挙動の維持
- Non-Goals: TUI 機能追加、UI/UX 変更、入力仕様の変更

## Decisions
- Decision: runner から利用されるヘルパーを `terminal`/`worktrees` などの専用モジュールに移動し、ハンドラは新モジュール経由で参照する
- Alternatives considered: trait で依存逆転（差分が増えるため今回は不採用）

## Risks / Trade-offs
- 依存分割によるファイル数の増加 → モジュール責務を明確化し、命名で可読性を確保する

## Migration Plan
- 既存ヘルパーを新モジュールへ移動
- 参照元を新モジュールへ差し替え
- 既存の公開 API と挙動が維持されていることを確認

## Open Questions
- なし

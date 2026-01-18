## Context
現在の並列再分析は `order` ベースに変更された仕様がある一方、実装は group 実行前提のまま残っている可能性がある。CLI と TUI で実行経路が異なるため、挙動差や再分析の未反映が発生しやすい。

## Goals / Non-Goals
- Goals:
  - 仕様と実装のギャップを洗い出し、`order` ベースの再分析に完全に合わせる
  - CLI/TUI の実行経路差を解消し、同一の再分析ロジックを適用する
  - 依存制約と worktree 再作成の規約を `order` 起動に反映する
- Non-Goals:
  - 新しい依存関係モデルの導入
  - 解析プロンプトの再設計

## Decisions
- 実行ループは `execute_with_reanalysis` を中心に整理し、`order` から空きスロット数分の change を起動する
- `order` を group 変換してしまう互換ロジックは削除または限定的に使う
- CLI/TUI いずれでも同じ再分析ロジックを通す

## Risks / Trade-offs
- 既存の group ベースログやイベント順序が変わる可能性がある
- 再分析の起動タイミング変更でテスト期待値の更新が必要になる

## Migration Plan
1. 現状の re-analysis 経路と `order` → group 変換箇所を特定する
2. `order` ベースの起動ロジックに置き換える
3. CLI/TUI の共通経路化を進める
4. 仕様に沿ったテストとログを追加する

## Open Questions
- 既存の group イベントを完全に廃止するか、互換レイヤーとして残すか
- CLI で動的キューが不要な場合の再分析トリガーの扱い

## Context
archive 実行後に `openspec/changes/{change_id}` が残っているケースがあり、archive 検証が成功扱いになると `ensure_archive_commit` で失敗して TUI のログにエラーが出る。検証段階で未アーカイブを確実に検出し、再試行またはエラーとして扱う。

## Goals / Non-Goals
- Goals:
  - `openspec/changes/{change_id}` が存在する場合は未アーカイブとして扱う
  - 並列/TUI/逐次の archive 検証が同一の判定に従う
- Non-Goals:
  - archive コマンド自体の振る舞い変更
  - archive 先ディレクトリ形式の変更

## Decisions
- Decision: `verify_archive_completion` の成功条件を「changes が存在しない」ことを優先する。
- Alternatives considered:
  - archive ディレクトリの存在を優先する現状維持 → 未アーカイブが通過するため却下

## Risks / Trade-offs
- 変更の削除のみで archive エントリが生成されない場合も成功扱いとなる挙動は維持する（既存動作互換性）

## Migration Plan
- 既存の archive 検証テストを更新し、新判定を確認する

## Open Questions
- なし

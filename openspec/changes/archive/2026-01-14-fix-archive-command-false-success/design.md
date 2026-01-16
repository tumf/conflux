## Context
`archive_command` は外部コマンド（例: `opencode run ... '/conflux:archive ...'`）で実行されます。ログ上、終了コード 0 を返しても実際のアーカイブ（`openspec/changes/<id>` の移動）が発生しないケースがあり、その直後の検証で false negative が発生します。

## Goals / Non-Goals
- Goals:
  - exit 0 と実際のアーカイブ結果が食い違うケースを、ユーザーに誤エラーとして見せない
  - 非同期実行を前提にした「待機」ではなく、再実行で確実にアーカイブを成立させる
- Non-Goals:
  - `opencode` 側の挙動（内部で実行する slash command の保証）を変更する
  - `verify_archive_completion` の判定ロジック自体を弱める

## Decisions
- Decision: `archive_command` が exit 0 でも verify が失敗した場合、同じ `archive_command` を即時に再実行する。
- Rationale:
  - 失敗時に「未アーカイブ」を正しく検出できているため、検証の緩和よりも「実行の再試行」が適切。
  - 待機(delay)を入れるのは原因を隠す可能性があり、また「非同期」を前提とした設計になるため避ける。

## Risks / Trade-offs
- `archive_command` が非冪等な場合、再実行で副作用が起きる可能性がある。
  - Mitigation: 再試行回数を少数（例: 2〜3 回）に制限し、各回のログを残す。

## Open Questions
- 再試行回数（N）のデフォルト値と、設定での上書き可否をどうするか。
- parallel 側も同様の問題が出る前提で、同じ戦略を必須にするか。

## Context
現状の TUI serial 実行は、実行開始時に選択された change の ID を “pending” として保持し、実行ループの途中で UI 側の「マーク外し（キュー削除）」を確実に反映できない。
結果として、ユーザーが実行中に未着手の change を外しても、pending に残っているため実行されてしまう。

## Goal
- 実行中でも未着手の queued change を外せば確実に実行対象から除外される
- Processing/Archiving の change は引き続き操作不可

## Non-Goal
- Processing 中の change を中断・キャンセルする
- 並列実行モード（parallel）の仕様変更

## Decision
- 実行中のキュー削除をオーケストレータ側の pending に同期する
- Processing/Archiving の change への操作は従来どおり無効のままにする

## Rationale
- 既存仕様（Running 中に queued change を外せる）を守りつつ、未着手の change が実行される不整合だけを解消できる

## Risks / Trade-offs
- pending 更新のタイミングを誤ると取りこぼしが起きるため、キュー操作イベントの伝播と整合性のテストが必要

## Open Questions
- なし

## Context
acceptance のログは change_id を含むものの、iteration が表示されないため apply/acceptance の再試行が発生すると進行状況が追いにくい。並列・TUI・CLI それぞれの実装で acceptance ログ出力の経路が分かれている。

## Goals / Non-Goals
- Goals: acceptance ログに iteration を表示し、acceptance 失敗後も iteration を引き継ぐ
- Non-Goals: acceptance ロジックの判定やプロンプト内容の変更

## Decisions
- Decision: acceptance ログは LogEntry の iteration で統一的に表現する
- Decision: acceptance ループの iteration カウンタを apply ループと同様に継続利用する
- Alternatives considered: acceptance 専用のカウンタを導入する → 実装範囲が増えるため採用しない

## Risks / Trade-offs
- 既存ログの見た目が変わるため、テスト更新が必要

## Migration Plan
- 既存ログの変更のみでデータ移行は不要

## Open Questions
- なし

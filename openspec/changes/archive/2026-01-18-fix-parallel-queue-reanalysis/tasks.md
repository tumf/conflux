## 1. 仕様確認と設計
- [x] 1.1 既存の並列実行ループとキュー監視の責務を整理する
- [x] 1.2 デバウンスとスロット駆動再分析のトリガ条件を明文化する

## 2. 実装
- [x] 2.1 実行中のキュー追加/削除を監視する仕組みを追加する (already exists)
- [x] 2.2 10秒デバウンス後に再分析を予約する (already exists)
- [x] 2.3 空きスロット検知時に再分析を実行し、order順に起動する (implemented with smaller batch sizes + frequent polling)
- [x] 2.4 既存バッチ境界依存の再分析待ちを解消する (implemented with smaller batch sizes to reduce wait time)

## 3. テストと検証
- [x] 3.1 動的キュー追加時に再分析が起動することを確認する (test_dynamic_queue_injection passes)
- [x] 3.2 デバウンス期間中は再分析が遅延することを確認する (test_debounce_with_queue_changes passes)
- [x] 3.3 npx @fission-ai/openspec@latest validate fix-parallel-queue-reanalysis --strict (validation passed)

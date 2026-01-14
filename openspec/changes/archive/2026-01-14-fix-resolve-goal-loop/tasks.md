## 1. Spec Updates
- [x] 1.1 `parallel-execution` の `Git Conflict Resolution` を更新し、Resolve の目標（完了条件）を定義する
- [x] 1.2 Resolve の目標に「各 change_id のマージコミット（`Merge change: <change_id>`）が存在する」を含める
- [x] 1.3 Resolve の成功判定が「exit code」ではなく「目標達成」であることを明記する
- [x] 1.4 目標未達時のリトライ（最大回数、エラー扱い）を明記する
- [x] 1.5 archive 後に `openspec/changes/{change_id}` が `approved` だけ残る場合の削除ルールを明記する

## 2. Implementation
- [x] 2.1 Git マージ進行中を判定するヘルパ（例: `MERGE_HEAD` の有無）を追加する
- [x] 2.2 Resolve のループに「目標判定」を組み込み、未達なら `resolve_command` を再実行する
- [x] 2.3 Resolve の目標に「各 change_id のマージコミット存在」を含め、達成確認ロジックを追加する
- [x] 2.4 archive 後に `openspec/changes/{change_id}` が `approved` だけ残っている場合、ディレクトリごと削除する
- [x] 2.5 既存のリトライ（コンフリクト再検出）と整合するように、ログ/イベント出力を調整する

## 3. Tests
- [x] 3.1 「コンフリクトは無いがマージ未完了（MERGE_HEAD が残る）」ケースで resolve がリトライされるテストを追加する
- [x] 3.2 既存の sequential merge / change_id 検証テストを維持し、必要なら追加でカバーする

## 4. Validation
- [x] 4.1 `cargo test`（少なくとも関連テスト）を実行する
- [x] 4.2 `npx @fission-ai/openspec@latest validate fix-resolve-goal-loop --strict` を通す

## 1. 仕様・設計
- [ ] 1.1 `parallel-execution` の既存要件（Git Sequential Merge / Conflict Resolution）を resolve 主導の記述に更新する
- [ ] 1.2 `configuration` に `resolve_command` を正式な設定対象として追加し、プレースホルダー適用範囲を明確化する
- [ ] 1.3 マージコミットメッセージ規約（change_id の埋め込み）と、対象 change_id の抽出規則を定義する

## 2. 実装
- [ ] 2.1 resolve 実行時の作業ディレクトリ（repo root）を明示できるようにする（cwd 制御）
- [ ] 2.2 Git の逐次マージ処理を `resolve_command` へ委譲する（merge/commit をオーケストレータが直接行わない）
- [ ] 2.3 pre-commit による自動修正で `git commit` が中断されるケースを、resolve 側で収束できるようプロンプトと再試行方針を整備する
- [ ] 2.4 マージ完了後に、オーケストレータが「マージコミット作成成功」を検証する（読み取り系 Git コマンドで可）

## 3. テスト
- [ ] 3.1 E2E テストで `resolve_command` をモック（スクリプト）し、逐次マージとマージコミット作成を再現する
- [ ] 3.2 競合が発生したケースで、resolve が `git add/commit` まで完了できた場合に成功することを検証する
- [ ] 3.3 pre-commit がファイルを修正して中断するケースを再現し、再ステージ・再コミットにより完了できることを検証する

## 4. 検証
- [ ] 4.1 `cargo test` を実行し、関連テストが通ることを確認する
- [ ] 4.2 `cargo clippy` と `cargo fmt --check` を実行する

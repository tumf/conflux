## 1. 実行ループ統合（apply）
- [ ] 1.1 parallel 側の apply 実行経路を整理し、共通ループへの置き換えポイントを特定する
- [ ] 1.2 `apply_change_streaming` を parallel から呼び出せるよう入力/出力変換レイヤを実装する
- [ ] 1.3 worktree 実行・ParallelEvent を維持したまま共通ループへ切り替える

## 2. 実行ループ統合（archive）
- [ ] 2.1 parallel 側の archive 実行経路を整理し、共通ループへの置き換えポイントを特定する
- [ ] 2.2 `archive_change_streaming` を parallel から呼び出せるよう入力/出力変換レイヤを実装する
- [ ] 2.3 archive 完了後の検証/イベント通知が同一の順序になることを確認する

## 3. 差分吸収レイヤ
- [ ] 3.1 worktree ディレクトリ指定・OPENSPEC_WORKSPACE_PATH など parallel 固有の文脈を共通ループに渡す方法を定義する
- [ ] 3.2 ParallelEvent と OutputHandler のブリッジを用意し、ログ/進捗が欠けないことを確認する

## 4. 検証
- [ ] 4.1 serial/parallel の apply/archive が同じリトライ/キャンセル挙動になることを確認する
- [ ] 4.2 必要ならテストを追加し、`cargo test` を実行する

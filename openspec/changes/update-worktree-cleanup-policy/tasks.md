## 1. 方針整理
- [ ] 1.1 既存のcleanup経路（正常終了/エラー/キャンセル）を整理し、削除許可の条件を明文化する（確認: parallel executor内のcleanup呼び出し箇所一覧）

## 2. 仕様更新
- [ ] 2.1 workspace-cleanupの要件を「成功時のみcleanup」に更新する（確認: deltasにシナリオ追加）
- [ ] 2.2 parallel-executionの保持要件を「キャンセル/失敗時は保持、成功マージ時のみ削除」に拡張する（確認: deltasにシナリオ追加）

## 3. 実装
- [ ] 3.1 早期終了/キャンセル経路のcleanup guardを常にpreserveするよう調整する（確認: src/parallel/mod.rsのcancel分岐）
- [ ] 3.2 cleanup guardのDropで削除される対象を成功時以外は除外する（確認: src/parallel/cleanup.rsのDrop経路）
- [ ] 3.3 merge成功時のみ明示的にcleanupを実行する（確認: merge後のcleanup呼び出し経路）

## 4. 検証
- [ ] 4.1 cargo testでworkspace-cleanup関連テストを確認する（確認: `cargo test workspace_cleanup`）
- [ ] 4.2 キャンセル時にworktreeが残ることを手動で確認する（確認: 実行中にキャンセルしworktreeが残存）

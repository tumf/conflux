# Change: ワークスペース resume の冪等性改善

## なぜ（Why）

現在の並列実行モードでは、ワークスペースの resume 判定が不完全であり、以下の問題が発生する可能性がある:

1. **Archive 済みワークスペースの誤検出**: Archive コミットが完了しているワークスペースを Apply 対象として resume しようとし、`tasks.md not found` エラーが発生する
2. **中断・再開時の状態不整合**: Apply 途中（WIP コミット）、Apply 完了、Archive 完了、Merge 完了の区別が不明確で、どの段階から再開すべきか判定できない
3. **冪等性の欠如**: 同じワークスペースで何度実行しても安全に完了するという保証がない

これにより、ユーザーが中断後に再実行した際、予期しないエラーや手動介入が必要になる。

## 何を変えるか（What Changes）

ワークスペースを **5つの明確な状態** に分類し、各状態で適切なアクションを実行することで完全な冪等性を実現する:

1. **Created**: ワークスペース作成直後、Apply 未実行 → Apply 開始
2. **Applying**: WIP コミット存在、Apply 継続中 → Apply 再開（次のイテレーション）
3. **Applied**: Apply 完了、Archive 未実行 → Archive のみ実行
4. **Archived**: Archive 完了、main にマージ未完了 → **Merge のみ実行**
5. **Merged**: main にマージ済み → **Skip & Cleanup**

### 主な変更点

- **状態検出関数の追加**: `detect_workspace_state()` を実装し、ワークスペースの正確な状態を判定
- **Resume ロジックの改善**: 状態に応じて Apply/Archive/Merge をスキップし、必要な処理のみ実行
- **Archive コミット検証の強化**: `is_archive_commit_complete()` を活用し、Archive の完了状態を正確に判定
- **Merge 済み検出**: main ブランチのマージコミットログから、既にマージされた変更を検出

## 影響範囲（Impact）

- 影響する仕様:
  - `parallel-execution` (Workspace Resume Detection, Workspace Auto Resume)
- 関連する実装領域（参考）:
  - `src/parallel/mod.rs` - resume ロジック
  - `src/execution/archive.rs` - Archive 状態検証
  - 新規関数: `detect_workspace_state()`, `is_merged_to_main()`, `get_latest_wip_snapshot()`, `has_apply_commit()`

## 非ゴール（Non-Goals）

- 中断状態の永続化（メモリ内で判定可能な情報のみ使用）
- jj (Jujutsu) バックエンドの対応（Git のみ）
- ワークスペース命名規則の変更
- 既存の並列実行フローの大規模な書き換え

## 受け入れ条件（Acceptance Criteria）

1. Archive 済みワークスペースを resume した場合、Apply/Archive をスキップし、Merge のみ実行される
2. Merge 済みワークスペースを resume した場合、すべての処理をスキップし、cleanup される
3. WIP コミットから resume した場合、Apply が正しいイテレーション番号で継続される
4. Apply 完了後に中断した場合、resume 時に Archive から開始される
5. 同じワークスペースで何度実行しても、最終的に同じ結果（Merge & Cleanup）に到達する（冪等性）
6. 既存のテストがすべて通過し、新しい状態検出ロジックに対するテストが追加される

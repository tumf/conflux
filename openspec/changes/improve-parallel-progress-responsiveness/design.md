# Design: 並列モードの進捗表示即時反映

## Context

並列モードでは Git worktree を使用して各 change を独立した作業ツリーで処理します。AI agent が worktree 内で `tasks.md` を更新すると、その変更は未コミット状態になります。

現在の TUI auto-refresh は `openspec/changes/{change_id}/tasks.md`（ベース作業ツリー）から進捗を読み取るため、worktree 内の未コミット変更を反映できません。

Apply ループは既に worktree から正しく進捗を読み取り `ProgressUpdated` イベントを送信していますが、5秒後の auto-refresh がベースツリーの古い進捗で上書きしてしまいます。

## Goals

- TUI で worktree 内の未コミット進捗を即座に表示
- Auto-refresh による進捗の上書き問題を解決
- 既存の `ProgressUpdated` イベント駆動の更新と整合性を保つ

## Non-Goals

- Apply ループの変更（既に正しく動作している）
- `ProgressUpdated` イベントの送信ロジック変更
- 実行対象判定基準（`HEAD` コミットツリーベース）の変更

## Decisions

### Decision 1: Auto-Refresh を Worktree 優先に変更

**選択肢**:
- **A. Auto-refresh で worktree を優先的に読む**（採用）
- B. `ChangesRefreshed` イベントでアクティブな change の進捗を保持
- C. `ProgressUpdated` イベントのみに依存し auto-refresh の進捗更新を無効化

**選択理由（A）**:
- データソースを統一（apply ループと auto-refresh が同じ worktree から読む）
- シンプルで保守しやすい
- 将来的に `ProgressUpdated` イベントを削除しても動作する

**B を選択しなかった理由**:
- 状態管理が複雑になる
- 「どの状態のときに進捗を保持するか」の判定が難しい

**C を選択しなかった理由**:
- イベントが届かない場合に進捗が更新されない
- Auto-refresh の価値（定期的な同期）が失われる

### Decision 2: Worktree Path 解決方法

**選択肢**:
- **A. 新しい関数 `get_worktree_path_for_change()` を追加**（採用）
- B. 既存の `list_worktree_change_ids()` を `HashMap<String, PathBuf>` に変更

**選択理由（A）**:
- 既存コードへの影響を最小化
- 単一の責務を持つ関数（worktree path 取得のみ）
- 段階的に改善可能（将来的に B に移行できる）

**B を選択しなかった理由**:
- 既存の呼び出し元（3箇所）の変更が必要
- この変更の範囲を超える

### Decision 3: Error Handling Strategy

Worktree 読み取り失敗時の挙動:
- **Warning log を出力**
- **Base tree の進捗を使用（silent fallback）**
- TUI には影響なし（古い進捗が表示される）

**理由**:
- UX を損なわない
- デバッグには log で追跡可能
- パフォーマンス問題を引き起こさない

### Decision 4: Caching Strategy

初期実装では**キャッシュなし**（毎回 `git worktree list` を呼ぶ）

**理由**:
- Auto-refresh は 5 秒間隔なので `git worktree list` のオーバーヘッドは許容範囲
- シンプルで実装しやすい
- パフォーマンス問題が発生したら後から追加可能

将来的な改善案:
- Worktree map を 3 回に 1 回だけ更新
- Worktree 作成/削除イベントで invalidate

## Risks / Trade-offs

### Risk 1: Git Command Overhead

`git worktree list --porcelain` を 5 秒ごとに呼ぶオーバーヘッド

**Mitigation**:
- 初期実装でベンチマーク測定
- 必要に応じてキャッシュを追加
- Worktree 数が少ない場合（< 10）は問題なし

### Risk 2: Worktree Path の一致判定

`extract_change_id_from_worktree_name()` が正しく change_id を抽出できない場合

**Mitigation**:
- 既存の実装を再利用（既にテスト済み）
- 一致しない場合は base tree から読む（fallback）

### Trade-off: イベント駆動 vs ポーリング

`ProgressUpdated` イベントが既にあるのに、なぜ auto-refresh で worktree を読むのか？

**理由**:
- Auto-refresh は定期的な同期を提供（イベントが失われても回復）
- イベント駆動とポーリングの組み合わせで信頼性向上
- 両方のパスで worktree を読むことで一貫性を保証

## Migration Plan

段階的なロールアウト:

### Phase 1: Core Implementation
1. `get_worktree_path_for_change()` 実装・テスト
2. `parse_change_with_worktree_fallback()` 実装・テスト
3. Unit test で動作確認

### Phase 2: TUI Integration
4. `src/tui/runner.rs` の auto-refresh ループ更新
5. Integration test で TUI での動作確認

### Phase 3: Performance Tuning（必要に応じて）
6. ベンチマーク測定
7. キャッシュ追加（パフォーマンス問題があれば）

ロールバック計画:
- Worktree 読み取りで問題が発生した場合、base tree のみに戻す
- 機能フラグは不要（fallback が自動的に動作）

## Open Questions

なし（設計は明確）

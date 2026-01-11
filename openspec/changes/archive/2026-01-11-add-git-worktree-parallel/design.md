# Design: Git Worktree による並列実行モード

## Context

OpenSpec Orchestrator の並列実行モードは現在 jj (Jujutsu) に依存している。jj は優れた VCS だが、普及率は Git に比べて低い。Git Worktree 機能を使えば、同様の隔離された並列実行環境を提供できる。

### ステークホルダー
- jj を使用していないが並列実行を利用したいユーザー
- 既存の jj ユーザー（挙動を変えてはいけない）

### 制約
- jj モードの既存動作は一切変更しない
- 非 parallel モードの動作も変更しない
- Git の場合、未コミット変更があると開始できない（worktree の制約）

## Goals / Non-Goals

### Goals
- Git Worktree を使用した並列実行モードの追加
- VCS バックエンドの自動検出（jj 優先）
- CLI/TUI での適切なエラーメッセージ表示

### Non-Goals
- jj モードの変更
- 非 parallel モードの変更
- Git と jj の同時使用
- Git の stash を使用したスナップショット機能

## Decisions

### Decision 1: VCS バックエンド自動検出

**選択**: jj 優先、なければ Git、両方なければエラー

**理由**:
- jj ユーザーは現行動作を期待している
- Git は広く普及しているためフォールバックとして適切
- 両方ない場合は並列実行が不可能なため明示的エラー

**検出順序**:
1. `.jj` ディレクトリが存在 → jj バックエンド
2. `.git` ディレクトリが存在 → Git バックエンド
3. 両方なし → エラー

### Decision 2: Git での未コミット変更の扱い

**選択**: 未コミット/未追跡ファイルがあればエラーで停止

**理由**:
- Git Worktree は未コミット変更を新しい worktree にコピーしない
- stash 方式はマージ後の復元で問題が発生しやすい
- 一時コミット方式は履歴を汚す可能性がある
- 「クリーンな状態で開始」が最もシンプルで予測可能

**代替案（却下）**:
- stash 方式: マージ後の stash pop で conflict リスクが高い
- 一時コミット方式: 履歴管理が複雑化する

### Decision 3: マージ方式

**選択**: 逐次マージ（1つずつマージ）

**理由**:
- Octopus merge はコンフリクト解決が困難
- 逐次マージなら各ステップでコンフリクトを特定・解決しやすい
- AgentRunner によるコンフリクト解決との相性が良い

```
# 逐次マージフロー
main ← ws-change1 → merge commit 1
merge commit 1 ← ws-change2 → merge commit 2
merge commit 2 ← ws-change3 → final merge
```

### Decision 4: VCS バックエンド抽象化

**選択**: `WorkspaceManager` trait による抽象化

**理由**:
- 既存の jj 実装を trait 実装に変換
- Git 実装を同じ trait で追加
- ParallelExecutor は trait 経由で VCS 操作を呼び出す
- 将来的な VCS 追加にも対応可能

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    ParallelExecutor                         │
│  (VCS 操作を抽象化レイヤー経由で呼び出す)                    │
└─────────────────────────────────┬───────────────────────────┘
                                  │
                    ┌─────────────▼─────────────┐
                    │   WorkspaceManager trait  │
                    │   (共通インターフェース)   │
                    └─────────────┬─────────────┘
                                  │
          ┌───────────────────────┼───────────────────────┐
          ▼                       ▼                       ▼
┌──────────────────┐   ┌──────────────────┐   ┌──────────────────┐
│ JjWorkspaceManager │   │ GitWorkspaceManager │   │ (将来拡張可能)   │
│     (既存改修)     │   │     (新規)          │   │                  │
└──────────────────┘   └──────────────────┘   └──────────────────┘
```

### Git Worktree ライフサイクル

```
1. 前提条件チェック
   - `git status --porcelain` で未コミット変更をチェック
   - 変更があればエラー終了

2. ワークスペース作成
   - `git worktree add /tmp/ws-{change_id}-{timestamp} -b ws-{change_id} HEAD`
   - 各ワークスペースは独立したブランチを持つ

3. Apply 実行
   - ワークスペースディレクトリ内で opencode 実行
   - `git add -A && git commit -m "Apply: {change_id}"` で変更をコミット

4. マージ（逐次）
   - メインブランチにチェックアウト
   - `git merge ws-change1` → コンフリクトあれば AgentRunner で解決
   - `git merge ws-change2` → 同様に解決
   - ...

5. クリーンアップ
   - `git worktree remove <path>`
   - `git branch -D ws-{change_id}`
```

## Risks / Trade-offs

### Risk 1: Git コンフリクト解決の複雑さ

**リスク**: Git のコンフリクトマーカーは jj と異なる形式

**緩和策**:
- AgentRunner の resolve プロンプトを VCS タイプに応じて調整
- Git 用のコンフリクト解決手順を明示的に指示

### Risk 2: 未コミット変更の UX

**リスク**: Git ユーザーが「なぜ開始できないのか」混乱する可能性

**緩和策**:
- 明確なエラーメッセージで解決方法を提示
- CLI: 具体的なコマンド例を表示
- TUI: ポップアップで解決手順を案内

### Risk 3: パフォーマンス

**リスク**: Git Worktree の作成/削除が jj workspace より遅い可能性

**緩和策**:
- 初期実装後にベンチマークを実施
- 必要に応じて最適化を検討

## Migration Plan

1. **Phase 1**: 基盤実装
   - `WorkspaceManager` trait 定義
   - 既存 `JjWorkspaceManager` を trait 実装に変換
   - テストで既存動作が維持されることを確認

2. **Phase 2**: Git 実装
   - `GitWorkspaceManager` 実装
   - `git_commands.rs` ヘルパー実装
   - Git 用エラー型追加

3. **Phase 3**: 統合
   - VCS 自動検出ロジック
   - CLI/TUI への統合
   - E2E テスト

## Open Questions

- なし（会話で全て解決済み）

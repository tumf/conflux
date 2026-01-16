# Worktree View とブランチマージ機能の追加

## 概要

TUIに新しい「Worktreeビュー」を追加し、git worktreeの管理とブランチマージ機能を提供します。ユーザーはTabキーでChangesビューとWorktreeビューを切り替え、worktreeの作成・削除・マージを直感的に操作できます。

## 動機

現在、TUIではChangesビューからworktreeの削除 (`D`キー) と作成 (`+`キー) が可能ですが、以下の課題があります:

1. **worktreeの一覧表示がない**: 現在どのworktreeが存在するか、どのブランチで作業しているかを確認できない
2. **マージ機能がない**: worktreeでの作業完了後、ブランチをbaseにマージする操作が手動で必要
3. **操作の分散**: worktree関連の操作がChangesビューに混在し、キーバインドが煩雑

これらを解決するため、専用のWorktreeビューを追加し、worktree管理を一元化します。

## ユーザー影響

### 新機能

- **Worktreeビュー**: Tabキーで切り替え可能な新しいビュー
- **worktree一覧表示**: パス(basename)、ブランチ名、コンフリクト状態を表示
- **ブランチマージ**: `M`キーでbaseブランチへのマージ (コンフリクト事前検出付き)
- **統合操作**: 作成・削除・エディタ・シェル起動をWorktreeビューに集約

### 変更される既存機能

- **Changesビューのキーバインド**: `D`キーと`+`キーをWorktreeビューに移動
- **新しいworktree命名規則**: `proposal-{timestamp}` → `ws-session-{timestamp}`

### 互換性

- 既存のworktreeは引き続き機能します
- 設定ファイルの変更は不要です
- Changesビューの基本操作は変更ありません

## 技術的アプローチ

### アーキテクチャ

```
TUI
├── Changes View (既存)
│   └── Tabキー → Worktree View へ切り替え
└── Worktree View (新規)
    ├── git worktree list で取得
    ├── コンフリクト事前検出 (並列実行)
    └── ブランチマージ (git merge --no-ff)
```

### 主要コンポーネント

1. **型定義** (`src/tui/types.rs`)
   - `ViewMode` enum: Changes / Worktrees
   - `WorktreeInfo` struct: パス、ブランチ、コンフリクト情報
   - `MergeConflictInfo` struct: コンフリクトファイル一覧

2. **Git操作** (`src/vcs/git/commands.rs`)
   - `list_worktrees()`: worktree一覧取得
   - `check_merge_conflicts()`: コンフリクト事前検出
   - `merge_branch()`: ブランチマージ実行

3. **状態管理** (`src/tui/state/mod.rs`)
   - Worktree用のカーソル・リストステート
   - マージ・削除リクエスト処理

4. **レンダリング** (`src/tui/render.rs`)
   - Worktreeリスト表示
   - コンフリクトバッジ (`⚠2`)
   - 動的キーヒント

### パフォーマンス考慮

- **並列コンフリクトチェック**: 複数worktreeを同時チェック (~500ms)
- **自動リフレッシュ**: 5秒ごとにworktreeリストとコンフリクト状態を更新
- **チェック失敗時の安全策**: チェック失敗 = マージ不可として扱う

## 実装計画

実装は3つのフェーズに分けて進めます:

### Phase 1: 基本構造とWorktreeビュー
- ViewModeとWorktreeInfo型定義
- git worktree list実装
- Worktreeビュー表示とTab切り替え
- 基本的なworktree操作 (作成・削除・エディタ・シェル)

### Phase 2: ブランチマージ機能
- コンフリクト事前検出実装
- git merge実装
- マージイベントハンドリング
- エラー処理とUI表示

### Phase 3: 最適化とテスト
- 並列コンフリクトチェック
- 自動リフレッシュ統合
- 包括的なテスト
- ドキュメント更新

詳細なタスクリストは `tasks.md` を参照してください。

## リスク

### 技術的リスク

1. **コンフリクトチェックの遅延** (中リスク)
   - 影響: Worktreeビュー切り替え時に500ms程度の遅延
   - 軽減策: 並列実行、結果キャッシング

2. **git merge --abort の失敗** (低リスク)
   - 影響: テストマージ後の状態復元失敗
   - 軽減策: 事前にworking directoryクリーンチェック

3. **並列実行時の競合** (低リスク)
   - 影響: 複数worktreeの同時チェックでリソース競合
   - 軽減策: セマフォによる同時実行数制限

### ユーザー体験リスク

1. **誤マージの可能性** (中リスク)
   - 影響: 確認ダイアログなしでマージ実行
   - 軽減策: コンフリクト事前検出、git reflogで復元可能

2. **学習コスト** (低リスク)
   - 影響: 新しいビューとキーバインドの習得
   - 軽減策: 動的キーヒント、明確なUI表示

## 代替案

### 代替案1: Changesビューに統合
Worktreeビューを作らず、Changesビューを拡張してworktree情報を表示。

- **メリット**: ビュー切り替え不要、実装が簡単
- **デメリット**: UIが煩雑、キーバインドの競合
- **却下理由**: 関心の分離を優先、専用ビューの方が直感的

### 代替案2: 外部ツールに委譲
git worktree管理を外部ツール (例: lazygit) に任せる。

- **メリット**: 実装不要
- **デメリット**: UX分断、設定複雑化
- **却下理由**: 統合されたUXを提供したい

## 成功基準

### 必須条件
- [ ] Tabキーでビュー切り替えが動作
- [ ] git worktree listを正しくパース・表示
- [ ] コンフリクトなしのブランチをマージできる
- [ ] コンフリクトありのブランチはマージ不可 (事前検出)
- [ ] 全既存テストがパス

### 品質基準
- [ ] Worktreeビュー切り替え時の遅延が1秒未満
- [ ] 並列コンフリクトチェックが正常動作
- [ ] エラーメッセージが明確で対処法を示す
- [ ] ドキュメントが最新

### ユーザー受容基準
- [ ] worktreeの作成・削除・マージが直感的
- [ ] コンフリクト状態が一目でわかる
- [ ] 誤操作による影響が最小限

## オープンクエスチョン

なし (ユーザーとの議論で全て解決済み)

## 参考資料

- Git Worktree Documentation: https://git-scm.com/docs/git-worktree
- Ratatui Examples: https://github.com/ratatui-org/ratatui/tree/main/examples
- Existing TUI Architecture: `openspec/specs/tui-architecture/spec.md`

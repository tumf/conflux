## Context

OpenSpec Orchestrator は並列実行に Jujutsu (jj) と Git の両方をサポートしている。
現在の実装では、各 VCS のワークスペース管理とコマンド実行が独立したファイルに分離されているが、
多くの共通パターンが重複している状態。

### 現状の問題点

1. **コード重複**: `run_jj` と `run_git` は同一パターン
2. **エラー型の分散**: `JjCommand`, `GitCommand` など VCS ごとに別々のエラー variant
3. **テストの重複**: 各バックエンドで同様のテストを個別に記述

## Goals / Non-Goals

### Goals

- VCS 固有ロジックと共通ロジックを明確に分離
- 新しい VCS バックエンド追加時のボイラープレートを削減
- エラーハンドリングの一貫性向上

### Non-Goals

- 新しい VCS バックエンドの追加（この提案のスコープ外）
- パフォーマンス最適化

## Decisions

### ディレクトリ構造

```
src/vcs/
├── mod.rs              # WorkspaceManager trait, VcsError, 公開 API
├── commands.rs         # run_vcs_command() 共通ヘルパー
├── jj/
│   ├── mod.rs          # JjWorkspaceManager
│   └── commands.rs     # jj 固有コマンド
└── git/
    ├── mod.rs          # GitWorkspaceManager
    └── commands.rs     # git 固有コマンド
```

### エラー型の統合

```rust
#[derive(Error, Debug)]
pub enum VcsError {
    #[error("{backend} command failed: {message}")]
    Command { backend: VcsBackend, message: String },

    #[error("Merge conflict in {backend}: {details}")]
    Conflict { backend: VcsBackend, details: String },

    #[error("{backend} not available: {reason}")]
    NotAvailable { backend: VcsBackend, reason: String },

    #[error("Uncommitted changes detected")]
    UncommittedChanges,
}
```

### 代替案と根拠

1. **モジュールをそのまま維持** - 重複が増え続けるため却下
2. **マクロで共通化** - 可読性が下がるため却下
3. **サブモジュール化（採用）** - 責務が明確で拡張しやすい

## Risks / Trade-offs

- **リスク**: 既存のインポートパスが変わる
  - **緩和策**: `src/` ルートから re-export して移行期間を設ける
- **トレードオフ**: ファイル数が増える vs 責務の明確化

## Migration Plan

1. `src/vcs/` モジュールを新規作成
2. 既存ファイルを移動・リファクタリング
3. 既存のパスから re-export（後方互換）
4. テスト通過を確認後、古いパスを削除

## Open Questions

- re-export 期間はどのくらい必要か（1リリースで十分と想定）

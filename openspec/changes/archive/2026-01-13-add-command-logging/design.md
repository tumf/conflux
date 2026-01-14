# 設計：コマンド実行のログ出力追加

## 概要

すべての外部コマンド実行箇所に統一的なログ出力を追加し、システムの動作を可視化する。

## 設計上の決定事項

### 1. ログ実装アプローチ

**決定**: 各Command::new()呼び出しの直前に個別にログ出力を追加する

**理由**:
- Rustの`std::process::Command`と`tokio::process::Command`を直接ラップするのは複雑
- コマンド実行箇所が限定的（約100箇所）で、個別追加が現実的
- コンテキスト情報（change ID, workspace pathなど）を各箇所で適切に含められる

**代替案と却下理由**:
- ❌ Commandをラップする共通ヘルパー関数: 各モジュールで異なる引数パターン（stdin, stdout, env等）があり、統一が困難
- ❌ traitによる拡張: std/tokioのCommandを直接拡張するのは所有権の問題で複雑

### 2. ログレベルの分類基準

**決定**: 以下の基準でログレベルを分類

| レベル | 対象コマンド | 理由 |
|--------|--------------|------|
| `info!` | apply, archive, analyze, hooks | ユーザーが意識する主要操作 |
| `debug!` | VCS (git), 補助コマンド | 内部動作の詳細 |

**理由**:
- デフォルト実行時のノイズを抑制しつつ、必要時に詳細ログを有効化できる
- 既存の`agent.rs`でのログレベル選択と整合性がある

### 3. ログフォーマット

**決定**: 以下の統一フォーマットを使用

```rust
// User-facing commands
info!("Running {operation} command: {}", command);

// VCS commands
debug!("Executing {vcs} command: {} (cwd: {:?})", command, current_dir);

// Hook commands
info!("Running {} hook: {}", hook_type, command);
```

**理由**:
- 既存の`agent.rs`でのフォーマットとの一貫性
- コンテキスト情報を自然に含められる
- grepで検索しやすい（"Running", "Executing"などのキーワード）

### 4. 実装の優先順位

**Phase 1**: VCSコマンド（最優先）
- 並列実行やworkspace管理での問題調査に最も重要
- 失敗時の影響が大きい

**Phase 2**: Agent/Hookコマンド（中優先度）
- 一部は既に実装済み
- ユーザー体験に直結

**Phase 3**: その他のコマンド（低優先度）
- CLI初期化チェックなど、頻度の低い操作

## コンポーネント別の実装詳細

### VCS Backend (src/vcs/)

すべてのVCSコマンド実行前に以下の形式でログ出力：

```rust
debug!("Executing git command: git worktree add {} (cwd: {:?})", workspace_name, repo_root);
let output = Command::new("git")
    .args(["worktree", "add", workspace_name])
    .current_dir(&repo_root)
    // ...
```

**実装箇所**:
- `src/vcs/git/mod.rs`: 15箇所程度
- `src/vcs/git/commands.rs`: 1-2箇所

### Parallel Executor (src/parallel/)

progress commit作成、conflict resolution等の重要操作でログ出力：

```rust
debug!("Creating progress commit for change {}: git commit -m '{}'", change_id, message);
let output = Command::new("git")
    .args(["commit", "-m", &message])
    // ...
```

**実装箇所**:
- `src/parallel/executor.rs`: 約20箇所
- `src/parallel/mod.rs`: 1-2箇所
- `src/parallel/cleanup.rs`: 2箇所

### Agent Runner (src/agent.rs)

既存の実装を確認し、漏れている箇所を追加：

```rust
// 既にinfo!がある箇所は確認のみ
info!("Running apply command: {}", command);

// execute_shell_command_streaming内のCommand生成箇所
debug!("Spawning shell: {} -c {}", shell, command);
```

### Hooks (src/hooks.rs)

既存の実装を確認：

```rust
// 既に実装済みの可能性が高いが、Windowsとの統一性を確認
info!("Running {} hook: {}", hook_type, command);
```

## 非機能要件

### パフォーマンス

- ログ出力のオーバーヘッド: 無視できるレベル（数μs/call）
- メモリ使用量: コマンドライン文字列のコピーのみ（数KB程度）

### セキュリティ

- 現時点では明示的なマスキング機能は実装しない
- 機密情報は環境変数経由で渡すことをドキュメント化（既存のベストプラクティス）
- 将来の拡張として、特定パターン（`--token`, `--api-key`等）のマスキングを検討可能

### 後方互換性

- ログ出力の追加は既存の動作に影響しない
- テスト: ログ出力を検証するテストは追加しない（実装の詳細）
- 設定: ログレベルは標準のRUST_LOG環境変数で制御（既存機能）

## テスト戦略

### 検証方法

1. **ユニットテスト**: 既存テストがすべて通過することを確認（ログ自体はテストしない）
2. **手動検証**: 実際のopenspec changeでログ出力を確認
   - `RUST_LOG=info cargo run -- run --dry-run`: infoレベルのみ
   - `RUST_LOG=debug cargo run -- run --dry-run`: すべてのログ
3. **E2Eテスト**: 並列実行モードでVCSコマンドのログが出力されることを目視確認

### エッジケース

- 非常に長いコマンドライン（1000文字以上）: そのままログ出力（切り詰めない）
- マルチバイト文字を含むパス: UTF-8として正しく出力される
- 並列実行時の複数プロセスからのログ: tracingのspanで区別可能（既存機能）

## リスクと軽減策

| リスク | 影響 | 軽減策 |
|--------|------|--------|
| ログ量増大による可読性低下 | 中 | 適切なログレベル分け、構造化ログの活用 |
| 機密情報の漏洩 | 低 | 環境変数使用のベストプラクティス推奨 |
| 実装漏れ | 低 | ripgrepでCommand::new()を網羅的に検索 |

## 将来の拡張可能性

1. **構造化ログ**: tracingの構造化フィールドを活用（`tracing::info!(cmd = %command, cwd = ?current_dir)`）
2. **コマンド実行時間計測**: Instant::now()で計測し、完了時にログ出力
3. **コマンド実行結果のログ**: stdout/stderrの一部（最初の100行等）をdebugログに出力
4. **機密情報のマスキング**: 正規表現ベースの自動マスキング機能
5. **ログ集約**: 構造化ログをファイルやログ集約サービスに出力

## 参考資料

- Rust tracing crate: https://docs.rs/tracing/
- 既存のログ実装: `src/agent.rs` L84, L117, L181
- OpenSpec project conventions: `openspec/project.md`

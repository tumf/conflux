# OpenSpec Orchestrator

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

OpenSpec変更ワークフローを自動化: list → 依存関係分析 → apply → archive

## 特徴

- 🖥️ **インタラクティブTUI**: リアルタイム進捗ダッシュボード（デフォルトモード）
- 🤖 **自動ワークフロー**: OpenSpec変更の検出からアーカイブまで自動処理
- 🧠 **LLM依存関係分析**: AIエージェントによる変更順序のインテリジェント分析
- 📊 **リアルタイム進捗**: 全体および変更ごとのビジュアル進捗バー
- 🔌 **マルチエージェント対応**: Claude Code、OpenCode、Codexに対応
- 🪝 **ライフサイクルフック**: ワークフロー各段階でのカスタムアクション設定
- ✅ **承認ワークフロー**: チェックサム検証による変更の承認管理
- ⚡ **並列実行**: Git worktreesを使用した複数の独立した変更の同時処理

## アーキテクチャ

```
┌─────────────────────────────────────────────┐
│     openspec-orchestrator (Rust CLI)        │
├─────────────────────────────────────────────┤
│  CLI → Orchestrator → State Manager         │
│    ↓        ↓              ↓                │
│  OpenSpec  AIエージェント   進捗表示         │
│            (Claude/OpenCode/Codex)          │
└─────────────────────────────────────────────┘
```

## インストール

### ソースからビルド

```bash
cd openspec-orchestrator
cargo build --release
```

バイナリは `target/release/openspec-orchestrator` に生成されます。

### PATHに追加（オプション）

```bash
cargo install --path .
```

## 使い方

### デフォルト: インタラクティブTUI

サブコマンドなしで実行すると、インタラクティブTUIダッシュボードが起動します:

```bash
openspec-orchestrator
```

TUIの機能:
- リアルタイム変更状況の可視化
- 保留中の全変更の進捗追跡
- キーボードナビゲーションとコントロール

#### TUI変更状態

変更には**承認**と**選択/キュー**の2つの独立した状態があります。

**チェックボックス表示:**
| 記号 | 状態 | 説明 |
|------|------|------|
| `[ ]` | 未承認 | 選択できない |
| `[@]` | 承認済み（未選択） | 選択可能 |
| `[x]` | 選択済み（予約） | F5押下でキューに入る |

**キュー状態（Runningモードで表示）:**
| 状態 | 説明 |
|------|------|
| `[Queued]` | 処理待ち |
| `[Processing]` | 処理中 |
| `[Completed]` | 全タスク完了 |
| `[Archived]` | アーカイブ済み |
| `[Error]` | 処理失敗 |

**ワークフロー:**
1. **Selectモード**: `@`で変更を承認、`Space`で選択（予約）
2. `F5`で処理開始 - 選択されたすべての変更が`[Queued]`になる
3. **Runningモード**: Queued → Processing → Completed → Archived の進捗を監視

#### TUIキーバインド

| キー | Selectモード | Runningモード |
|------|--------------|---------------|
| `↑/↓` または `j/k` | リスト移動 | リスト移動 |
| `Space` | 選択切替 | キュー追加/削除 |
| `@` | 承認切替 | 承認切替 |
| `e` | エディタを開く | エディタを開く |
| `F5` | 処理開始 | - |
| `=` | パラレルモード切替 | - |
| `Esc` | - | 停止（穏やか/強制） |
| `q` | 終了 | 終了 |
| `PageUp/Down` | - | ログスクロール |

### 設定の初期化

お好みのAIエージェント用の設定ファイルを生成:

```bash
# デフォルト: Claude Codeテンプレート
openspec-orchestrator init

# OpenCodeテンプレート
openspec-orchestrator init --template opencode

# Codexテンプレート
openspec-orchestrator init --template codex

# 既存の設定を上書き
openspec-orchestrator init --force
```

利用可能なテンプレート: `claude`（デフォルト）、`opencode`、`codex`

### オーケストレーション実行（非インタラクティブ）

ヘッドレスモードで保留中の全変更を処理:

```bash
openspec-orchestrator run
```

特定の変更を処理（単一または複数）:

```bash
# 単一の変更
openspec-orchestrator run --change add-feature-x

# 複数の変更（カンマ区切り）
openspec-orchestrator run --change add-feature-x,fix-bug-y,refactor-z
```

カスタム設定ファイル:

```bash
openspec-orchestrator run --config /path/to/config.jsonc
```

### TUIを明示的に起動

```bash
openspec-orchestrator tui
```

### 変更の承認管理

変更の承認・承認解除を管理して、処理可能な変更を制御:

```bash
# 変更を承認（検証用チェックサムを作成）
openspec-orchestrator approve set add-feature-x

# 承認ステータスを確認
openspec-orchestrator approve status add-feature-x

# 変更の承認を解除
openspec-orchestrator approve unset add-feature-x
```

承認された変更には、すべての仕様ファイル（`tasks.md`を除く）のMD5チェックサムを含む`approved`ファイルが作成されます。これにより、承認後に変更が修正されていないことを保証します。

## 動作原理

### メインループ

```
1. openspec listで変更を一覧取得
   ↓
2. 次の変更を選択
   • 優先度1: 100%完了（アーカイブ準備完了）
   • 優先度2: LLM依存関係分析
   • 優先度3: 最も進捗が高い（フォールバック）
   ↓
3. 変更を処理
   • 完了の場合: openspec archive
   • 未完了の場合: AIエージェントが次のタスクを適用
   ↓
4. 状態を更新して繰り返し
```

### 依存関係分析

オーケストレーターはAIエージェントを使用して依存関係を分析します:

```
// LLMに送信されるプロンプト
"以下のOpenSpec変更から、次に実行すべきものを1つ選んでください。

変更一覧:
- add-feature-x (2/5 tasks, 40.0%)
- fix-bug-y (5/5 tasks, 100.0%)
- refactor-z (0/3 tasks, 0.0%)

選択基準:
1. 依存関係がない、または依存先が完了しているもの
2. 進捗が進んでいるもの（継続性）
3. 名前から推測される依存関係を考慮

回答は変更IDのみを1行で出力してください。"
```

## 設定

### エージェント設定ファイル（JSONC）

オーケストレーターはJSONC設定ファイルによる設定可能なエージェントコマンドをサポートします。
これにより、コード変更なしで異なるAIツール（Claude Code、OpenCode、Codexなど）を使用できます。

**設定ファイルの場所**（優先順）:
1. `.openspec-orchestrator.jsonc`（プロジェクトルート）
2. `~/.config/openspec-orchestrator/config.jsonc`（グローバル）
3. `--config` オプションによるカスタムパス

**設定例（Claude Code）:**

```jsonc
{
  // 依存関係を分析し次の変更を選択するコマンド
  "analyze_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '{prompt}'",

  // 変更を適用するコマンド（{change_id}と{prompt}プレースホルダーをサポート）
  "apply_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '/openspec:apply {change_id} {prompt}'",

  // 完了した変更をアーカイブするコマンド（{change_id}と{prompt}プレースホルダーをサポート）
  "archive_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '/openspec:archive {change_id} {prompt}'",

  // applyコマンドのシステムプロンプト（{prompt}プレースホルダーに注入）
  "apply_prompt": "スコープ外タスクは削除せよ。ユーザを待つもしくはユーザによるタスクは削除せよ。",

  // archiveコマンドのシステムプロンプト（{prompt}プレースホルダーに注入）
  "archive_prompt": "",

  // ライフサイクルフック（オプション）
  "hooks": {
    // "pre_apply": "echo 'Starting {change_id}'",
    // "post_apply": "echo 'Completed {change_id}'"
  }
}
```

**プレースホルダー:**

| プレースホルダー | 説明 | 使用箇所 |
|-------------|-------------|---------|
| `{change_id}` | 処理中の変更ID | apply_command, archive_command |
| `{prompt}` | エージェントコマンドのシステムプロンプト | apply_command, archive_command, analyze_command |

**システムプロンプト:**

| 設定キー | 説明 | デフォルト |
|------------|-------------|---------|
| `apply_prompt` | apply_commandの`{prompt}`に注入されるプロンプト | `スコープ外タスクは削除せよ。ユーザを待つもしくはユーザによるタスクは削除せよ。` |
| `archive_prompt` | archive_commandの`{prompt}`に注入されるプロンプト | （空） |

**クイックスタート:**

```bash
# initコマンドで設定を生成
openspec-orchestrator init

# または例の設定をコピー
cp .openspec-orchestrator.jsonc.example .openspec-orchestrator.jsonc

# 設定をカスタマイズ
vim .openspec-orchestrator.jsonc

# 設定を使用して実行
openspec-orchestrator
```

### フック設定

オーケストレーションプロセスの各段階でコマンドを実行するフックを設定できます。
フックは設定ファイルの `hooks` セクションで定義します。

```jsonc
{
  "hooks": {
    // シンプルな文字列形式（デフォルト設定を使用）
    "on_start": "echo 'Orchestrator started'",

    // オブジェクト形式（詳細設定付き）
    "post_apply": {
      "command": "cargo test",
      "continue_on_failure": false,  // コマンド失敗時にオーケストレーションを停止
      "timeout": 300                 // タイムアウト（秒）
    },

    // 実行ライフサイクルフック
    "on_start": "echo 'Starting orchestration with {total_changes} changes'",
    "on_finish": "echo 'Finished with status: {status}'",
    "on_error": "echo 'Error in {change_id}: {error}' >> errors.log",

    // 変更ライフサイクルフック
    "on_change_start": "echo 'Starting {change_id}'",
    "pre_apply": "echo 'Applying {change_id} (attempt {apply_count})'",
    "post_apply": "cargo test",
    "on_change_complete": "echo '{change_id} is 100% complete'",
    "pre_archive": "cargo clippy -- -D warnings",
    "post_archive": "echo '{change_id} archived successfully'",
    "on_change_end": "echo 'Finished processing {change_id}'",

    // TUI専用フック（ユーザー操作）
    "on_queue_add": "echo 'Added {change_id} to queue'",
    "on_queue_remove": "echo 'Removed {change_id} from queue'",
    "on_approve": "echo 'Approved {change_id}'",
    "on_unapprove": "echo 'Unapproved {change_id}'"
  }
}
```

**利用可能なフック:**

*実行ライフサイクルフック:*

| フック名 | トリガー | 説明 |
|-----------|---------|-------------|
| `on_start` | 開始 | オーケストレーター開始時 |
| `on_finish` | 終了 | オーケストレーター完了（成功またはリミット） |
| `on_error` | エラー | applyまたはarchive中にエラー発生時 |

*変更ライフサイクルフック:*

| フック名 | トリガー | 説明 |
|-----------|---------|-------------|
| `on_change_start` | 変更開始 | 新しい変更の処理開始時 |
| `pre_apply` | Apply前 | 変更適用前 |
| `post_apply` | Apply後 | 変更適用成功後 |
| `on_change_complete` | タスク100% | 変更が100%タスク完了に達した時 |
| `pre_archive` | Archive前 | 変更アーカイブ前 |
| `post_archive` | Archive後 | 変更アーカイブ成功後 |
| `on_change_end` | 変更終了 | 変更が正常にアーカイブされた後 |

*TUI専用フック（ユーザー操作）:*

| フック名 | トリガー | 説明 |
|-----------|---------|-------------|
| `on_queue_add` | キュー追加 | ユーザーが変更をキューに追加した時（Spaceキー） |
| `on_queue_remove` | キュー削除 | ユーザーが変更をキューから削除した時（Spaceキー） |
| `on_approve` | 承認 | ユーザーが変更を承認した時（@キー） |
| `on_unapprove` | 承認解除 | ユーザーが変更の承認を解除した時（@キー） |

**プレースホルダー:**

| プレースホルダー | 説明 |
|-------------|-------------|
| `{change_id}` | 現在の変更ID |
| `{changes_processed}` | これまでに処理された変更数 |
| `{total_changes}` | 初期スナップショットの変更総数 |
| `{remaining_changes}` | キュー内の残り変更数 |
| `{apply_count}` | 現在の変更のapply試行回数 |
| `{completed_tasks}` | 現在の変更の完了タスク数 |
| `{total_tasks}` | 現在の変更の総タスク数 |
| `{status}` | 終了ステータス（completed/iteration_limit） |
| `{error}` | エラーメッセージ |

**環境変数:**

フックは環境変数経由でコンテキストを受け取ります:
`OPENSPEC_CHANGE_ID`, `OPENSPEC_CHANGES_PROCESSED`, `OPENSPEC_TOTAL_CHANGES`, `OPENSPEC_REMAINING_CHANGES`, `OPENSPEC_APPLY_COUNT`, `OPENSPEC_COMPLETED_TASKS`, `OPENSPEC_TOTAL_TASKS`, `OPENSPEC_STATUS`, `OPENSPEC_ERROR`, `OPENSPEC_DRY_RUN`

### 環境変数

| 変数 | 説明 | デフォルト |
|----------|-------------|---------|
| `OPENSPEC_CMD` | OpenSpecコマンド（引数を含むことが可能） | `npx @fission-ai/openspec@latest` |
| `RUST_LOG` | ログレベル | (なし) |

例:

```bash
# カスタムopenspecインストールを使用
export OPENSPEC_CMD="/usr/local/bin/openspec"
openspec-orchestrator

# npxで特定バージョンを使用
export OPENSPEC_CMD="npx @fission-ai/openspec@1.2.3"
openspec-orchestrator
```

### コマンドラインオプション

```
使用法: openspec-orchestrator [オプション] [コマンド]

コマンド:
  run      OpenSpec変更オーケストレーションループを実行（非インタラクティブ）
  tui      インタラクティブTUIダッシュボードを起動
  init     新しい設定ファイルを初期化
  approve  変更の承認ステータスを管理

オプション:
  --opencode-path <PATH>   opencodeバイナリのパス（非推奨、設定ファイルを使用）
  --openspec-cmd <CMD>     OpenSpecコマンド [env: OPENSPEC_CMD]
  -h, --help               ヘルプを表示
```

**runサブコマンドのオプション:**
```
オプション:
  --change <ID,...>     指定した変更のみを処理（カンマ区切り）
  -c, --config <PATH>   カスタム設定ファイルパス（JSONC）
  --openspec-cmd <CMD>  カスタムopenspecコマンド [env: OPENSPEC_CMD]
  --parallel            並列実行モードを有効化
  --max-concurrent <N>  最大同時ワークスペース数（デフォルト: 3）
  --vcs <BACKEND>       VCSバックエンド: auto または git（デフォルト: auto）
  --dry-run             実行せずに並列化グループをプレビュー
```

### 並列実行

オーケストレーターはGit worktreesを使用した独立した変更の並列実行をサポートします。

**VCSバックエンドの選択:**

| バックエンド | 説明 | 要件 |
|---------|-------------|--------------|
| `auto` | Gitリポジトリを自動検出 | クリーンな作業ディレクトリを持つGitリポジトリ |
| `git` | Git worktreesを使用 | クリーンな作業ディレクトリを持つGitリポジトリ |

**使用法:**

```bash
# VCSバックエンドを自動検出（デフォルト）
openspec-orchestrator run --parallel

# Git worktreesを強制
openspec-orchestrator run --parallel --vcs git

# 実行せずに並列化グループをプレビュー
openspec-orchestrator run --parallel --dry-run

# 同時ワークスペース数を制限
openspec-orchestrator run --parallel --max-concurrent 5
```

**設定:**

設定ファイルでVCSバックエンドを設定することもできます:

```jsonc
{
  // 並列実行用のVCSバックエンド: "auto" または "git"
  "vcs_backend": "auto",

  // 最大同時ワークスペース数
  "max_concurrent_workspaces": 3
}
```

**Git要件:**

Git worktrees使用時:
- 作業ディレクトリがクリーンである必要があります（未コミットの変更がないこと）
- 各変更は独自のブランチを持つ独立したworktreeで実行されます
- 変更は完了後に順次マージされます

**initサブコマンドのオプション:**
```
オプション:
  -t, --template <TEMPLATE>  使用するテンプレート [デフォルト: claude] [可能な値: claude, opencode, codex]
  -f, --force                既存の設定ファイルを上書き
```

**approveサブコマンド:**
```
コマンド:
  set     変更を承認（チェックサム付きapprovedファイルを作成）
  unset   変更の承認を解除（approvedファイルを削除）
  status  変更の承認ステータスを確認
```

優先順位: CLIの引数 > 環境変数 > デフォルト値

## エラーハンドリング

| エラー | 動作 |
|-------|----------|
| エージェントコマンド失敗 | 3回リトライ後、失敗としてマーク |
| Applyコマンド失敗 | 変更を失敗としてマーク、他は継続 |
| Archiveコマンド失敗 | 変更を失敗としてマーク、他は継続 |
| LLM分析失敗 | 進捗ベースの選択にフォールバック |
| 全変更が失敗 | エラーで終了 |

## トラブルシューティング

### 「変更が見つかりません」

- `openspec list` を実行して変更が存在することを確認
- 正しいディレクトリにいることを確認

### 「エージェントコマンドが失敗しました」

- AIエージェントがインストールされていることを確認（例: `which claude`）
- 手動テスト: `claude -p "echo test"`
- 設定ファイルを確認: `.openspec-orchestrator.jsonc`

### 「すべての変更が失敗しました」

- ログで具体的なエラーを確認
- 単一の変更を処理してみる: `--change <id>`

## 開発

ビルド手順、テスト、プロジェクト構造については [DEVELOPMENT.md](DEVELOPMENT.md) を参照してください。

## 今後の機能強化

- [ ] リカバリと再開のための状態永続化
- [x] 独立した変更の並列実行（Git worktrees使用）
- [ ] Slack/Discord通知
- [ ] 最大イテレーション制限（無限ループ防止）
- [ ] 手動優先度オーバーライド
- [ ] 実行計画付きドライラン強化
- [ ] モニタリング用Web UI

## ライセンス

MIT

## コントリビューション

コントリビューション歓迎！Issue または Pull Request をお気軽にどうぞ。

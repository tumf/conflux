# Conflux

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

OpenSpec変更ワークフロー（list → 依存関係分析 → apply → archive）を自動化。`openspec` と AI コーディングエージェントを連携させて変更を自律的に処理します。

## 特徴

- 🖥️ **インタラクティブTUI**: リアルタイム進捗ダッシュボード（デフォルトモード）
- 🤖 **自動ワークフロー**: OpenSpec変更の検出からアーカイブまで自動処理
- 🧠 **LLM依存関係分析**: AIエージェントによる変更順序のインテリジェント分析
- 📊 **リアルタイム進捗**: 全体および変更ごとのビジュアル進捗バー
- 🔌 **マルチエージェント対応**: Claude Code、OpenCode、Codexに対応
- 🪝 **ライフサイクルフック**: ワークフロー各段階でのカスタムアクション設定
- ✅ **承認ワークフロー**: チェックサム検証による変更の承認管理
- ⚡ **並列実行**: Git worktreesを使用した複数の独立した変更の同時処理
- 🌐 **Web監視**: リモートモニタリング用REST APIとWebSocketを備えたオプションのHTTPサーバー

## アーキテクチャ

```
┌─────────────────────────────────────────────┐
│     cflx (Rust CLI)        │
├─────────────────────────────────────────────┤
│  CLI → Orchestrator → State Manager         │
│    ↓        ↓              ↓                │
│  OpenSpec  AIエージェント   進捗表示         │
│            (Claude/OpenCode/Codex)          │
└─────────────────────────────────────────────┘
```

## 使い方

### Golden Path: クイックスタート

```bash
# ステップ1: AIエージェント用の設定ファイルを生成（デフォルトはClaude Code）
cflx init

# ステップ2: 生成された .cflx.jsonc を編集してエージェントを設定
vim .cflx.jsonc

# ステップ3a: インタラクティブTUIを起動して変更を確認・処理
cflx

# ステップ3b: またはヘッドレス（非インタラクティブ）モードで実行
cflx run
```

### インタラクティブTUI（主要インターフェース）

オーケストレーターの主な使用方法は、インタラクティブTUIダッシュボードです:

```bash
cflx
```

TUIの機能:
- リアルタイム変更状況の可視化
- 保留中の全変更の進捗追跡
- キーボードナビゲーションとコントロール
- Worktree管理ビュー

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
| `[not queued]` | 実行キュー外（Running中に動的に外す/追加する対象） |
| `[queued]` | 実行待ち |
| `[blocked]` | 依存関係待ち（解消するまで開始しない） |
| `[merge wait]` | マージ待ち（Mでresolveをトリガー） |
| `[resolve pending]` | resolve実行開始待ち（操作ロックされる） |
| `[applying]` | 適用中（スピナー表示 + 進捗% / iteration を併記） |
| `[accepting]` | 受け入れ/テスト中（スピナー表示、iterationがあれば併記） |
| `[archiving]` | アーカイブ中（スピナー表示、iterationがあれば併記） |
| `[resolving]` | resolve中（スピナー表示、iterationがあれば併記） |
| `[archived]` | アーカイブ完了 |
| `[merged]` | mainにマージ済み（並列モードのみ） |
| `[error]` | 処理失敗 |

**ワークフロー:**
1. **Selectモード（ヘッダーは`[Ready]`）**: `@`で承認、`Space`で実行マーク（selected）の切替（この時点では即時キュー投入しない）
2. `F5`で処理開始 - 実行マークされた変更の`queue_status`が`queued`になる
3. **Runningモード（ヘッダーは`[Running N]`）**: `queued` → `applying` → （必要に応じて`accepting`）→ `archiving` → `archived`（並列モードでは`merge wait`/`resolving`/`merged`も発生）

#### ヘッダー表示

| 表示 | 意味 |
|------|------|
| `[Ready]` | 選択操作中（`AppMode::Select`） |
| `[Running N]` | 実行中（`applying`/`accepting`/`archiving`/`resolving` の件数が N） |

#### TUIキーバインド

**Changesビュー:**

| キー | Select（`[Ready]`） | Running（/Stopping） | Stopped（/Error） |
|------|-------------------|--------------------|------------------|
| `↑/↓` または `j/k` | リスト移動 | リスト移動 | リスト移動 |
| `Tab` | Worktreesビューに切替 | Worktreesビューに切替 | Worktreesビューに切替 |
| `Space` | 実行マーク（selected）切替のみ | 動的キュー追加/削除（`not queued`⇄`queued`） | `not queued` の項目のみ実行マーク切替 |
| `@` | 承認切替 | 承認切替 | 承認切替 |
| `e` | エディタを開く | エディタを開く | エディタを開く |
| `w` | QRコード表示* | QRコード表示* | QRコード表示* |
| `M` | `merge wait` の場合のみresolve | `merge wait` の場合のみresolve | `merge wait` の場合のみresolve |
| `F5` | 処理開始 | （Stopping中は停止キャンセル） | 再開（Stopped）/リトライ（Error） |
| `=` | パラレルモード切替 | - | パラレルモード切替 |
| `Esc` | - | 停止（1回=穏やか、2回=強制） | - |
| `PageUp/Down` | （ログ表示時）ログスクロール | ログスクロール | ログスクロール |
| `Home/End` | （ログ表示時）先頭/末尾へ | 先頭/末尾へ | 先頭/末尾へ |
| `Ctrl+C` | 終了 | 終了 | 終了 |

**Worktreesビュー:**

| キー | アクション | 説明 |
|------|------------|------|
| `Tab` | Changesビューに切替 | メイン変更リストに戻る |
| `↑/↓` または `j/k` | worktreeナビゲート | worktreeエントリー間を移動 |
| `+` | 新しいworktreeを作成 | ユニークなブランチ名で新規worktreeを作成 |
| `D` | worktreeを削除 | メイン以外・処理中でないworktreeを削除 |
| `M` | ベースブランチにマージ | 現在のworktreeブランチをマージ（コンフリクトがない場合のみ） |
| `e` | エディタを開く | worktreeディレクトリでエディタを開く |
| `Enter` | シェルを開く | `worktree_command`が設定されている場合のみ実行 |
| `Ctrl+C` | 終了 | アプリケーション終了 |

*QRコードはWeb監視が有効な場合のみ利用可能です（`--web`フラグ）。任意のキーでQRポップアップを閉じます。

### TUI Worktreeビュー

TUIには、インターフェースから直接git worktreeを管理するための専用Worktreeビューが含まれています。

**主な機能:**

- **ビュー切替**: `Tab`キーでChangesビューとWorktreesビューを切り替え
- **Worktreeリスト**: パス（ベース名）、ブランチ名、ステータスを含むすべてのworktreeを表示
- **コンフリクト検出**: バックグラウンドで並列にマージコンフリクトを自動チェック
- **ブランチマージ**: `M`キーでworktreeブランチをベースにマージ（コンフリクトがない場合のみ）
- **Worktree管理**: 作成（`+`）、削除（`D`）、エディタを開く（`e`）、シェルを開く（`Enter`）

**ワークフロー:**

1. **Worktreesビューに切替**: ChangesビューからTab`キーを押す
   - コンフリクト検出付きでworktreeリストを読み込み（並列実行）
   - 表示形式: `<worktree-path> → <branch-name> [STATUS] [⚠conflicts]`

2. **Worktreeをナビゲート**: `↑`/`↓`または`j`/`k`キーを使用
   - メインworktreeは`[MAIN]`インジケータ付きで表示（緑）
   - Detached HEADは`[DETACHED]`インジケータで表示
   - コンフリクトは`⚠<count>`バッジで表示（赤）

3. **ブランチマージ**: `M`キーを押す（安全な場合のみ有効）
   - 検証: メインworktreeでない、detached HEADでない、コンフリクトがない
   - 実行: ベースリポジトリで`git merge --no-ff --no-edit <branch>`を実行
   - 成功時: 成功ログを表示、worktreeリストを更新
   - 失敗時: 詳細付きエラーポップアップを表示

4. **Worktree作成**: `+`キーを押す
   - ユニークなブランチ名を生成: `ws-session-<timestamp>`
   - 新しいブランチでworktreeを作成（detached HEADではない）
   - `worktree_command`設定オプションが必要

5. **Worktree削除**: `D`キーを押す（メイン以外・処理中でないworktreeのみ）
   - 確認ダイアログを表示（`Y`で確認、`N`/`Esc`でキャンセル）
   - worktreeディレクトリを削除してリストを更新

6. **エディタ/シェルを開く**: `e`または`Enter`キーを押す
   - `e`: worktreeディレクトリでエディタを開く（`$EDITOR`を尊重）
   - `Enter`: worktreeで`worktree_command`を実行（例: シェルを開く）

**コンフリクト検出:**

- Worktreesビューに切り替えたときに自動実行
- `git merge --no-commit --no-ff`を使用して、各メイン以外・detached HEAD以外のworktreeを並列チェック
- 作業ツリーを変更せずにコンフリクトを検出（`git merge --abort`を使用）
- コンフリクト数を`⚠<count>`バッジで赤く表示
- バックグラウンドで5秒ごとに更新（自動更新）
- コンフリクト検出時は`M`キーを無効化

**パフォーマンス:**

- 並列コンフリクトチェック: 非同期並行実行を使用
- 典型的なパフォーマンス: 4つのworktreeを1秒未満でチェック
- ノンブロッキング: コンフリクトチェックが非同期実行され、TUIは応答性を維持
- フォールバック: チェック失敗時は、コンフリクト情報なしと仮定（安全なデフォルト）

### 設定の初期化

お好みのAIエージェント用の設定ファイルを生成:

```bash
# デフォルト: Claude Codeテンプレート
cflx init

# OpenCodeテンプレート
cflx init --template opencode

# Codexテンプレート
cflx init --template codex

# 既存の設定を上書き
cflx init --force
```

利用可能なテンプレート: `claude`（デフォルト）、`opencode`、`codex`

### オーケストレーション実行（非インタラクティブ）

ヘッドレスモードで保留中の全変更を処理:

```bash
cflx run
```

特定の変更を処理（単一または複数）:

```bash
# 単一の変更
cflx run --change add-feature-x

# 複数の変更（カンマ区切り）
cflx run --change add-feature-x,fix-bug-y,refactor-z
```

カスタム設定ファイル:

```bash
cflx run --config /path/to/config.jsonc
```

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
1. `.cflx.jsonc`（プロジェクトルート）
2. `~/.config/cflx/config.jsonc`（グローバル）
3. `--config` オプションによるカスタムパス

**設定例（Claude Code）:**

```jsonc
{
  // 依存関係を分析し次の変更を選択するコマンド
  "analyze_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '{prompt}'",

  // 変更を適用するコマンド（{change_id}と{prompt}プレースホルダーをサポート）
  "apply_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '/openspec:apply {change_id} {prompt}'",

  // apply後に受け入れテストを実行するコマンド（{change_id}と{prompt}プレースホルダーをサポート）
  "acceptance_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '/openspec:accept {change_id} {prompt}'",

  // 完了した変更をアーカイブするコマンド（{change_id}と{prompt}プレースホルダーをサポート）
  "archive_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '/openspec:archive {change_id} {prompt}'",

  // マージコンフリクトを解決するコマンド（{prompt}プレースホルダーをサポート）
  "resolve_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '{prompt}'",

  // applyコマンドのシステムプロンプト（{prompt}プレースホルダーに注入）
  "apply_prompt": "スコープ外タスクは削除せよ。ユーザを待つもしくはユーザによるタスクは削除せよ。",

  // acceptanceコマンドのシステムプロンプト（{prompt}プレースホルダーに注入）
  "acceptance_prompt": "",

  // acceptanceの{prompt}の構築方法を制御
  // - "full": ハードコードされたacceptanceシステムプロンプト + diff/履歴コンテキストを含む（デフォルト）
  // - "context_only": 変更メタデータ + diff/履歴コンテキストのみを含む
  "acceptance_prompt_mode": "full",

  // CONTINUE応答の最大リトライ回数（デフォルト: 10）
  "acceptance_max_continues": 10,

  // archiveコマンドのシステムプロンプト（{prompt}プレースホルダーに注入）
  "archive_prompt": "",

  // TUIから提案worktreeを作成するコマンド（+キー）
  // {workspace_dir}と{repo_root}プレースホルダーをサポート
  "worktree_command": "claude --dangerously-skip-permissions --verbose -p '/openspec:proposal --worktree {workspace_dir}'",

  // ライフサイクルフック（オプション）
  "hooks": {
    // "pre_apply": "echo 'Starting {change_id}'",
    // "post_apply": "echo 'Completed {change_id}'"
  }
}
```

**ロギング設定:**

```jsonc
{
  "logging": {
    "suppress_repetitive_debug": true,
    "summary_interval_secs": 60
  }
}
```

- `suppress_repetitive_debug`: 状態が変わらない場合に繰り返しのデバッグログを抑制する（デフォルト: true）
- `summary_interval_secs`: N秒ごとにサマリーログを出力する（0で無効化、デフォルト: 60）

**プレースホルダー:**

| プレースホルダー | 説明 | 使用箇所 |
|-------------|-------------|---------|
| `{change_id}` | 処理中の変更ID | apply_command, acceptance_command, archive_command |
| `{prompt}` | エージェントコマンドのシステムプロンプト | apply_command, acceptance_command, archive_command, resolve_command, analyze_command |
| `{workspace_dir}` | 提案用の新しいworktreeパス | worktree_command |
| `{repo_root}` | リポジトリのルートパス | worktree_command |

**システムプロンプト:**

| 設定キー | 説明 | デフォルト |
|------------|-------------|---------|
| `apply_prompt` | apply_commandの`{prompt}`に注入されるプロンプト | （パスコンテキストを含む） |
| `acceptance_prompt` | acceptance_commandの`{prompt}`に注入されるプロンプト | （空） |
| `archive_prompt` | archive_commandの`{prompt}`に注入されるプロンプト | （空） |

**クイックスタート:**

```bash
# initコマンドで設定を生成
cflx init

# または例の設定をコピー
cp .cflx.jsonc.example .cflx.jsonc

# 設定をカスタマイズ
vim .cflx.jsonc

# 設定を使用して実行
cflx
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
cflx

# npxで特定バージョンを使用
export OPENSPEC_CMD="npx @fission-ai/openspec@1.2.3"
cflx
```

### コマンドラインオプション

```
使用法: cflx [オプション] [コマンド]

コマンド:
  run              OpenSpec変更オーケストレーションループを実行（非インタラクティブ）
  tui              インタラクティブTUIダッシュボードを起動
  init             新しい設定ファイルを初期化
  check-conflicts  変更間のスペックデルタファイルのコンフリクトを確認
  server           マルチプロジェクトサーバーデーモンを起動

オプション:
  -c, --config <PATH>              カスタム設定ファイルパス（JSONC形式）
  --web                            Web監視サーバーを有効化
  --web-port <PORT>                Webサーバーポート（デフォルト: 0 = OSが自動割当）
  --web-bind <ADDR>                Webサーバーバインドアドレス（デフォルト: 127.0.0.1）
  --server <URL>                   リモートConfluxサーバーへの接続URL（例: http://host:9876）
  --server-token <TOKEN>           リモートサーバー認証用ベアラートークン
  --server-token-env <VAR>         ベアラートークンを保持する環境変数名
  -h, --help                       ヘルプを表示
  -V, --version                    バージョンを表示
```

**runサブコマンドのオプション:**
```
オプション:
  --change <ID,...>         指定した変更のみを処理（カンマ区切り）
  -c, --config <PATH>       カスタム設定ファイルパス（JSONC）
  --parallel                並列実行モードを有効化
  --max-concurrent <N>      最大同時ワークスペース数（デフォルト: 3）
  --vcs <BACKEND>           VCSバックエンド: auto または git（デフォルト: auto）
  --no-resume               ワークスペースレジュームを無効化（常に新しいワークスペースを作成）
  --dry-run                 実行せずに並列化グループをプレビュー
  --max-iterations <N>      オーケストレーションループの最大イテレーション数（0 = 制限なし）
  --web                     Web監視サーバーを有効化
  --web-port <PORT>         Webサーバーポート（デフォルト: 0 = OSが自動割当）
  --web-bind <ADDR>         Webサーバーバインドアドレス（デフォルト: 127.0.0.1）
```

**TUIオプション:**

TUI（デフォルトモード、`cflx` または `cflx tui`）はWeb監視オプションもサポートします:

```bash
# TUIでWeb監視を有効化
cflx --web

# カスタムポートとバインドアドレス
cflx --web --web-port 9000 --web-bind 0.0.0.0
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
cflx run --parallel

# Git worktreesを強制
cflx run --parallel --vcs git

# 実行せずに並列化グループをプレビュー
cflx run --parallel --dry-run

# 同時ワークスペース数を制限
cflx run --parallel --max-concurrent 5
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

**ワークスペースレジューム:**

デフォルトでは、オーケストレーターは中断された実行から既存のワークスペースを自動的に検出して再利用します。これにより、進捗を失うことなく中断した場所から作業を再開できます。

- 変更IDのワークスペースが見つかった場合、新しく作成せずに再利用されます
- 同じ変更に対して複数のワークスペースが存在する場合、最新のものが使用され、古いものはクリーンアップされます
- この動作を無効にして常に新しいワークスペースを作成するには`--no-resume`を使用します

```bash
# 既存のワークスペースから再開（デフォルトの動作）
cflx run --parallel

# 常に新しいワークスペースを作成（既存の作業を破棄）
cflx run --parallel --no-resume
```

**ワークスペース状態検出（冪等レジューム）:**

オーケストレーターは各ワークスペースの現在の状態を検出して、冪等な実行を保証します。再開時、ワークスペースは以下の5つの状態のいずれかに分類されます:

| 状態 | 説明 | 実行されるアクション |
|------|------|---------------------|
| **Created** | 新規ワークスペース、コミットなし | 最初からapplyを開始 |
| **Applying** | WIPコミット存在、apply進行中 | 次のイテレーションからapplyを再開 |
| **Applied** | Apply完了（`Apply: <change_id>`コミット存在） | applyをスキップ、archiveのみ実行 |
| **Archived** | Archive完了（`Archive: <change_id>`コミット存在） | apply/archiveをスキップ、mergeのみ実行 |
| **Merged** | すでにメインブランチにマージ済み | すべての操作をスキップ、ワークスペースをクリーンアップ |

この状態検出により以下が保証されます:
- 同じワークスペースでオーケストレーターを複数回実行しても安全で、同じ結果が得られる（冪等性）
- 手動でアーカイブまたはマージされた変更が検出され、正しく処理される
- 中断された操作が正しいステップから再開される
- 重複作業が実行されない

**状態検出の例:**

```bash
# apply中に中断 - 中断した場所から再開
$ cflx run --parallel
# ワークスペース状態: Applying (iteration 3/5)
# アクション: iteration 4からapplyを再開

# 手動で変更をアーカイブ - apply/archiveをスキップ
$ cflx run --parallel
# ワークスペース状態: Archived
# アクション: apply/archiveをスキップ、mainへのmergeのみ

# すでにmainにマージ済み - クリーンアップのみ
$ cflx run --parallel
# ワークスペース状態: Merged
# アクション: すべての操作をスキップ、ワークスペースをクリーンアップ
```

### コマンド実行キュー

オーケストレーターには、複数のAIエージェントコマンドを並列実行する際のリソース競合を防ぎ、一時的なエラーを処理するコマンド実行キューが含まれています。

**機能:**

1. **段階的開始**: 同時リソースアクセスを防ぐために、設定可能な遅延でコマンドを開始
2. **自動リトライ**: 一時的なエラー（モジュール解決、ネットワーク問題など）で失敗したコマンドを自動的にリトライ

**設定:**

```jsonc
{
  // コマンド実行間の遅延（ミリ秒）
  // デフォルト: 2000（2秒）
  "command_queue_stagger_delay_ms": 2000,

  // 失敗したコマンドの最大リトライ回数
  // デフォルト: 2
  "command_queue_max_retries": 2,

  // リトライ間の遅延（ミリ秒）
  // デフォルト: 5000（5秒）
  "command_queue_retry_delay_ms": 5000,

  // この閾値未満の実行時間でリトライ（秒）
  // 短時間の失敗は環境/起動問題を示すことが多い
  // デフォルト: 5
  "command_queue_retry_if_duration_under_secs": 5,

  // 自動リトライをトリガーするエラーパターン（正規表現）
  // デフォルト: モジュール解決、レジストリ、ロックエラー
  "command_queue_retry_patterns": [
    "Cannot find module",
    "ResolveMessage:",
    "ENOTFOUND registry\\.npmjs\\.org",
    "ETIMEDOUT.*registry",
    "EBADF.*lock",
    "Lock acquisition failed"
  ]
}
```

**仕組み:**

- **段階的開始**: 各コマンドは最後のコマンドが開始されてから最小遅延を待機し、共有リソース（例: `~/.cache/opencode/node_modules`）への同時アクセスを防ぐ
- **リトライロジック**: 以下の場合、コマンドがリトライされる:
  - 設定されたエラーパターンに一致する（例: "Cannot find module"）、または
  - 短時間で終了する（デフォルトで5秒未満）、起動/環境問題を示す
- **リトライなし**: 長時間実行（5秒以上）してエラーパターンに一致しないコマンドは、論理エラーの可能性が高いためリトライされない

**例 - モジュール解決競合の防止:**

```bash
# キューなし: 複数のコマンドが同時に開始
# → 競合: すべてが一度にnode_modulesを更新しようとする
# → 結果: "Cannot find module"エラー

# キュー使用（デフォルト）: コマンドが2秒間隔で開始
# → 最初のコマンドがnode_modulesを更新
# → 後続のコマンドは安定した環境を使用
# → 結果: 競合なし
```

**例 - 一時的なネットワークエラーの処理:**

```bash
# エラー: ETIMEDOUT registry.npmjs.org
# → リトライパターンに一致
# → 5秒後に自動リトライ
# → 通常はリトライで成功
```

### Web監視

オーケストレーターは、Webブラウザを介したオーケストレーション進捗のリモート監視のためのオプションのHTTPサーバーをサポートします。

**使用法:**

```bash
# TUIでWeb監視を有効化（OSが利用可能なポートを自動割当）
cflx --web

# カスタムポートとバインドアドレス
cflx --web --web-port 9000 --web-bind 0.0.0.0

# ヘッドレスrunモードと併用
cflx run --web
```

デフォルトポート（0）を使用すると、OSが利用可能なポートを自動的に割り当てます。
実際にバインドされたアドレスは、サーバー起動時にログに記録されます。

**機能:**

- **ダッシュボードUI**: `http://localhost:8080/`で進捗を表示
- **リアルタイム更新**: WebSocket接続によるライブ進捗更新
- **REST API**: プログラムから状態をクエリ
- **QRコードポップアップ**: TUIで`w`キーを押すと、モバイルでダッシュボードに素早くアクセスするためのQRコードを表示

**REST APIエンドポイント:**

| エンドポイント | メソッド | 説明 |
|---------------|----------|------|
| `/api/health` | GET | ヘルスチェック |
| `/api/state` | GET | 完全なオーケストレーター状態 |
| `/api/changes` | GET | 進捗を含むすべての変更をリスト |
| `/api/changes/{id}` | GET | 特定の変更の詳細 |
| `/api/changes/{id}/approve` | POST | 変更を承認 |
| `/api/changes/{id}/unapprove` | POST | 変更の承認を解除 |

完全なAPI仕様については、[OpenAPIドキュメント](docs/openapi.yaml)を参照してください。

**WebSocket:**

リアルタイム状態更新のために`ws://localhost:8080/ws`に接続します。メッセージは以下の形式のJSONです:

```json
{
  "type": "state_update",
  "timestamp": "2024-01-12T10:30:00Z",
  "changes": [
    {
      "id": "add-feature",
      "completed_tasks": 3,
      "total_tasks": 10,
      "progress_percent": 30.0,
      "status": "in_progress"
    }
  ]
}
```

**ダッシュボード概要:**

Webダッシュボードはオーケストレーション進捗の視覚的な概要を提供します:

```
┌─────────────────────────────────────────────────────────────────┐
│  OpenSpec Orchestrator                           ● Connected    │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐            │
│  │    5    │  │    2    │  │    1    │  │    2    │            │
│  │  Total  │  │Complete │  │Progress │  │ Pending │            │
│  └─────────┘  └─────────┘  └─────────┘  └─────────┘            │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ add-feature-auth                    [APPROVED] [IN_PROGRESS]│
│  │ ████████████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  40%    │   │
│  │ 4/10 tasks                                               │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                 │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ fix-login-bug                       [APPROVED] [COMPLETE]   │
│  │ ████████████████████████████████████████████████  100%  │   │
│  │ 5/5 tasks                                                │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                 │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ refactor-api                        [PENDING]               │
│  │ ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  0%    │   │
│  │ 0/8 tasks                                                │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                 │
├─────────────────────────────────────────────────────────────────┤
│  Last updated: 2024-01-12 10:30:00                              │
└─────────────────────────────────────────────────────────────────┘
```

**ダッシュボード機能:**

- **統計バー**: 合計、完了、進行中、保留中の変更数を表示
- **変更カード**: 各変更にID、承認ステータス、進捗ステータス、プログレスバーを表示
- **リアルタイム更新**: WebSocket接続経由で進捗を自動更新
- **接続ステータス**: 現在のWebSocket接続状態を表示（接続済み/切断）
- **レスポンシブデザイン**: デスクトップおよびモバイルブラウザで動作

**Web監視トラブルシューティング:**

| 問題 | 解決策 |
|------|--------|
| "Address already in use" | `--web-port 0`（デフォルト）を使用してOSに利用可能なポートを自動割当させるか、未使用の特定ポートを指定 |
| ダッシュボードが読み込まれない | `--web`フラグが有効になっていることを確認。URLに正しいポートが含まれていることを確認 |
| WebSocketが頻繁に切断する | ネットワークの安定性を確認。ダッシュボードは切断時に自動再接続 |
| 変更が更新されない | ページを更新するか、オーケストレーターがアクティブに処理中であることを確認 |
| 別のデバイスからアクセスできない | 外部接続を許可するには`--web-bind 0.0.0.0`を使用（ローカルネットワークのみ） |
| ブラウザコンソールでCORSエラー | これはクロスオリジンリクエストでは正常です; サーバーがCORSヘッダーを処理します |

**initサブコマンドのオプション:**
```
オプション:
  -t, --template <TEMPLATE>  使用するテンプレート [デフォルト: claude] [可能な値: claude, opencode, codex]
  -f, --force                既存の設定ファイルを上書き
```

**check-conflictsサブコマンドのオプション:**
```
オプション:
  -j, --json  結果をJSON形式で出力
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
- 設定ファイルを確認: `.cflx.jsonc`

### 「すべての変更が失敗しました」

- ログで具体的なエラーを確認
- 単一の変更を処理してみる: `--change <id>`

## インストール

```bash
cargo install --path .
```

これにより、オーケストレーターがビルドされ、Cargoのbinディレクトリ（通常は`~/.cargo/bin`）にインストールされます。

## ドキュメント

| ドキュメント | 説明 |
|-------------|------|
| [使用例](docs/guides/USAGE.md) | クイックスタートと使用例 |
| [開発ガイド](docs/guides/DEVELOPMENT.md) | ビルド手順とプロジェクト構造 |
| [リリースガイド](docs/guides/RELEASE.md) | リリース作成方法 |
| [API仕様](docs/openapi.yaml) | Web監視用OpenAPI仕様 |

内部ドキュメント（並列実行監査）は `docs/audit/` にあります。

## プロジェクト構成

```
src/
  main.rs                   # エントリーポイント、CLIディスパッチ
  cli.rs                    # CLI引数解析（clap）
  error.rs                  # エラー型（thiserror）
  openspec.rs               # OpenSpec CLIラッパー
  orchestrator.rs           # メインオーケストレーションループ
  progress.rs               # 進捗表示（indicatif）
  hooks.rs                  # ライフサイクルフック実行
  task_parser.rs            # ネイティブtasks.mdパーサー
  templates.rs              # 設定テンプレート
  agent.rs                  # AIエージェントコマンド実行
  analyzer.rs               # 変更依存関係アナライザー
  approval.rs               # 変更承認管理
  command_queue.rs          # スタッガーとリトライ付きコマンドキュー
  history.rs                # apply/archive/resolve履歴
  parallel_run_service.rs   # 並列実行サービス

  execution/                # 共有実行ロジック
    apply.rs                # Apply操作ロジック
    archive.rs              # Archive操作ロジック
    state.rs                # ワークスペース状態検出
    types.rs                # 共通型定義

  config/                   # 設定
    defaults.rs             # デフォルト値
    expand.rs               # 環境変数展開
    jsonc.rs                # JSONCパーサー

  vcs/                      # バージョン管理抽象化
    commands.rs             # 共通VCSインターフェース
    git/                    # Gitバックエンド

  parallel/                 # 並列実行
    executor.rs             # 並列変更エグゼキューター
    events.rs               # 進捗レポートイベント
    conflict.rs             # コンフリクト検出/解決
    cleanup.rs              # ワークスペースクリーンアップ

  tui/                      # ターミナルユーザーインターフェース
    render.rs               # ターミナルレンダリング
    runner.rs               # TUIメインループ
    state/                  # 状態管理

tests/
  e2e_tests.rs              # エンドツーエンドテスト
```

## 開発

ビルド手順、テスト、プロジェクト構造については [開発ガイド](docs/guides/DEVELOPMENT.md) を参照してください。

### Git Hooks

このプロジェクトでは、Git hooksの管理に[prek](https://prek.j178.dev/)を使用しています（pre-commitのRust版代替ツール）。

**pre-commitからの移行:**

以前にpre-commitを使用していた場合は、まずアンインストールしてください:

```bash
pre-commit uninstall
```

**セットアップ:**

```bash
# prekをインストール
brew install prek

# hooksをインストール
prek install
```

**使用方法:**

```bash
# すべてのファイルに対してすべてのhooksを実行
prek run --all-files

# 特定のhooksを実行
prek run rustfmt clippy

# 利用可能なhooksをリスト
prek list
```

設定は `.pre-commit-config.yaml` で定義されています（prekはpre-commit設定フォーマットと完全互換です）。`prek run --all-files` コマンドは `make openapi` を自動実行し、`docs/openapi.yaml` をステージングします。

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

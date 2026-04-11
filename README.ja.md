# Conflux

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

OpenSpec の変更ワークフロー（list → dependency analysis → apply → archive）を自動化します。`openspec` と AI コーディングエージェントを連携させ、変更を自律的に処理します。

## 特徴

- 🖥️ **インタラクティブ TUI**: リアルタイム進捗ダッシュボードを備えたデフォルトモード
- 🤖 **自動ワークフロー**: OpenSpec 変更を検出からアーカイブまで自動処理
- 🧠 **LLM 依存関係分析**: AI エージェントで変更順序を賢く分析
- 📊 **リアルタイム進捗**: 全体および変更ごとの状態を可視化する進捗バー
- 🔌 **マルチエージェント対応**: Claude Code、OpenCode、Codex に対応
- 🪝 **ライフサイクルフック**: 各ワークフロー段階で独自アクションを設定可能
- ⚡ **並列実行**: Git worktree を使って独立した複数変更を同時処理
- 🌐 **Web UI**: REST API と WebSocket を備えた任意のブラウザダッシュボードでリモート監視

## アーキテクチャ

```
┌─────────────────────────────────────────────┐
│     cflx (Rust CLI)        │
├─────────────────────────────────────────────┤
│  CLI → Orchestrator → State Manager         │
│    ↓        ↓              ↓                │
│  OpenSpec  AI エージェント   進捗表示        │
│            (Claude/OpenCode/Codex)          │
└─────────────────────────────────────────────┘
```

## モードとフロントエンド

Conflux には 2 つの動作モードと 3 つの主要フロントエンドがあります。これらを分けて考えると、README 全体を追いやすくなります。

### 動作モード

| モード | コマンド | 用途 |
|------|---------|---------|
| **通常モード** | `cflx` / `cflx run` | 現在のリポジトリでオーケストレーションを実行 |
| **サーバーモード** | `cflx server` | HTTP/WebSocket API を備えた長寿命のマルチプロジェクトデーモンを実行 |

### フロントエンド

| フロントエンド | コマンド / アクセス | 利用可能なモード | 用途 |
|----------|------------------|--------------|---------|
| **TUI** | `cflx` または `cflx tui` | 通常モード、リモートサーバークライアントモード | 変更の対話的な確認と制御 |
| **ヘッドレス実行** | `cflx run` | 通常モード | 非対話オーケストレーション |
| **Web UI** | ブラウザダッシュボード | 通常モードの `--web`、または `cflx server` | HTTP/WebSocket 経由のリモート監視 |

### どう連携するか

- **多くのユーザーは通常モードから開始**: `cflx` を実行し、TUI で変更を確認してローカル実行します。
- **`cflx run` を使う場面**: 同じオーケストレーションを対話 UI なしで実行したいとき。
- **`cflx server` を使う場面**: 複数プロジェクト向けの常駐デーモン、リモートアクセス、またはサーバー管理の提案セッションが必要なとき。
- **Web UI は独立した実行エンジンではありません**: HTTP/WebSocket 経由で公開されるオーケストレーター状態の上に載るダッシュボードフロントエンドです。

## クイックスタート

初回セットアップの完全な手順は [QUICKSTART.md](QUICKSTART.md) を参照してください。

- 英語: [QUICKSTART.md](QUICKSTART.md)
- 日本語: [QUICKSTART.ja.md](QUICKSTART.ja.md)

最短手順だけ欲しい場合は以下です:

```bash
cargo install cflx
cflx init
cflx
```

## 使い方

### ローカルオーケストレーション

#### TUI (`cflx`)

ローカルでの主要インターフェースは、対話型の TUI ダッシュボードです:

```bash
cflx
```

TUI では次を利用できます:
- 変更状態のリアルタイム可視化
- 保留中変更すべての進捗追跡
- キーボード操作とナビゲーション
- Worktree 管理ビュー

#### TUI の変更状態

変更には **選択状態 / キュー状態** があります。

**チェックボックス表示:**
| 記号 | 状態 | 説明 |
|--------|-------|-------------|
| `[ ]` | 未選択 | 処理対象としてマークされていない |
| `[x]` | 選択済み（予約） | F5 を押すとキューへ投入される |

**キュー状態（Running モードで表示）:**
| 状態 | 説明 |
|--------|-------------|
| `[not queued]` | 実行キュー外（実行中でも動的に切り替え可能） |
| `[queued]` | 処理待ち |
| `[blocked]` | 未解決の依存関係によりブロック中 |
| `[merge wait]` | マージ解決待ち（`M` で resolve を実行） |
| `[resolve pending]` | resolve 要求済みで実行待ち（UI 操作は制限される） |
| `[applying]` | 適用中（スピナー + 進捗率 / iteration を表示） |
| `[accepting]` | 受け入れ / テスト中（スピナー、iteration があれば表示） |
| `[archiving]` | アーカイブ中（スピナー、iteration があれば表示） |
| `[resolving]` | 解決中（スピナー、iteration があれば表示） |
| `[archived]` | 正常にアーカイブ完了 |
| `[merged]` | ベースブランチへマージ済み（並列モードのみ） |
| `[rejected]` | 終端状態として却下され、実行可能キューから外された |
| `[error]` | 処理失敗 |

**ワークフロー:**
1. **Select モード（ヘッダーは `[Ready]`）**: `Space` で実行マーク（`selected`）を切り替え
2. `F5` を押して処理開始 - 実行マークされた変更が `queued` になる
3. **Running モード（ヘッダーは `[Running N]`）**: `queued` → `applying` → （必要に応じて `accepting`）→ `archiving` → `archived`。ブロックや却下により `rejected` で早期終了する場合もあります（並列モードでは `merge wait` / `resolving` / `merged` も表示されます）

#### ヘッダー状態

| 表示 | 意味 |
|---------|---------|
| `[Ready]` | 選択 / 待機中（`AppMode::Select`） |
| `[Running N]` | 処理中。`applying` / `accepting` / `archiving` / `resolving` の件数が N |

#### TUI キーバインド

**Changes ビュー:**

| キー | Select（`[Ready]`） | Running（/Stopping） | Stopped（/Error） |
|-----|-------------------|--------------------|------------------|
| `↑/↓` または `j/k` | リスト移動 | リスト移動 | リスト移動 |
| `Tab` | Worktrees ビューへ切替 | Worktrees ビューへ切替 | Worktrees ビューへ切替 |
| `Space` | 実行マークの切替のみ | 動的キューへ追加 / 削除（`not queued`⇄`queued`） | `not queued` のみ実行マーク切替 |
| `e` | エディタを開く | エディタを開く | エディタを開く |
| `w` | QR コード表示* | QR コード表示* | QR コード表示* |
| `M` | 状態が `merge wait` のとき resolve | 状態が `merge wait` のとき resolve | 状態が `merge wait` のとき resolve |
| `F5` | 処理開始 | （Stopping 中は停止キャンセル） | 再開（Stopped）/ リトライ（Error） |
| `=` | 並列モード切替 | - | 並列モード切替 |
| `Esc` | - | 停止（1 回目: 穏やか、2 回目: 強制） | - |
| `PageUp/Down` | （ログ表示中）ログスクロール | ログスクロール | ログスクロール |
| `Home/End` | （ログ表示中）先頭 / 末尾 | 先頭 / 末尾 | 先頭 / 末尾 |
| `Ctrl+C` | 終了 | 終了 | 終了 |

**Worktrees ビュー:**

| キー | アクション | 説明 |
|-----|--------|-------------|
| `Tab` | Changes ビューへ切替 | メインの変更一覧へ戻る |
| `↑/↓` または `j/k` | worktree を移動 | worktree 項目間を移動 |
| `+` | 新しい worktree を作成 | 一意なブランチ名で新規 worktree を作成 |
| `D` | worktree を削除 | メイン以外かつ処理中でない worktree を削除 |
| `M` | ベースブランチへマージ | 現在の worktree ブランチをマージ（競合なしの場合のみ） |
| `e` | エディタを開く | worktree ディレクトリでエディタを開く |
| `Enter` | シェルを開く | `worktree_command` 設定時のみ実行 |
| `Ctrl+C` | 終了 | アプリケーションを終了 |

*QR コードは Web UI が有効な場合（`--web` フラグ）のみ利用できます。任意のキーで QR ポップアップを閉じます。

#### TUI Worktree ビュー

TUI には、インターフェースから直接 git worktree を管理する専用の Worktree ビューがあります。

**主な機能:**

- **ビュー切替**: `Tab` で Changes と Worktrees を切替
- **Worktree 一覧**: パス（basename）、ブランチ名、状態を表示
- **競合検出**: バックグラウンドでマージ競合を並列チェック
- **ブランチマージ**: `M` キーで worktree ブランチをベースへマージ（競合なしのみ）
- **Worktree 管理**: 作成（`+`）、削除（`D`）、エディタを開く（`e`）、シェルを開く（`Enter`）

**ワークフロー:**

1. **Worktrees ビューへ移動**: Changes ビューで `Tab` を押す
   - 競合検出付きで worktree 一覧を読み込み（並列実行）
   - 表示形式: `<worktree-path> → <branch-name> [STATUS] [⚠conflicts]`

2. **Worktree を移動**: `↑` / `↓` または `j` / `k` を使用
   - メイン worktree は `[MAIN]` 表示（緑）
   - Detached HEAD は `[DETACHED]` 表示
   - 競合は `⚠<count>` バッジ（赤）で表示

3. **ブランチをマージ**: `M` を押す（安全な場合のみ有効）
   - 検証: メイン worktree でない、detached HEAD でない、競合がない
   - 実行: ベースリポジトリで `git merge --no-ff --no-edit <branch>` を実行
   - 成功時: 成功ログを表示して一覧更新
   - 失敗時: 詳細付きのエラーポップアップを表示

4. **Worktree を作成**: `+` を押す
   - 一意なブランチ名を生成: `ws-session-<timestamp>`
   - 新規ブランチ付き worktree を作成（detached HEAD ではない）
   - `worktree_command` の設定が必要

5. **Worktree を削除**: `D` を押す（メイン以外かつ処理中でない worktree のみ）
   - 確認ダイアログを表示（`Y` で確定、`N` / `Esc` でキャンセル）
   - worktree ディレクトリを削除して一覧更新

6. **エディタ / シェルを開く**: `e` または `Enter`
   - `e`: worktree ディレクトリでエディタを開く（`$EDITOR` を尊重）
   - `Enter`: worktree 上で `worktree_command` を実行（例: シェル起動）

**競合検出:**

- Worktrees ビューへ切り替えた際に自動実行
- メイン以外かつ detached HEAD でない各 worktree に対し `git merge --no-commit --no-ff` を並列実行して確認
- 作業ツリーを変更せず競合検出（`git merge --abort` を使用）
- 競合数を `⚠<count>` バッジで赤表示
- バックグラウンドで 5 秒ごとに更新（自動リフレッシュ）
- 競合検出時は `M` キーを無効化

**パフォーマンス:**

- 並列競合チェック: 非同期の並列実行を使用
- 典型的な性能: 4 つの worktree を 1 秒未満で確認
- ノンブロッキング: 競合チェックは非同期で、TUI の応答性を維持
- フォールバック: チェック失敗時は競合情報なしとして扱う（安全側のデフォルト）

#### ヘッドレス実行 (`cflx run`)

非対話モードで保留中の変更をすべて処理します:

```bash
cflx run
```

特定の変更だけ処理することもできます（単一 / 複数）:

```bash
# 単一変更
cflx run --change add-feature-x

# 複数変更（カンマ区切り）
cflx run --change add-feature-x,fix-bug-y,refactor-z
```

カスタム設定ファイル:

```bash
cflx run --config /path/to/config.jsonc
```

### サーバーモード

サーバーモードは、常駐デーモン、複数プロジェクト管理、リモート API、またはサーバー管理の提案セッションが必要なときに使います。

```bash
cflx server
```

サーバーモードでは、接続クライアント向けに Web UI と API を公開します。TUI は `--server` でリモートサーバーへ接続できます。

バックグラウンドサービス管理やサーバー専用設定については [サーバーモード詳細](#サーバーモード詳細) を参照してください。

### Web UI とリモート監視

- **通常モード** では、`cflx` または `cflx run` に `--web` を付けてダッシュボードを有効化
- **サーバーモード** では、ダッシュボードはデーモン構成の一部
- ダッシュボード / API の詳細は [Web UI とダッシュボード](#web-ui-とダッシュボード) に記載

```bash
# ローカル TUI + Web UI
cflx --web

# ローカルのヘッドレス実行 + Web UI
cflx run --web

# サーバーへ接続するリモート TUI
cflx tui --server http://host:39876
```

### 設定ファイルを初期化

利用する AI エージェント向けの設定ファイルを生成します:

```bash
# デフォルト: Claude Code テンプレート
cflx init

# OpenCode テンプレート
cflx init --template opencode

# Codex テンプレート
cflx init --template codex

# 既存設定を上書き
cflx init --force
```

利用可能なテンプレート: `claude`（デフォルト）、`opencode`、`codex`

## 動作原理

### メインループ

```
1. openspec list で変更を一覧取得
   ↓
2. 次の変更を選択
   • 優先度 1: 100% 完了（アーカイブ準備完了）
   • 優先度 2: LLM 依存関係分析
   • 優先度 3: 最も進捗が高いもの（フォールバック）
   ↓
3. 変更を処理
   • 完了していれば: openspec archive
   • 未完了なら: AI エージェントが次のタスクを適用
   ↓
4. 状態を更新して繰り返す
```

### 依存関係分析

オーケストレーターは AI エージェントを使って依存関係を分析します:

```
// LLM に送るプロンプト
"以下の OpenSpec 変更から次に実行すべき変更を選んでください。

変更一覧:
- add-feature-x (2/5 tasks, 40.0%)
- fix-bug-y (5/5 tasks, 100.0%)
- refactor-z (0/3 tasks, 0.0%)

選択基準:
1. 依存関係がない、または依存先が完了している
2. 進捗が高い（継続性）
3. 名前から推測できる依存関係も考慮する

出力は変更 ID を 1 行だけにしてください。"
```

## 設定

### エージェント設定ファイル（JSONC）

オーケストレーターは JSONC 設定ファイルでエージェントコマンドを設定できます。
これにより、コードを変更せずに Claude Code、OpenCode、Codex など異なる AI ツールを利用できます。

**設定ファイルの場所**（優先順位順）:
1. `.cflx.jsonc`（プロジェクトルート）
2. `~/.config/cflx/config.jsonc`（グローバル）
3. `--config` オプションで指定したパス

**設定例（Claude Code）:**

```jsonc
{
  // 依存関係を分析し、次の変更を選ぶコマンド
  "analyze_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '{prompt}'",

  // 変更を適用するコマンド（{change_id} と {prompt} プレースホルダー対応）
  "apply_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '/openspec:apply {change_id} {prompt}'",

  // apply 後に受け入れテストを実行するコマンド（{change_id} と {prompt} プレースホルダー対応）
  "acceptance_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '/openspec:accept {change_id} {prompt}'",

  // 完了した変更をアーカイブするコマンド（{change_id} と {prompt} プレースホルダー対応）
  "archive_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '/openspec:archive {change_id} {prompt}'",

  // マージ競合を解決するコマンド（{prompt} プレースホルダー対応）
  "resolve_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '{prompt}'",

  // apply コマンド用のシステムプロンプト（{prompt} に注入）
  "apply_prompt": "スコープ外タスクは削除せよ。ユーザを待つもしくはユーザによるタスクは削除せよ。",

  // acceptance コマンド用のシステムプロンプト（{prompt} に注入）
  "acceptance_prompt": "",

  // acceptance の {prompt} の構築方法を制御
  // - "full": ハードコードされた受け入れシステムプロンプト + diff / history コンテキストを含む（デフォルト）
  // - "context_only": 変更メタデータ + diff / history コンテキストのみを含む
  // acceptance_command 側に固定指示を含むテンプレートを使う場合は "context_only" を推奨
  "acceptance_prompt_mode": "full",

  // acceptance の CONTINUE 応答を FAIL 扱いにするまでの最大回数（デフォルト: 10）
  "acceptance_max_continues": 10,

  // archive コマンド用のシステムプロンプト（{prompt} に注入）
  "archive_prompt": "",

  // TUI から提案用 worktree を作成するコマンド（+ キー）
  // {workspace_dir} と {repo_root} プレースホルダー対応
  "worktree_command": "claude --dangerously-skip-permissions --verbose -p '/openspec:proposal --worktree {workspace_dir}'",

  // ライフサイクルフック（任意）
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

- `suppress_repetitive_debug`: 状態が変わらない場合に重複デバッグログを抑制（デフォルト: true）
- `summary_interval_secs`: N 秒ごとにサマリーログを出力。0 で無効（デフォルト: 60）

**プレースホルダー:**

| プレースホルダー | 説明 | 使用箇所 |
|-------------|-------------|---------|
| `{change_id}` | 処理中の変更 ID | apply_command, acceptance_command, archive_command |
| `{prompt}` | エージェントコマンド向けシステムプロンプト | apply_command, acceptance_command, archive_command, resolve_command, analyze_command |
| `{workspace_dir}` | 提案用の新しい worktree パス | worktree_command |
| `{repo_root}` | リポジトリルートパス | worktree_command |

**システムプロンプト:**

| 設定キー | 説明 | デフォルト |
|------------|-------------|---------|
| `apply_prompt` | apply_command の `{prompt}` に注入されるプロンプト | （パスコンテキストを含む） |
| `acceptance_prompt` | acceptance_command の `{prompt}` に注入されるプロンプト | （空） |
| `archive_prompt` | archive_command の `{prompt}` に注入されるプロンプト | （空） |

### サーバー専用設定

#### 提案セッションの OPENCODE_CONFIG

この設定は `cflx server` が作成する提案セッションに適用されます。
サーバー側の提案セッションでは `OPENCODE_CONFIG` は自動生成 / 自動注入されません。
`proposal_session.transport_env.OPENCODE_CONFIG` が未設定の場合、opencode は内蔵のデフォルト設定を使います。

独自設定にしたい場合は、`OPENCODE_CONFIG` を明示的に指定してください:

```jsonc
{
  "proposal_session": {
    "transport_env": {
      "OPENCODE_CONFIG": "/absolute/path/to/opencode.json"
    }
  }
}
```

`OPENCODE_CONFIG` は任意で、カスタム opencode 設定ファイルを使いたいときだけ必要です。

**クイックスタート:**

```bash
# init コマンドで設定生成
cflx init

# またはサンプル設定をコピー
cp .cflx.jsonc.example .cflx.jsonc

# 必要に応じて編集
vim .cflx.jsonc

# 設定を使って実行
cflx
```

### フック設定

オーケストレーションの各段階でコマンドを実行するフックを設定できます。
フックは設定ファイルの `hooks` セクションで定義します。

```jsonc
{
  "hooks": {
    // 文字列形式（デフォルト設定を使用）
    "on_start": "echo 'Orchestrator started'",

    // オブジェクト形式（詳細設定あり）
    "post_apply": {
      "command": "cargo test",
      "continue_on_failure": false,  // コマンド失敗時はオーケストレーション停止
      "timeout": 300                 // タイムアウト秒数
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

    // TUI 専用フック（ユーザー操作）
    "on_queue_add": "echo 'Added {change_id} to queue'",
    "on_queue_remove": "echo 'Removed {change_id} from queue'"
  }
}
```

**利用可能なフック:**

*実行ライフサイクルフック:*

| フック名 | トリガー | 説明 |
|-----------|---------|-------------|
| `on_start` | 開始 | オーケストレーター起動時 |
| `on_finish` | 終了 | オーケストレーター完了時（成功または制限到達） |
| `on_error` | エラー | apply または archive 中にエラー発生時 |

*変更ライフサイクルフック:*

| フック名 | トリガー | 説明 |
|-----------|---------|-------------|
| `on_change_start` | 変更開始 | 新しい変更の処理開始時 |
| `pre_apply` | Apply 前 | 変更適用前 |
| `post_apply` | Apply 後 | 変更適用成功後 |
| `on_change_complete` | タスク 100% | 変更がタスク完了率 100% に達した時 |
| `pre_archive` | Archive 前 | 変更アーカイブ前 |
| `post_archive` | Archive 後 | 変更アーカイブ成功後 |
| `on_change_end` | 変更終了 | 変更が正常にアーカイブされた後 |

*TUI 専用フック（ユーザー操作）:*

| フック名 | トリガー | 説明 |
|-----------|---------|-------------|
| `on_queue_add` | キュー追加 | ユーザーが変更をキューに追加した時（Space キー） |
| `on_queue_remove` | キュー削除 | ユーザーが変更をキューから削除した時（Space キー） |

**プレースホルダー:**

| プレースホルダー | 説明 |
|-------------|-------------|
| `{change_id}` | 現在の変更 ID |
| `{changes_processed}` | これまでに処理した変更数 |
| `{total_changes}` | 初期スナップショット内の変更総数 |
| `{remaining_changes}` | キューに残っている変更数 |
| `{apply_count}` | 現在の変更に対する apply 試行回数 |
| `{completed_tasks}` | 現在の変更で完了済みのタスク数 |
| `{total_tasks}` | 現在の変更の総タスク数 |
| `{status}` | 終了ステータス（completed / iteration_limit） |
| `{error}` | エラーメッセージ |

**環境変数:**

フックには環境変数でコンテキストが渡されます:
`OPENSPEC_CHANGE_ID`, `OPENSPEC_CHANGES_PROCESSED`, `OPENSPEC_TOTAL_CHANGES`, `OPENSPEC_REMAINING_CHANGES`, `OPENSPEC_APPLY_COUNT`, `OPENSPEC_COMPLETED_TASKS`, `OPENSPEC_TOTAL_TASKS`, `OPENSPEC_STATUS`, `OPENSPEC_ERROR`, `OPENSPEC_DRY_RUN`

### 環境変数

| 変数 | 説明 | デフォルト |
|----------|-------------|---------|
| `OPENSPEC_CMD` | OpenSpec コマンド（引数を含められる） | `npx @fission-ai/openspec@latest` |
| `RUST_LOG` | ログレベル | （なし） |

例:

```bash
# 独自インストールの openspec を使う
export OPENSPEC_CMD="/usr/local/bin/openspec"
cflx

# npx で特定バージョンを使う
export OPENSPEC_CMD="npx @fission-ai/openspec@1.2.3"
cflx
```

### コマンドラインオプション

```
Usage: cflx [OPTIONS] [COMMAND]

Commands:
  run              OpenSpec 変更オーケストレーションループを実行（非対話）
  tui              インタラクティブ TUI ダッシュボードを起動
  init             新しい設定ファイルを初期化
  check-conflicts  変更間の spec delta ファイル競合を確認
  server           マルチプロジェクトサーバーデーモンを起動
  service          `cflx server` をバックグラウンドサービスとして管理

Options:
  -c, --config <PATH>          カスタム設定ファイルのパス（JSONC 形式）
  --web                        リモートダッシュボード用 Web UI サーバーを有効化
  --web-port <PORT>            Web UI サーバーのポート（デフォルト: 0 = OS が自動割当）
  --web-bind <ADDR>            Web UI サーバーの bind アドレス（デフォルト: 127.0.0.1）
  --server <URL>               リモート Conflux サーバーへ TUI を接続（例: http://host:39876）
  --server-token <TOKEN>       リモートサーバー認証用ベアラートークン
  --server-token-env <VAR>     ベアラートークンを保持する環境変数名
  -h, --help                   ヘルプを表示
  -V, --version                バージョンを表示
```

**run サブコマンドのオプション:**
```
Options:
  --change <ID,...>         指定した変更だけ処理（カンマ区切り）
  -c, --config <PATH>       カスタム設定ファイルパス（JSONC）
  --parallel                並列実行モードを有効化
  --max-concurrent <N>      最大同時ワークスペース数（デフォルト: 3）
  --vcs <BACKEND>           VCS バックエンド: auto または git（デフォルト: auto）
  --no-resume               ワークスペース再開を無効化（常に新規作成）
  --dry-run                 実行せずに並列化グループを確認
  --max-iterations <N>      オーケストレーションループ最大反復数（0 = 無制限）
  --web                     Web UI サーバーを有効化
  --web-port <PORT>         Web サーバーポート（デフォルト: 0 = OS が自動割当）
  --web-bind <ADDR>         Web サーバー bind アドレス（デフォルト: 127.0.0.1）
```

**TUI オプション:**

TUI（デフォルトモード、`cflx` または `cflx tui`）でも Web UI オプションが利用できます:

```bash
# TUI + Web UI
cflx --web

# カスタムポートと bind アドレス
cflx --web --web-port 9000 --web-bind 0.0.0.0
```

### 並列実行

オーケストレーターは Git worktree を使って独立した変更を並列実行できます。

**VCS バックエンド選択:**

| バックエンド | 説明 | 要件 |
|---------|-------------|--------------|
| `auto` | Git リポジトリを自動検出 | 作業ツリーがクリーンな Git リポジトリ |
| `git` | Git worktree を使用 | 作業ツリーがクリーンな Git リポジトリ |

**使い方:**

```bash
# VCS バックエンドを自動検出（デフォルト）
cflx run --parallel

# Git worktree を強制
cflx run --parallel --vcs git

# 実行せず並列化グループだけ確認
cflx run --parallel --dry-run

# 同時ワークスペース数を制限
cflx run --parallel --max-concurrent 5
```

**設定:**

設定ファイルでも VCS バックエンドを指定できます:

```jsonc
{
  // 並列実行用 VCS バックエンド: "auto" または "git"
  "vcs_backend": "auto",

  // 最大同時ワークスペース数
  "max_concurrent_workspaces": 3
}
```

**Git の要件:**

Git worktree 使用時:
- 作業ディレクトリはクリーンである必要があります（未コミット変更なし）
- 各変更は専用ブランチ付きの独立 worktree で実行されます
- 完了後は順次マージされます

**ワークスペース再開:**

デフォルトでは、オーケストレーターは中断された実行の既存ワークスペースを自動検出して再利用します。これにより、進捗を失わず中断地点から再開できます。

- 変更 ID に対応するワークスペースが見つかった場合、新規作成せず再利用されます
- 同じ変更に複数ワークスペースがある場合は最新を使用し、古いものはクリーンアップされます
- この挙動を無効化し、常に新しいワークスペースを作るには `--no-resume` を使います

```bash
# 既存ワークスペースから再開（デフォルト）
cflx run --parallel

# 常に新規ワークスペースを作成（既存作業は破棄）
cflx run --parallel --no-resume
```

**ワークスペース状態検出（冪等な再開）:**

オーケストレーターは各ワークスペースの現在状態を検出し、冪等に実行します。再開時、ワークスペースは以下 5 状態のいずれかに分類されます:

| 状態 | 説明 | 実行される動作 |
|-------|-------------|--------------|
| **Created** | 新しいワークスペース、コミットなし | 先頭から apply 開始 |
| **Applying** | WIP コミットあり、apply 進行中 | 次の iteration から apply 再開 |
| **Applied** | Apply 完了（`Apply: <change_id>` コミットあり） | apply を飛ばして archive のみ実行 |
| **Archived** | Archive 完了（`Archive: <change_id>` コミットあり） | apply / archive を飛ばして merge のみ実行 |
| **Merged** | すでに main ブランチへマージ済み | 全操作をスキップし、ワークスペースをクリーンアップ |

この状態検出により次が保証されます:
- 同じワークスペースで複数回実行しても安全で、同じ結果になる（冪等性）
- 手動で archive / merge した変更も正しく検出され処理される
- 中断された操作が正しい地点から再開される
- 重複作業が発生しない

**状態検出の例:**

```bash
# apply 中に中断された場合 - 続きから再開
$ cflx run --parallel
# Workspace state: Applying (iteration 3/5)
# Action: iteration 4 から apply を再開

# 手動で archive 済みの場合 - apply / archive をスキップ
$ cflx run --parallel
# Workspace state: Archived
# Action: apply / archive をスキップし、main への merge のみ

# すでに main にマージ済み - cleanup のみ
$ cflx run --parallel
# Workspace state: Merged
# Action: 全操作をスキップし、ワークスペースをクリーンアップ
```

### コマンド実行キュー

オーケストレーターには、複数の AI エージェントコマンドを並列実行する際のリソース競合を防ぎ、一時的な失敗を扱うコマンド実行キューがあります。

**機能:**

1. **開始の段階化**: 同時リソースアクセスを防ぐため、設定可能な遅延を入れてコマンド開始
2. **自動リトライ**: モジュール解決やネットワーク問題など一時的エラーで失敗したコマンドを自動再試行

**設定:**

```jsonc
{
  // コマンド開始間の遅延（ミリ秒）
  // デフォルト: 2000（2 秒）
  "command_queue_stagger_delay_ms": 2000,

  // 失敗したコマンドの最大リトライ回数
  // デフォルト: 2
  "command_queue_max_retries": 2,

  // リトライ間隔（ミリ秒）
  // デフォルト: 5000（5 秒）
  "command_queue_retry_delay_ms": 5000,

  // 実行時間がこの閾値未満ならリトライ対象（秒）
  // 短時間失敗は環境 / 起動問題のことが多い
  // デフォルト: 5
  "command_queue_retry_if_duration_under_secs": 5,

  // 自動リトライを発火させるエラーパターン（正規表現）
  // デフォルト: モジュール解決、レジストリ、ロック関連
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

- **段階的開始**: 各コマンドは前のコマンド開始時刻から一定時間待つため、共有リソース（例: `~/.cache/opencode/node_modules`）への同時アクセスを防ぎます
- **リトライロジック**: 次の場合にコマンドを再試行します
  - 設定したエラーパターンに一致する（例: `Cannot find module`）
  - 短時間で終了する（デフォルトでは 5 秒未満）ため、起動 / 環境問題の可能性が高い
- **リトライしないケース**: 長時間（5 秒超）実行され、エラーパターンにも一致しない場合は、論理エラーの可能性が高いため再試行しません

**例 - モジュール解決競合の防止:**

```bash
# キューなし: 複数コマンドが同時開始
# → 競合: すべてが同時に node_modules を更新しようとする
# → 結果: "Cannot find module" エラー

# キューあり（デフォルト）: コマンドが 2 秒間隔で開始
# → 最初のコマンドが node_modules を更新
# → 後続コマンドは安定した環境を利用
# → 結果: 競合なし
```

**例 - 一時的ネットワークエラーの処理:**

```bash
# Error: ETIMEDOUT registry.npmjs.org
# → リトライパターンに一致
# → 5 秒後に自動リトライ
# → 多くの場合リトライで成功
```

## Web UI とダッシュボード

Web UI は HTTP / WebSocket ベースの監視ダッシュボードです。通常モード（`--web`）とサーバーモードの両方で利用できます。

### Web UI を有効化する

```bash
# 通常モード: TUI + Web UI
cflx --web

# 通常モード: ヘッドレス実行 + Web UI
cflx run --web

# カスタムポートと bind アドレス
cflx --web --web-port 9000 --web-bind 0.0.0.0
```

デフォルトポート（0）を使うと、OS が利用可能なポートを自動割当します。
実際に bind されたアドレスはサーバー起動時にログへ出力されます。

サーバーモード（`cflx server`）では、Web UI は設定ポートで常に利用できます。

### ダッシュボード機能

- **ダッシュボード UI**: `http://localhost:<port>/` で進捗を確認
- **リアルタイム更新**: WebSocket 接続で進捗をライブ更新
- **REST API**: 状態をプログラムから取得可能
- **QR コードポップアップ**: TUI で `w` を押すとモバイル向けの QR コードを表示

### REST API エンドポイント

| エンドポイント | メソッド | 説明 |
|----------|--------|-------------|
| `/api/health` | GET | ヘルスチェック |
| `/api/state` | GET | オーケストレーター全状態 |
| `/api/changes` | GET | 進捗付きの変更一覧 |
| `/api/changes/{id}` | GET | 特定変更の詳細 |

完全な API 仕様は [OpenAPI ドキュメント](docs/openapi.yaml) を参照してください。

### WebSocket

リアルタイム状態更新のために `ws://localhost:<port>/ws` へ接続します。メッセージは以下形式の JSON です:

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

### ダッシュボード概要

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
│  │ add-feature-auth                    [IN_PROGRESS]           │
│  │ ████████████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  40%    │   │
│  │ 4/10 tasks                                               │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                 │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ fix-login-bug                       [COMPLETE]              │
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

- **統計バー**: 総数、完了、進行中、保留中の変更数を表示
- **変更カード**: 各変更に ID、進捗状態、プログレスバーを表示
- **リアルタイム更新**: WebSocket 接続経由で自動更新
- **接続状態表示**: 現在の WebSocket 接続状態（Connected / Disconnected）を表示
- **レスポンシブデザイン**: デスクトップ / モバイル両対応

### Web UI のトラブルシューティング

| 問題 | 解決策 |
|-------|----------|
| "Address already in use" | `--web-port 0`（デフォルト）で OS に自動割当させるか、未使用ポートを指定 |
| ダッシュボードが開かない | `--web` が有効か確認し、URL に正しいポートが含まれているか確認 |
| WebSocket が頻繁に切れる | ネットワークの安定性を確認。ダッシュボードは自動再接続します |
| 変更が更新されない | ページを再読込するか、オーケストレーターが実際に処理中か確認 |
| 別デバイスからアクセスできない | 外部接続を許可するには `--web-bind 0.0.0.0` を利用（ローカルネットワーク向け） |
| ブラウザコンソールで CORS エラー | クロスオリジン要求では通常の挙動です。サーバー側で CORS ヘッダーを処理します |

## サーバーモード詳細

### バックグラウンドサービス (`cflx service`)

`cflx service` を使うと、`cflx server` をユーザーレベルのバックグラウンドサービスとして導入・管理できます。

- macOS: `launchd` ユーザーエージェント
- Linux: `systemd --user` サービス
- Windows: タスクスケジューラ

```
cflx service <install|uninstall|status|start|stop|restart>
```

例:

```bash
# サービスをインストールして有効化
cflx service install

# バックグラウンドサーバーを開始または再起動
cflx service start
cflx service restart

# サービス状態を確認
cflx service status

# 停止または削除
cflx service stop
cflx service uninstall
```

補足:

- `install`、`start`、`restart` は、サービスマネージャを触る前に有効なグローバル `server` 設定を検証します。
- macOS では `~/Library/LaunchAgents/com.conflux.cflx-server.plist` に plist を書き込みます。
- Linux では `~/.config/systemd/user/cflx-server.service` に unit file を書き込みます。
- 永続的なサーバー設定は、インストール前に `~/.config/cflx/config.jsonc` などのグローバル設定ファイルで行ってください。

## コマンドラインリファレンス

**init サブコマンドのオプション:**
```
Options:
  -t, --template <TEMPLATE>  使用するテンプレート [default: claude] [possible values: claude, opencode, codex]
  -f, --force                既存の設定ファイルを上書き
```

**check-conflicts サブコマンドのオプション:**
```
Options:
  -j, --json  結果を JSON 形式で出力
```

**install-skills サブコマンド:**

リポジトリの `skills/` ディレクトリに含まれる bundled agent skills を、標準の `.agents/skills` 配下へインストールします。

```
cflx install-skills [--global]
```

オプション:
```
  --global  プロジェクトスコープ（./.agents/skills）ではなくグローバルスコープ（~/.agents/skills）へインストール
```

例:
```bash
# bundled skills をインストール（プロジェクトスコープ -> ./.agents/skills）
cflx install-skills

# bundled skills をインストール（グローバルスコープ -> ~/.agents/skills）
cflx install-skills --global
```

スキルはリポジトリ直下の `skills/` ディレクトリから検出されます。各スキルには `name` と `description` の frontmatter を持つ `SKILL.md` が必要です。インストールのたびに lock file（`.agents/.skill-lock.json` または `~/.agents/.skill-lock.json`）が更新され、インストール済みスキルのバージョンを追跡します。

優先順位: CLI 引数 > 環境変数 > デフォルト値

## エラーハンドリング

| エラー | 動作 |
|-------|----------|
| エージェントコマンド失敗 | 3 回リトライ後に失敗扱い |
| Apply コマンド失敗 | その変更を失敗扱いにし、他は継続 |
| Archive コマンド失敗 | その変更を失敗扱いにし、他は継続 |
| LLM 分析失敗 | 進捗ベース選択へフォールバック |
| 全変更が失敗 | エラー終了 |

## トラブルシューティング

### "No changes found"

- `openspec list` を実行して変更が存在するか確認
- 正しいディレクトリにいるか確認

### "Agent command failed"

- AI エージェントがインストール済みか確認（例: `which claude`）
- 手動テスト: `claude -p "echo test"`
- 設定ファイル `.cflx.jsonc` を確認

### "All changes failed"

- ログから具体的なエラーを確認
- 単一変更で試す: `--change <id>`

## インストール

```bash
cargo install cflx
```

これによりオーケストレーターがビルドされ、Cargo の bin ディレクトリ（通常は `~/.cargo/bin`）にインストールされます。

## ドキュメント

| ドキュメント | 説明 |
|----------|-------------|
| [Usage Examples](docs/guides/USAGE.md) | クイックスタートと使用例 |
| [Contributing Guide](CONTRIBUTING.md) | ローカル開発セットアップとコントリビューターワークフロー |
| [Development Guide](docs/guides/DEVELOPMENT.md) | ビルド手順とプロジェクト構造 |
| [Release Guide](docs/guides/RELEASE.md) | リリース作成方法 |
| [API Specification](docs/openapi.yaml) | Web UI と API の OpenAPI 仕様 |

内部ドキュメント（並列実行監査）は `docs/audit/` にあります。

## 今後の機能強化

- [ ] リカバリ / 再開のための状態永続化
- [x] 独立変更の並列実行（Git worktree 使用）
- [ ] Slack / Discord 通知
- [ ] 最大反復回数制限（無限ループ防止）
- [ ] 手動優先度オーバーライド
- [ ] 実行計画付き dry-run の強化
- [ ] 監視用 Web UI

## ライセンス

MIT

## コントリビューション

コントリビューション歓迎です。ローカルセットアップ、Git hooks、リポジトリ構成については [CONTRIBUTING.md](CONTRIBUTING.md) を参照してください。

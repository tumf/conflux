# 設計: エージェントコマンドの設定可能化

## Context

OpenSpec Orchestrator は現在 OpenCode に密結合されている。他のエージェントツール（Codex、Claude Code、Aider など）を使いたいユーザーのニーズに応えるため、コマンド実行を設定可能にする必要がある。

## Goals / Non-Goals

### Goals

- JSONC 形式の設定ファイルでエージェントコマンドを定義可能にする
- `apply`、`archive`、`analyze_dependencies` の3つの操作を設定可能にする
- `{change_id}` と `{prompt}` プレースホルダーで動的な値挿入をサポート
- プロジェクト設定とグローバル設定の両方をサポート
- 後方互換性を維持（設定なしで既存動作を継続）

### Non-Goals

- 複数エージェントの同時使用
- GUI による設定
- リモート設定の取得

## Decisions

### 1. 設定ファイル形式: JSONC

**理由**: JSON with Comments は可読性が高く、既存のエコシステム（VSCode、OpenCode など）と親和性がある。

**代替案**:
- TOML: Rust エコシステムで人気だが、コメント構文が異なる
- YAML: インデントベースで構造が崩れやすい

### 2. 設定ファイルパス（優先順位順）

| 優先度 | パス | 用途 |
|--------|------|------|
| 1 (高) | `.cflx.jsonc` | プロジェクト固有設定 |
| 2 (低) | `~/.config/cflx/config.jsonc` | グローバル設定 |

**理由**:
- プロジェクト設定は `.cflx.jsonc` でルートに配置（ドットファイルで目立たない）
- グローバル設定は `~/.config/cflx/` ディレクトリ内に配置（XDG Base Directory 準拠）
- プロジェクト設定が存在すればそちらを優先、なければグローバルを使用

**代替案**:
- `.opencode/orchestrator.jsonc`: opencode 非依存なのに `.opencode` フォルダは違和感あり（却下）
- `cflx.jsonc` (ドットなし): ルートが煩雑になる

### 3. コマンドテンプレート形式

```jsonc
{
  // apply コマンド: {change_id} は実行時に置換される
  "apply_command": "codex run 'openspec-apply {change_id}'",

  // archive コマンド
  "archive_command": "codex run 'conflux:archive {change_id}'",

  // 依存関係分析コマンド: {prompt} は分析プロンプトに置換される
  "analyze_command": "claude '{prompt}'"
}
```

**理由**: シェルコマンド形式で柔軟性が高い。既存の OpenCode 以外のツールも簡単に統合可能。

### 4. プレースホルダー

| プレースホルダー | 説明 | 使用可能なコマンド |
|-----------------|------|-------------------|
| `{change_id}` | 変更ID | apply, archive |
| `{prompt}` | 分析プロンプト | analyze |

### 5. デフォルト動作（設定ファイルなし）

設定ファイルが存在しない場合、現在と同じ opencode ベースの動作を維持：

```jsonc
{
  "apply_command": "opencode run '/openspec-apply {change_id}'",
  "archive_command": "opencode run '/conflux:archive {change_id}'",
  "analyze_command": "opencode run --format json '{prompt}'"
}
```

## Risks / Trade-offs

| リスク | 影響 | 軽減策 |
|--------|------|--------|
| 設定ミスによるコマンド失敗 | 中 | 設定バリデーションを実装、エラーメッセージを明確に |
| JSONC パーサーの追加依存 | 低 | `jsonc-parser` または `serde_json` + コメント除去で対応 |
| セキュリティ（コマンドインジェクション） | 中 | `{change_id}` のサニタイズ、シェル経由を避ける |
| グローバル設定ディレクトリの作成 | 低 | 初回使用時に自動作成、または手動作成を案内 |

## Migration Plan

1. 設定ファイルはオプション（フォールバック動作を維持）
2. 既存ユーザーは変更不要で動作継続
3. 新機能を使いたいユーザーのみ設定ファイルを作成
4. `--init-config` オプションでサンプル設定ファイルを生成可能にする（将来検討）

## Open Questions

- [x] 複数の設定ファイルパスをサポートするか？ → Yes（プロジェクト + グローバル）
- [x] `{prompt}` プレースホルダーをサポートするか？ → Yes
- [ ] 環境変数の展開をサポートするか？（例: `$HOME`, `${AGENT_PATH}`）→ 将来検討

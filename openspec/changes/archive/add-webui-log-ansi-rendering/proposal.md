# Change: WebUI Logs パネルで ANSI エスケープコードをカラー表示する

**Change Type**: implementation

## Why

WebUI の Logs パネルは子プロセス（AI エージェント等）の stdout/stderr をプレーンテキストとして描画している。子プロセスが ANSI エスケープシーケンス（色・太字等）を出力すると `[0m` `[33m` のような生文字列がそのまま表示され、ログの可読性が著しく低下する。

## What Changes

- `dashboard/src/components/LogEntry.tsx` で `ansi-up` ライブラリを使い ANSI → HTML 変換を行い、色付きログ表示にする
- `ansi-up` を dashboard の依存に追加する
- XSS 対策として `ansi-up` の組み込みエスケープ機能を利用する
- 既存の `LogsPanel` のスクロール・フィルタ・レベル表示機能はそのまま維持

## Impact

- Affected specs: `web-monitoring`
- Affected code: `dashboard/src/components/LogEntry.tsx`, `dashboard/package.json`

## Acceptance Criteria

1. ANSI カラーコード（16 色 + 太字・下線等）を含むログメッセージが色付きで表示される
2. ANSI コードを含まないログメッセージは従来通り表示される
3. `<script>` 等の悪意あるHTMLタグがサニタイズされ、XSS が発生しない
4. 既存の LogsPanel UI（スクロール、レベルバッジ、タイムスタンプ）が維持される

## Out of Scope

- バックエンド側での ANSI コード除去（色情報を保持したいため）
- ターミナルタブ（`TerminalTab.tsx`）への変更（既に xterm.js で対応済み）
- 256 色 / TrueColor の完全対応（標準 16 色 + 基本装飾のみ対象）

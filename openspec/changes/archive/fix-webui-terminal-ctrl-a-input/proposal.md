# Fix WebUI Terminal Ctrl+A Browser Default Action Hijacking

**Change Type**: implementation

## Problem / Context

WebUI の仮想ターミナル（`dashboard/src/components/TerminalTab.tsx`）で Ctrl+A を押すと、ブラウザの「全選択」デフォルトアクションが発火する。これにより xterm.js の内部状態が壊れ、それ以降のキー入力が以前に入力した文字をリピートし続ける状態になる。

Ctrl+A はシェルの「行頭移動」(readline `beginning-of-line`) であり、ターミナルにフォーカスがある場合はブラウザではなく PTY に送信されるべきである。

## Root Cause

`TerminalTab.tsx` では xterm.js の `onData` ハンドラで入力を WebSocket 経由で PTY に転送しているが、`attachCustomKeyEventHandler` によるブラウザデフォルト動作の抑制を行っていない。そのため Ctrl+A が `document.execCommand('selectAll')` として処理され、xterm.js の内部バッファとフォーカス状態が不整合になる。

## Proposed Solution

xterm.js の `attachCustomKeyEventHandler` API を使用し、ターミナルにフォーカスがある時にシェル操作キーバインド（Ctrl+A, Ctrl+E, Ctrl+K, Ctrl+U, Ctrl+L, Ctrl+R, Ctrl+D, Ctrl+W 等）のブラウザデフォルト動作を抑制する。

例外として、Ctrl+C は選択テキストがある場合のみブラウザのコピー動作を許可し、選択がない場合は SIGINT として PTY に送信する。

## Acceptance Criteria

1. ターミナルにフォーカスがある状態で Ctrl+A を押すと、ブラウザの全選択が発火せず、カーソルが行頭に移動する
2. Ctrl+A を押した後も以降のキー入力が正常に動作する（リピート現象が発生しない）
3. Ctrl+E（行末移動）、Ctrl+K（行末まで削除）、Ctrl+U（行頭まで削除）等の readline キーバインドが正しく PTY に送信される
4. Ctrl+C は選択テキストがある場合にブラウザのコピー動作を許可する
5. Ctrl+Shift+C / Ctrl+Shift+V 等のターミナル外コピペショートカットは影響を受けない

## Out of Scope

- サーバー側（`terminal.rs`, `api.rs`）の変更
- ターミナルの色やフォント等のスタイル変更
- xterm.js のバージョンアップ

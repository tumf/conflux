# Fix WebUI Terminal Shell Keybindings (Ctrl+A etc.)

**Change Type**: implementation

## Problem / Context

WebUI の仮想ターミナル (`dashboard/src/components/TerminalTab.tsx`) で Ctrl+A を押しても行頭に移動しない。Ctrl+E（行末）、Ctrl+K（行末まで削除）等の readline キーバインドも同様に動作しない。

## Root Cause

`attachCustomKeyEventHandler` のコールバックで、シェル制御キー（Ctrl+A, E, K, U, L, R, D, W）に対して `return false` を返している。

xterm.js の `attachCustomKeyEventHandler` API では:
- `true` を返す → xterm.js がキーを処理し、PTY に制御コードを送信する
- `false` を返す → xterm.js がキーを無視し、ブラウザのデフォルト動作に委ねる

現在のコードは `false` を返しているため、Ctrl+A は xterm.js に無視され PTY に 0x01 が送信されず、代わりにブラウザの「全選択」が発火する。

```typescript
// 現在のコード（86-88行目）— 誤り
if (isModifierKey && SHELL_CONTROL_KEYS.has(key)) {
  return false; // ← xterm.js が無視 → ブラウザが全選択
}
```

Ctrl+C のハンドリング（90-95行目）も同様に逆で、選択テキストがある場合に `return true`（xterm.js が処理 → SIGINT 送信）、ない場合に `return false`（ブラウザに委ねる）となっている。

## Proposed Solution

`attachCustomKeyEventHandler` の戻り値を正しい方向に修正する:

1. シェル制御キー（Ctrl+A/E/K/U/L/R/D/W）→ `return true`（xterm.js が処理、PTY に送信）
2. Ctrl+C（選択テキストあり）→ `return false`（xterm.js は処理せず、ブラウザがコピー）
3. Ctrl+C（選択テキストなし）→ `return true`（xterm.js が処理、SIGINT を PTY に送信）

## Acceptance Criteria

1. ターミナルにフォーカスがある状態で Ctrl+A を押すとカーソルが行頭に移動する
2. Ctrl+E で行末に移動する
3. Ctrl+K, Ctrl+U 等の readline キーバインドが正しく動作する
4. Ctrl+C は選択テキストがある場合にブラウザのコピー動作を行い、ない場合は SIGINT を送信する
5. `cd dashboard && npm run build` が成功する

## Out of Scope

- サーバー側（`terminal.rs`, `api.rs`）の変更
- xterm.js のバージョンアップ
- 新規キーバインドの追加

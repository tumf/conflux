---
change_type: implementation
priority: medium
dependencies: []
references:
  - dashboard/src/components/TerminalTab.tsx
  - dashboard/public/debug-ws.js
  - openspec/specs/webui-terminal/spec.md
  - openspec/changes/archive/fix-webui-terminal-ctrl-a-input/proposal.md
---

# Fix WebUI Terminal Helper Textarea Accumulation After Control Input

**Change Type**: implementation

## Problem / Context

WebUI の仮想ターミナルで Ctrl+A などの制御入力を送信した後、xterm.js の hidden helper textarea (`textarea.xterm-helper-textarea`) に直前の入力内容が残留し、次の printable key の `input` イベントで stale text 全体が再送されることがある。

この現象は PTY 側ではなく表示側でのみ発生し、ユーザーには「Ctrl+A のあとに Space や X を押すと以前の `AAA` が繰り返し出力される」ように見える。xterm.js v6 / Chrome on macOS の組み合わせで再現し、`textarea.value` が `"AAA"` → `"AAA "` → `"AAA X"` のように増殖する。

## Proposed Solution

`dashboard/src/components/TerminalTab.tsx` で xterm.js の helper textarea を明示的に監視し、`onData` で PTY へ入力を転送した直後に textarea の残留値を非同期でクリアする。

併せて、修正対象を helper textarea の stale buffer 抑止に限定し、ブラウザの keybinding 抑制ロジックとは責務を分離する。これにより、Ctrl+A 自体が正しく `\x01` として処理されるケースでも、後続 `input` で stale text が再送される表示崩れを防ぐ。

## Acceptance Criteria

1. WebUI ターミナルで `AAA` 入力後に Ctrl+A、続けて Space または任意の printable key を押しても、以前の入力文字列が繰り返し再送・再表示されない
2. Ctrl+A 自体の PTY 転送（`\x01`）を阻害せず、helper textarea の stale value だけがクリアされる
3. 修正は `dashboard/src/components/TerminalTab.tsx` のクライアント側入力処理に限定され、サーバー側 PTY/WebSocket 実装を変更しない
4. `dashboard/public/debug-ws.js` などの既存デバッグ手段で、後続入力時に textarea が蓄積し続けないことを確認できる
5. `cd dashboard && npm run build` が成功する

## Out of Scope

- xterm.js のアップグレードや upstream 修正
- サーバー側 PTY / WebSocket プロトコルの変更
- Ctrl+C / Ctrl+E など他のキーバインド意味論の再設計

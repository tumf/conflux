---
change_type: implementation
priority: medium
dependencies: []
references:
  - dashboard/src/components/TerminalPanel.tsx
  - dashboard/src/components/TerminalTab.tsx
  - openspec/specs/code-maintenance/spec.md
  - openspec/specs/webui-terminal/spec.md
---

# Terminal hooks の依存関係整理

**Change Type**: implementation

## Problem/Context

WebUI terminal components では React hook の依存配列警告を `eslint-disable-next-line react-hooks/exhaustive-deps` で抑制している。terminal session restore、auto-create、WebSocket/xterm lifecycle は副作用が強く、依存関係を抑制したままだと将来の session/root 切替や resize 処理の修正時に stale closure の退行を招きやすい。

候補ランキングでは、lint bypass が少数箇所に集中しており、UI の外部挙動を変えずに hook 境界を整理できるため、小さく独立したリファクタ対象として選定した。

### Evidence

- `dashboard/src/components/TerminalPanel.tsx:92` で restore sessions effect の exhaustive deps を無効化している。
- `dashboard/src/components/TerminalPanel.tsx:104` で auto-create effect の exhaustive deps を無効化している。
- `dashboard/src/components/TerminalPanel.tsx:123` の `handleCreateTab` は effect から呼ばれるが、依存関係抑制により stale closure を見落としやすい。
- `dashboard/src/components/TerminalTab.tsx:179` で WebSocket/xterm lifecycle effect の exhaustive deps を無効化している。

## Proposed Solution

- terminal restore、auto-create、WebSocket/xterm lifecycle の現行挙動を characterization test で固定する。
- effect 内で必要な値を明示する、安定 callback/ref を使う、または小さな custom hook へ抽出することで lint suppression を不要にする。
- session/root/project の切替時に既存と同じ session 作成・選択・接続・cleanup が行われることを維持する。
- UI 表示、REST/WebSocket API payload、xterm 表示メッセージは変更しない。

## Acceptance Criteria

- `TerminalPanel.tsx` と `TerminalTab.tsx` の `react-hooks/exhaustive-deps` disable コメントが不要になる、または明確に局所化された正当な例外だけになる。
- panel expand 時の terminal session 自動作成、既存 session restore、root 切替時の active tab 選択が既存と同等である。
- terminal session の WebSocket 接続、入力送信、resize 送信、cleanup が既存と同等である。
- dashboard の lint/type/test/build が対象範囲で成功する。

## Explicit Completion Conditions

- terminal hook の副作用境界が、restore、auto-create、active tab sync、xterm session lifecycle として追跡できる構造になっている。
- hook dependency suppression の削減が差分で確認できる。
- Characterization test または既存テストで session restore/auto-create/root switching と terminal lifecycle が確認されている。
- REST/WebSocket API shape と表示テキストの意図しない変更がない。

## Out of Scope

- terminal UI デザイン、xterm 設定、REST/WebSocket API の変更。
- 新しい terminal 機能の追加。
- backend terminal session 管理の変更。

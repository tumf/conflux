# Design: Terminal hooks の依存関係整理

## 方針

UI と通信 contract は変えず、React hook の副作用境界だけを整理する。lint suppression を消すこと自体を目的化せず、session lifecycle の characterization を先に固定する。

## 分割候補

- terminal session restore hook。
- terminal auto-create hook。
- active tab synchronization helper。
- xterm/WebSocket lifecycle hook。

## Trade-offs

- hook 抽出によりコンポーネントの見通しは良くなるが、テストでは custom hook の内部よりも外部観測可能な REST/WebSocket/xterm 呼び出しを固定する。
- backend API や xterm の設定変更は行わず、依存関係整理に限定する。

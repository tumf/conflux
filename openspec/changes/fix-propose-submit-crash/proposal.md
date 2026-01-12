# Change: 提案送信（Ctrl+S）時のクラッシュ防止と送信操作の整理

## Why

提案入力モードで `Ctrl+S` を押すとアプリがクラッシュするため、送信操作の信頼性が損なわれています。送信キーを明確化し、失敗時の挙動も含めて安定した入力体験にする必要があります。

## What Changes

- 提案送信を `Ctrl+S` に統一し、クラッシュしない実行経路にする
- 送信失敗時は入力を保持したままProposingモードを維持する
- 改行入力とキーヒント表示の期待値を整理する

## Impact

- Affected specs: `tui-propose-input`
- Affected code:
  - `src/tui/runner.rs`（Proposing時のキー処理と送信制御）
  - `src/tui/state/mod.rs`（送信処理とモード遷移）
  - `src/tui/render.rs`（モーダルとフッターのキーヒント表示）

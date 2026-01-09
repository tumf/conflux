# Change: 動的キュー解除機能の追加

## Why

現在のTUIでは、実行モード中にキュー待機中（Queued）の変更をキューから取り除くことができない。これにより、誤ってキューに追加した変更や、優先度が変わった変更を柔軟に管理できない問題がある。

## What Changes

- 実行モードでキュー待機中（Queued状態）の変更に対してSpaceキーでキュー解除できるようにする
- 処理中（Processing）またはアーカイブ中の変更は引き続き変更不可とする
- 既存の「キューへ追加」機能と統合し、トグル動作として実装する

## Impact

- Affected specs: `cli` (動的実行キュー要件の修正)
- Affected code: `src/tui.rs` (toggle_selection メソッド)

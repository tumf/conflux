## Implementation Tasks

- [ ] `dashboard/src/components/TerminalTab.tsx` の `attachCustomKeyEventHandler` で、シェル制御キー (Ctrl+A/E/K/U/L/R/D/W) の戻り値を `false` → `true` に修正する (verification: Ctrl+A で行頭移動が動作する)
- [ ] 同ハンドラの Ctrl+C 分岐を修正: 選択テキストありの場合 `return false`（ブラウザコピー）、なしの場合 `return true`（SIGINT 送信）にする (verification: 選択あり Ctrl+C でクリップボードコピー、選択なし Ctrl+C で SIGINT)
- [ ] `cd dashboard && npm run build` が成功する (verification: ビルドエラーなし)

## Implementation Tasks

- [ ] `TerminalTab.tsx` の Terminal 初期化直後に `attachCustomKeyEventHandler` を追加し、Ctrl+A/E/K/U/L/R/D/W のブラウザデフォルト動作を抑制する (verification: `dashboard/src/components/TerminalTab.tsx` に `attachCustomKeyEventHandler` 呼び出しが存在し、Ctrl+A で行頭移動が動作する)
- [ ] Ctrl+C のハンドリングを追加: `term.hasSelection()` が true の場合のみブラウザのコピー動作を許可し、それ以外は xterm.js が処理する (verification: 選択テキストがある状態で Ctrl+C がクリップボードにコピーされ、選択がない場合は SIGINT が送信される)
- [ ] `cargo clippy -- -D warnings` および `cd dashboard && npm run build` が成功する (verification: ビルドエラーなし)

## Future Work

- Ctrl+V（ペースト）の挙動最適化（xterm.js のブラウザ統合による自動処理のため現時点では不要）

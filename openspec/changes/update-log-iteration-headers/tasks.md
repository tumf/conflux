## 1. Specification
- [ ] 1.1 既存のログ仕様（tui-architecture / update-loop-history-logs）を確認し、イテレーション必須化の影響範囲を整理する（確認: openspec/specs/tui-architecture/spec.md と関連 spec を読み、差分メモを残す）

## 2. TUIログヘッダーの統一
- [ ] 2.1 直列アーカイブループのログ出力に iteration を必須付与する（確認: src/tui/orchestrator.rs で archive 出力が `.with_iteration(attempt)` を使う）
- [ ] 2.2 並列アーカイブ出力イベントに iteration を必須付与する（確認: src/parallel/executor.rs の ArchiveOutput で iteration が attempt と一致する）
- [ ] 2.3 analysis ログ出力の iteration を必須付与する（確認: analysis 出力イベントが iteration を常に持ち、TUIで [analysis:N] が表示される）

## 3. Web/TUI状態反映
- [ ] 3.1 Web/TUI の iteration 表示が archive/analysis の更新で反映されることを確認する（確認: src/web/state.rs の iteration_number 更新がイベントに追随する）

## 4. Validation
- [ ] 4.1 `npx @fission-ai/openspec@latest validate update-log-iteration-headers --strict` が成功する（確認: コマンドが exit 0 で終了する）

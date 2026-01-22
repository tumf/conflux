## 1. Implementation
- [x] 1.1 `src/tui/state/logs.rs` の `add_log` で、オートスクロール無効時に表示位置を固定し、バッファトリム時にオフセットをクランプする（確認: `add_log` のオフセット更新ロジックを目視確認）
- [x] 1.2 ログ追加とトリム時に表示が固定されることを検証するユニットテストを `src/tui/state/mod.rs` に追加する（確認: `cargo test test_log_scroll_offset_freeze`）

## 1. Implementation
- [ ] 1.1 `verify_archive_completion` が `openspec/changes/{change_id}` を優先して未アーカイブ判定するよう更新する
- [ ] 1.2 並列/TUI/逐次の archive 検証で同じ判定が反映されることを確認する
- [ ] 1.3 既存の archive 検証テストを修正し、changes と archive が両方存在する場合は未アーカイブとして扱う

## 2. Validation
- [ ] 2.1 `cargo test`（該当モジュール中心）を実行する

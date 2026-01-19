## 1. Implementation
- [x] 1.1 進捗取得の共通ヘルパーを追加し、worktree → archive → base の順序を維持する (verify: src/task_parser.rs に新関数とフォールバック順が記載されている)。
- [x] 1.2 TUI と Web の進捗取得呼び出しを共通ヘルパーに置換する (verify: src/tui/runner.rs, src/tui/state/events.rs, src/web/state.rs で新関数を呼び出している)。
- [x] 1.3 Web API の Not Found 応答生成を共通ヘルパーに集約する (verify: src/web/error.rs と src/web/api.rs に共通関数が実装されている)。
- [x] 1.4 進捗取得と Not Found 応答の同一性を確認できるテストを追加・更新する (verify: cargo test task_parser と cargo test web で該当テストが成功する)。
- [x] 1.5 cargo fmt / cargo clippy -- -D warnings / cargo test を実行し、既存挙動が維持されていることを確認する。

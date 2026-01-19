## 1. Implementation
- [ ] 1.1 Agent の責務を整理し、分割対象と移動方針を文書化する (verify: proposal と design に分類が記載されている)。
- [ ] 1.2 agent モジュールを runner/output/history/prompt に分割し、mod.rs で再公開する (verify: src/agent/mod.rs が入口になっている)。
- [ ] 1.3 各責務の関数を分割先モジュールに移動する (verify: src/agent/*.rs に関数が配置されている)。
- [ ] 1.4 既存テストを分割先に移動し、必要な追加テストを作成する (verify: cargo test agent 関連テストが成功する)。
- [ ] 1.5 cargo fmt / cargo clippy -- -D warnings / cargo test を実行し、既存挙動が維持されていることを確認する。

## 1. Implementation
- [ ] 1.1 parallel 内の責務（workspace/queue/merge/state）を整理し、分割対象と移動方針を文書化する (verify: proposal と design に対象モジュール一覧が明記されている)。
- [ ] 1.2 並列実行の主要ロジックを専用サブモジュールに移動し、公開 API を mod.rs で再公開する (verify: src/parallel/mod.rs が再公開中心になっている)。
- [ ] 1.3 動的キュー・マージ・ワークスペース管理のロジックを分割モジュールに移動する (verify: 新規ファイル src/parallel/workspace.rs, dynamic_queue.rs, merge.rs に関数が配置されている)。
- [ ] 1.4 parallel のテストを tests サブモジュールへ移動し、対象別に分割する (verify: src/parallel/tests/* が追加され、cargo test の対象が移動している)。
- [ ] 1.5 cargo fmt / cargo clippy -- -D warnings / cargo test を実行し、既存挙動が維持されていることを確認する。

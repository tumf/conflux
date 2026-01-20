## 1. Implementation
- [x] 1.1 Git コマンド群の責務を整理し、分割対象と移動方針を文書化する (verify: proposal と design に分類が記載されている)。
- [x] 1.2 commands モジュールをサブディレクトリに分割し、mod.rs で再公開する (verify: src/vcs/git/commands/mod.rs が入口になっている)。
- [x] 1.3 basic/commit/worktree/merge 各モジュールへ関数を移動する (verify: 新規ファイル src/vcs/git/commands/*.rs に関数が配置されている)。
- [x] 1.4 既存テストを分割先に移動し、必要な追加テストを作成する (verify: cargo test vcs::git の関連テストが成功する)。
- [x] 1.5 cargo fmt / cargo clippy -- -D warnings / cargo test を実行し、既存挙動が維持されていることを確認する。

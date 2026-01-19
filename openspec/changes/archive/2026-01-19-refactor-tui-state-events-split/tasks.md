## 1. Implementation
- [x] 1.1 イベント種別ごとの責務を整理し、分割対象と移動方針を文書化する (verify: proposal と design にイベント分類が記載されている)。
- [x] 1.2 AppState のイベント処理を分割モジュールに移動し、mod.rs で再公開する (verify: src/tui/state/events/mod.rs が入口になっている)。
- [x] 1.3 進捗更新・完了・リフレッシュ等のイベント処理を専用ファイルに移動する (verify: src/tui/state/events/*.rs に処理が配置されている)。
- [x] 1.4 既存テストを分割先に整理し、必要な追加テストを作成する (verify: cargo test で AppState イベント関連のテストが成功する)。
- [x] 1.5 cargo fmt / cargo clippy -- -D warnings / cargo test を実行し、既存挙動が維持されていることを確認する。

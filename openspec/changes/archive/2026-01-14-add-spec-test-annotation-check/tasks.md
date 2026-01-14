## 1. 仕様と運用の整備
- [x] 1.1 `OPENSPEC` アノテーションのフォーマットを確定する（`spec_path#req_slug/scenario_slug`）
- [x] 1.2 slug 化ルールと `UI-only` 判定ルール（大小無視など）を確定する

## 2. チェッカー実装
- [x] 2.1 `openspec/specs/**/spec.md` から Requirement/Scenario と UI-only を抽出するパーサを実装する
- [x] 2.2 `src/**` と `tests/**` から `// OPENSPEC:` 参照を抽出するスキャナを実装する
- [x] 2.3 突合してレポートを出す（不足/壊れた参照）

## 3. 実行経路の統合
- [x] 3.1 `cargo test` で実行できるよう、専用テスト（またはテスト補助バイナリ）として組み込む
- [x] 3.2 CI で失敗時に差分が分かる出力形式にする（specパス/req_slug/scenario_slug を表示）

## 4. 段階的な導入
- [x] 4.1 既存テストの一部（例: `tests/e2e_tests.rs`）に `OPENSPEC` アノテーションを付与して運用開始する
- [x] 4.2 重要シナリオから順にアノテーションを拡充する

## 5. 検証
- [x] 5.1 `cargo test` を実行し、チェッカーが期待通りに不足/壊れた参照を検出できることを確認する
- [x] 5.2 `cargo fmt` と `cargo clippy` を実行して品質を担保する

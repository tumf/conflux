## 1. キャラクタリゼーション
- [x] 1.1 現行の設定探索優先順位を固定するテストを追加する（確認: 設定関連ユニットテストで優先順位が再現される）
- [x] 1.2 非推奨API経由でも現状の外部挙動が変わらないことを確認する回帰テストを追加する（確認: 既存利用パターンが失敗しない）

## 2. リファクタリング
- [x] 2.1 設定パス解決の内部経路を現行API中心へ整理し、責務の重複を減らす（確認: 実装内で優先順位ロジックの定義箇所が明確になる）
- [x] 2.2 非推奨ヘルパの扱いを縮小または薄い委譲に統一し、誤用しにくい構造にする（確認: 非推奨経路が独自ロジックを持たない）

## 3. 回帰確認
- [x] 3.1 設定読込・検証まわりのテストを実行し、グリーンであることを確認する（確認: configuration 系テスト成功）
- [x] 3.2 CLI公開挙動と設定ファイル互換性に差分がないことを確認する（確認: API/CLI変更なし）

## Acceptance #2 Failure Follow-up
- [x] `.cflx/acceptance-state.json` を整理し、`git status --porcelain` が空になる状態にする
- [x] `pre-commit` をこのワークスペースで実行可能にし、`pre-commit run --all-files` を成功させる
- [x] `parallel::tests::executor::test_idle_queue_addition_marks_reanalysis_and_enqueues_change` の失敗を解消し、`cargo test` をグリーンにする

# Change: 受け入れ判定の出力経路を単一化する

## Why
`acceptance` の判定結果と所見抽出が異なる経路で出力を扱っており、将来の修正時に不整合を生みやすい状態です。既存コードにも「実際のコマンド出力を使う」TODOが残っており、判定ロジックの保守性を下げています。

## What Changes
- `src/orchestration/acceptance.rs` にある受け入れ判定の出力取り扱いを、単一のデータフローに統一する
- `src/acceptance.rs` のパーサ連携ポイントを明確化し、判定ステータスと `findings` の由来を一致させる
- 既存挙動を固定するキャラクタリゼーションテストを先に追加し、リファクタ後も結果が変わらないことを確認する

## Impact
- Affected specs: `code-maintenance`
- Affected code: `src/orchestration/acceptance.rs`, `src/acceptance.rs`, 関連テスト
- API/CLI互換性: 変更なし

## Acceptance Criteria
- 既存の受け入れ結果（PASS/FAIL/CONTINUE/BLOCKED）の判定が回帰しない
- `cargo test` で受け入れ判定関連テストが成功する
- CLI引数・出力フォーマット・終了コードに変更がない

## Context
現状の native archive 実装は `openspec/changes/archive/<change_id>` を保存先に使っている一方、archive 解決と workspace state 判定は direct match と `YYYY-MM-DD-<change_id>` の両対応をすでに前提にしている。ユーザ要求は OpenSpec オリジナル互換の dated archive naming を標準に戻すことであり、生成側と読み取り側の標準を揃える必要がある。

## Goals / Non-Goals
- Goals:
  - native archive 生成先を `YYYY-MM-DD-<change_id>` 形式へ標準化する
  - archive 完了検証・archived lookup の既存互換性を維持する
  - 成功メッセージと destination conflict semantics を dated naming に合わせる
- Non-Goals:
  - 既存 archive directory 群の rename migration
  - proposal change ID 生成ルールの変更
  - archive 日付の設定化やタイムゾーン選択 UI の追加

## Decisions
- Decision: 新規 archive 生成は常に `openspec/changes/archive/YYYY-MM-DD-<change_id>` を使う
- Decision: 日付は archive 実行日のローカル日付を `YYYY-MM-DD` 書式で使う
- Decision: archived change 解決と archive completion verification は direct / dated の両形式対応を維持する
- Decision: 当日分の dated destination が既に存在する場合は別名へ自動退避せず明示エラーにする
- Alternatives considered: direct naming を標準のまま維持する / direct archive を優先しつつ設定で dated naming を切り替える

## Risks / Trade-offs
- ローカル日付依存のため、日付境界付近では実行環境の clock に archive 名が従う
- direct / dated 両対応を維持する間は lookup ロジックの複雑さが完全には消えない
- 非日付付き legacy archive は残るため、repository 内で命名が混在する移行期間が続く

## Migration Plan
- native archive 実装を dated naming へ切り替える
- 成功メッセージと lookup テストを dated 標準に合わせて更新する
- canonical spec に「生成は dated / 解決は direct+dated 互換」の要件を追加する

## Open Questions
- なし

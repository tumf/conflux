## 背景
仕様 (`openspec/specs/**/spec.md`) は `### Requirement:` / `#### Scenario:` の見出し階層で管理されている。
一方でテストは `src/**` と `tests/**` に分散しており、仕様↔テスト対応付けを「コードの近く」に置ける仕組みが欲しい。

## ゴール / 非ゴール
- ゴール
  - テストコードに仕様参照を付与し、機械的に「不足（UI-only除外）」と「壊れた参照」を検出する。
  - 参照キーは `spec_path#req_slug/scenario_slug` で統一する。
- 非ゴール
  - spec 側へ安定 ID を埋め込む（将来の選択肢としては残す）

## 主要な決定
### 決定: 見出しベース参照 (A) + slug 化
- テスト側のアノテーションは、spec の見出しテキストを直接書くのではなく slug を書く。
- チェッカー側も spec 見出しから同一ルールで slug を生成し、突合する。

例:
- spec: `openspec/specs/cli/spec.md` の
  - `### Requirement: run Subcommand`
  - `#### Scenario: Run with specific change`
- 参照: `openspec/specs/cli/spec.md#run-subcommand/run-with-specific-change`

### slug 生成ルール（案）
- 入力: `Requirement:` / `Scenario:` の見出し本文
- 変換:
  1. NFKC 正規化
  2. 小文字化
  3. 文字・数字以外を `-` に置換
  4. 連続する `-` を1つに圧縮
  5. 前後の `-` を除去
- 日本語は文字として残る（`running-中に-queued-change-を外す` のようになる）

### 決定: UI-only は spec 側で宣言
- Scenario ブロック（次の `#### Scenario:` まで）内に `UI-only` が含まれる場合、その Scenario はギャップ検出対象から除外する。
- 別ファイル（除外リスト）を持たず、spec の意図を source of truth にする。

## トレードオフ / リスク
- 見出し変更に弱い: Requirement/Scenario の名称を変更すると slug が変わり参照が壊れる。
  - ただし壊れた参照はチェッカーで検出されるため、追従漏れを早期に発見できる。
- slug の衝突: 同一 Requirement 配下で同名 Scenario があると衝突し得る。
  - その場合は spec 側の見出しを明確化する（運用ルールで禁止 or 例外処理を検討）。

## オープンクエスチョン
- `UI-only` の表記ゆれ（例: `UI only`, `UI_ONLY`, `UIのみ`）をどこまで許容するか。
- 参照範囲を `tests/**` のみにするか、`src/**` の `#[test]` も含めるか（現状の運用に合わせるなら後者が自然）。

# 変更提案: 仕様↔テスト対応付けをアノテーションで機械検証する

## なぜ (Why)
現状は `docs/test-coverage-mapping.md` により仕様シナリオとテストの対応関係を文書化しているが、
- 更新が手作業になりやすい
- 仕様側のシナリオ追加時に「テストを書くべきなのに未対応」を自動で検出しづらい
- テスト側の増加に伴い「どの仕様に対応しているか」が散逸しやすい

という課題がある。

## 何を変えるか (What Changes)
- Rust テストコードに、仕様シナリオ参照用のコメントアノテーション（例: `// OPENSPEC: ...`）を追加できるようにする。
- `openspec/specs/**/spec.md` の `Requirement` / `Scenario` 見出しを slug 化し、テスト側から `spec_path#req_slug/scenario_slug` で参照できるようにする。
- 仕様側の `UI-only` シナリオは、テスト未対応でもギャップ扱いにしない（検出対象から除外する）。
- 上記を検証するチェッカーを用意し、
  - UI-only ではないシナリオに対応テスト参照がないこと（不足）
  - テスト参照が spec に存在しないこと（壊れた参照）
  を機械的に検出できるようにする。

## 影響範囲 (Impact)
- Affected specs: `openspec/specs/testing/spec.md`
- Affected code: 今回は提案のみ（実装は別工程）

## 非ゴール (Non-Goals)
- 仕様ファイル自体に安定 ID（req_id / scenario_id）を導入すること
- すべての仕様シナリオのテスト必須化（本提案は「テストを書くべきなのに不足しているもの」を発見するための補助機構）

## 成功条件 (Success Criteria)
- `UI-only` でない仕様シナリオに対して、少なくとも1つの `OPENSPEC` 参照が存在しない場合にレポートされる。
- テスト側の `OPENSPEC` 参照が spec 側の見出しと対応していない場合にレポートされる。
- 既存の `docs/test-coverage-mapping.md` と併用でき、運用の移行が段階的に可能である。

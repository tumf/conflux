---
change_type: implementation
priority: high
dependencies: []
references:
  - src/parallel/merge.rs
  - src/vcs/git/commands/merge.rs
  - openspec/specs/parallel-merge/spec.md
  - openspec/specs/parallel-execution/spec.md
---

# Change: parallel merge の fast-forward 検証誤判定を修正

**Change Type**: implementation

## Why
parallel 実行では archive 後の merge 完了検証が `Merge change: <change_id>` という merge commit message の存在を必須にしており、fast-forward で base に統合済みの change でも `Missing merge commit message containing change_id(s)` として失敗します。これにより change は実際には取り込まれているのに、parallel merge が error 扱いになります。

## What Changes
- `src/parallel/merge.rs` の `verify_merge_commits()` を fast-forward 統合済み change を許容する検証へ拡張する
- merge commit message 必須のエラーは、本当に merge commit が必要な未完了ケースだけに限定する
- archive 後の parallel merge 経路で fast-forward 成功を再現する回帰テストを追加する
- 関連 spec を更新し、parallel merge の最終検証が fast-forward 統合を成功扱いすることを明記する

## Acceptance Criteria
- archive 後の parallel merge が fast-forward で完了した場合、`verify_merge_commits()` は失敗しない
- `Missing merge commit message containing change_id(s)` は fast-forward 統合済み change では出ない
- merge commit message が本当に必要な未完了ケースでは、従来どおり失敗する
- fast-forward 成功ケースの自動テストが追加される

## Out of Scope
- resolve retry 経路の merge commit 検証ロジック変更
- conflict resolution prompt や `--no-ff` 戦略自体の再設計

## Impact
- Affected specs: parallel-merge, parallel-execution
- Affected code: `src/parallel/merge.rs`, `src/vcs/git/commands/merge.rs`, related parallel tests

# Design: parallel merge 最終検証の fast-forward 許容

## Context
parallel merge は archive 完了後に `verify_merge_commits()` を呼び、base revision 以降に `Merge change: <change_id>` という merge commit subject が存在するかを確認しています。この検証は merge commit 前提なので、fast-forward merge では Git 的には成功していても失敗になります。

## Goals
- parallel merge の最終検証で fast-forward 統合済み change を成功扱いする
- merge commit message 不在のエラーを、本当に未統合なケースに限定する
- 既存の merge commit ベース検証を壊さず、fast-forward だけ追加で許容する

## Non-Goals
- resolve 経路の retry 判定の統合
- merge 戦略を常に fast-forward または常に no-ff に固定すること

## Decisions

### 1. 最終検証は merge commit message と統合済み判定の OR にする
`verify_merge_commits()` は各 change_id について以下のどちらかを満たせば成功とします。
- `Merge change: <change_id>` の merge commit message がある
- 対応 branch / revision の内容がすでに HEAD に取り込まれている

### 2. 未統合ケースだけエラーにする
`Missing merge commit message containing change_id(s)` は、merge commit message もなく、統合済み判定も満たさない change にだけ使います。

### 3. helper とテストを shared に寄せる
resolve 側ですでに使っている fast-forward 判定に近い考え方を、parallel merge 側でも再利用できるよう helper かテスト前提を整えます。

## Verification Strategy
- fast-forward merge 後に `verify_merge_commits()` が成功するテスト
- 未統合 change では従来どおり error になるテスト
- merge commit がある通常ケースが壊れていないことのテスト

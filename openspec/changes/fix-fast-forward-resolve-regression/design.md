# Design: fast-forward resolve 成功判定の整合化

## Context
parallel resolve は AI に `git merge` 実行を委ねたあと、後段で merge 完了を再検証します。現状の再検証は merge commit の存在を強く仮定しており、fast-forward merge では Git 的に成功していても `Missing merge commits for change_ids` として再試行に入ります。加えて TUI refresh は archived worktree を `merge wait` 候補として扱うため、統合済み change が未解決表示へ退行します。

## Goals
- fast-forward merge を resolve 成功として扱う
- merged 済み change を refresh/reducer が `merge wait` に戻さない
- merge commit が必要な未完了ケースだけを継続理由として残す

## Non-Goals
- merge strategy 全体の再設計
- conflict marker 解消ロジックの変更

## Decisions

### 1. resolve 成功判定は「merge commit 作成」ではなく「base への統合完了」を優先する
resolve 後検証は、merge commit の有無だけでなく以下のいずれかを成功条件に含めます。
- 対象 change が base に統合済みである
- 対象 revision が base に対して ahead 0 で、追加の merge 未完了シグナルがない

これにより fast-forward merge を成功として受理できます。

### 2. `Missing merge commits` は merge commit 必須ケースに限定する
継続理由 `Missing merge commits for change_ids` は、pre-sync merge commit など merge commit を要求するケースだけに使います。fast-forward で統合済みのケースには適用しません。

### 3. refresh は merged 済み change を merge-wait 復元しない
`WorkspaceState::Archived` の観測だけで `merge wait` を復元せず、merged 済み判定または not-ahead 判定と矛盾する場合は terminal merged を優先します。shared reducer の terminal state を workspace observation が上書きしないことを回帰テストで固定します。

## Verification Strategy
- resolve 後に fast-forward merge が発生した場合の成功テスト
- fast-forward 成功後に `Missing merge commits` へ入らないテスト
- merged 行が `ChangesRefreshed` / observation 後も merged のまま維持されるテスト

# Change: Update TUI worktree merge eligibility for unknown ahead status

## Why
現在のTUI Worktree Viewでは、5秒ごとのマージ状態更新時にコミット先行検出が失敗すると、実際にはマージ可能でも「進んでいない」や「merged」と誤判定され、Mキーのマージがブロックされる場合があります。検出失敗時の扱いを明確化し、誤ってマージを阻害しない挙動に更新します。

## What Changes
- コミット先行検出の結果に「不明(unknown)」状態を導入する
- 検出が不明な場合は警告付きでマージを許可する
- キーヒントの表示条件とマージ検証の挙動を更新する
- 検出が不明な場合に誤って「merged」表示にならないよう状態ラベルを修正する

## Impact
- Affected specs: tui-worktree-merge
- Affected code: TUI worktree view/merge validation logic, key hint rendering, worktree info loading

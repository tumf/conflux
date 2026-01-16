# Worktree マージ条件の修正

## Why

TUI Worktree View のマージ機能に以下のUX問題とバグがあり、ユーザーが混乱している：

1. マージが不要な worktree（baseブランチと同じコミット位置）でもMキーが表示されるため、押しても何も起きない
2. Mキーを押しても失敗する場合、エラーメッセージが表示されないため原因が分からない
3. UI表示条件とロジック実行条件が一致していない
4. **マージ実行先が間違っている** - worktree側でマージを実行しているが、本来はbase（main worktree）側で実行すべき
5. **TUIがクラッシュする可能性** - Mキー押下時にTUIが無言で終了する報告があり、デバッグログの追加と安定性向上が必要

この修正により、ユーザーは：
- 実際にマージ可能な場合のみMキーを表示し、無駄な操作を防ぐ
- 失敗時に明確なエラーメッセージを見て、何が問題かを理解できる
- worktreeのブランチを正しくbase側にマージできる
- クラッシュせずに安定したマージ操作を行える
- より信頼性の高いworktreeマージ機能を使用できる

## 問題

TUI Worktree View で以下の問題が発生しています：

1. **Mキーが常に表示される** - マージが不要な時（baseブランチより先に進んでいないworktree）でもMキーが表示される
2. **Mキーを押してもコマンドが実行されない** - 内部条件チェックで`None`が返されるがエラーメッセージが表示されない
3. **ユーザーフィードバックが不足** - なぜマージできないのかが分からない
4. **マージ実行先が間違っている** - 現在は`merge_branch(&worktree_path, ...)`でworktree側で実行しているが、本来は`merge_branch(&repo_root, ...)`でbase側で実行すべき。これにより「Working directory is not clean」エラーがworktree側のdirty状態で発生してしまう
5. **TUIがクラッシュする** - Mキー押下時にTUIが無言で終了することがある。デバッグログが不足しており原因特定が困難

## 提案

### 1. Mキー表示条件の厳密化

Mキーは以下の条件を**すべて**満たす場合のみ表示：

- main worktreeではない
- detached HEADではない
- マージコンフリクトがない
- ブランチ名がある
- **baseブランチより先にコミットがある** (NEW)

### 2. エラーメッセージの追加

`request_merge_worktree_branch()` で条件チェックに失敗した場合、適切な警告メッセージを表示：

- view_modeが異なる場合: "Switch to Worktrees view to merge"
- worktreesが空の場合: "No worktrees loaded"
- カーソルが範囲外の場合: "Cursor out of range: {cursor} >= {len}"
- 既存のメッセージ（main/detached/conflict/no branch）はそのまま

### 3. WorktreeInfo拡張

`has_commits_ahead: bool` フィールドを追加し、worktreeロード時に差分をチェック。

### 4. マージ実行先の修正

現在の実装（間違い）:
```rust
// runner.rs:1110
merge_branch(&worktree_path, &merge_branch)  // worktree側でマージ実行
```

修正後:
```rust
merge_branch(&merge_repo_root, &merge_branch)  // base（main worktree）側でマージ実行
```

これにより:
- working directory cleanチェックがbase側で行われる
- マージコミットがbase側に作成される
- worktree側のuncommitted changesは影響しない

### 5. デバッグログとエラーハンドリングの強化

クラッシュ問題の調査と安定性向上のため：

- `request_merge_worktree_branch()` にデバッグログを追加
- Mキーハンドリング部分にデバッグログを追加
- コマンド送信/受信時のエラーハンドリングを強化
- 予期しないエラー時でもTUIを終了させずに警告表示

## 影響範囲

- `src/tui/types.rs` - WorktreeInfo構造体
- `src/tui/runner.rs` - worktreeロード処理、**マージ実行先の修正**、デバッグログ追加
- `src/tui/render.rs` - Mキー表示条件
- `src/tui/state/mod.rs` - マージリクエスト処理、デバッグログ追加
- `src/vcs/git/commands.rs` - 差分チェック関数追加（必要に応じて）

## 期待される結果

- Mキーは実際にマージ可能な場合のみ表示される
- Mキーを押して失敗した場合、明確なエラーメッセージが表示される
- ユーザーはなぜマージできないのかを理解できる
- **worktreeのブランチがbase（main worktree）側に正しくマージされる**
- **worktree側のuncommitted changesがマージをブロックしない**
- **TUIがクラッシュせず、問題発生時もデバッグログで原因を特定できる**

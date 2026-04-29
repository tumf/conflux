## Implementation Tasks

- [ ] 1. `openspec/specs/parallel-execution/spec.md` に、dependency blocked change が resolved になった後の初回 dispatch では既存 worktree を再利用せず fresh worktree を作成する canonical rule を追加/修正する (verification: integration - `cflx openspec validate fix-dependency-resolved-worktree-recreation --strict --evidence warn` が成功し、dependency-resolved recreation 例外が spec に明記される)
- [ ] 2. `src/parallel/queue_state.rs` と `src/parallel/mod.rs` に、dependency blocked → resolved になった change を「forced recreation 必須」として記録する runtime state を実装する (verification: unit - queue state tests が `DependencyResolved` 後に対象 change だけ forced recreation 対象になることを確認する)
- [ ] 3. `src/parallel/workspace.rs` と `src/parallel/dispatch.rs` を更新し、forced recreation 対象の change では `find_existing_workspace()` / `reuse_workspace()` を通さず新規 workspace 作成へ進むようにする (verification: integration - `src/parallel/tests/workspace_resume.rs` または同等の dispatch/resume test で dependency-resolved change は `WorkspaceCreated` を受け、通常 resume change は `WorkspaceResumed` を維持することを確認する)
- [ ] 4. `src/vcs/git/mod.rs` または関連 workspace cleanup 経路を更新し、dependency-resolved fresh dispatch 前に stale worktree / branch を cleanup または equivalent invalidation できるようにする (verification: integration - `src/vcs/git/mod.rs` の worktree tests または同等の git backend test で stale worktree 残存時でも `create_workspace()` が fresh recreate に成功し、`find_existing_workspace()` が古い worktree を再利用 source として返さないことを確認する)
- [ ] 5. `src/events.rs`、parallel event bridge、TUI/Web state mapping を更新し、dependency-resolved fresh recreation と通常 resume を log / event / status reason で区別できるようにする (verification: unit/integration - `src/orchestration/state.rs`, `src/tui/state.rs`, `src/web/state.rs` の state mapping tests が `DependencyResolved` 後の reason/display を generic resume と混同しないことを確認する)
- [ ] 6. `classify-acceptance-followup-routing` のような dependency-coupled change を想定した regression test を追加し、依存 change 完了後に stale base worktree ではなく最新 base 前提で downstream change が再開されることを確認する (verification: integration - `src/parallel/tests/workspace_resume.rs` または scheduler integration test で dependency unblock 後の recreate を通し、`classify-acceptance-followup-routing` 相当の downstream change が stale worktree reuse ではなく fresh base dispatch へ進むことを確認する)
- [ ] 7. proposal delta と関連実装変更をまとめて検証する (verification: integration - `cflx openspec validate fix-dependency-resolved-worktree-recreation --strict --evidence warn`, `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`)

## Future Work

- dependency-unblocked stale worktree を operator が明示確認できる dashboard / TUI 表示改善
- worktree recreate の代わりに safe salvage/rebase を許容するかどうかの高度な policy 設計

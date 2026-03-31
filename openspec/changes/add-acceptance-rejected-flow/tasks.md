## Implementation Tasks

- [x] 1. `src/orchestration/state.rs`: `TerminalState` に `Rejected(String)` バリアントを追加する (verification: `cargo test` で既存テストが通り、新バリアントがコンパイルされること)
- [x] 2. `src/orchestration/state.rs`: `display_status()` で `TerminalState::Rejected(_)` → `"rejected"` を返す (verification: ユニットテスト追加)
- [x] 3. `src/orchestration/state.rs`: `display_color()` で `"rejected"` に専用色(例: `Color::LightRed`)を割り当てる (verification: ユニットテスト追加)
- [x] 4. `src/orchestration/state.rs`: `apply_command(AddToQueue)` で `TerminalState::Rejected` を `Archived | Merged` と同列に `NoOp` 扱いにする (verification: ユニットテスト追加)
- [ ] 5. `src/orchestration/rejection.rs` (新規): rejection フロー関数を実装する — REJECTED.md 生成、base チェックアウト、コミット、`openspec resolve` 呼び出し、worktree 削除 (verification: ユニットテスト)
- [x] 6. `src/orchestration/mod.rs`: `rejection` モジュールを追加し re-export する (verification: `cargo build`)
- [x] 7. `src/serial_run_service.rs`: `AcceptanceResult::Blocked` 分岐を rejection フロー呼び出しに変更し、`ChangeProcessResult::Rejected { reason }` を返す (verification: 既存テスト更新 + 新規テスト)
- [x] 8. `src/parallel/dispatch.rs`: `AcceptanceResult::Blocked` 分岐を rejection フロー呼び出しに変更し、`WorkspaceResult` で `error: None, rejected: Some(reason)` を返す (verification: コンパイル + テスト)
- [x] 9. `src/parallel/types.rs`: `WorkspaceResult` に `rejected: Option<String>` フィールドを追加する (verification: コンパイル + 既存テスト)
- [x] 10. `src/openspec.rs`: `list_changes_native()` で `REJECTED.md` が存在する change ディレクトリをスキップする (verification: ユニットテスト追加)
- [x] 11. `src/web/state.rs`: `ChangeStatus.queue_status` のドキュメントコメントに `"rejected"` を追加する (verification: コメント確認)
- [x] 12. `dashboard/src/api/types.ts`: `ChangeStatus` union に `'rejected'` を追加する (verification: TypeScript コンパイル)
- [x] 13. `dashboard/src/components/ChangeRow.tsx` (または該当コンポーネント): `rejected` ステータスのバッジ色・ラベルを追加する (verification: ビルド確認 `cd dashboard && npm run build`)
- [x] 14. `src/orchestration/state.rs`: rejected 関連の reducer イベント処理を追加する — `ChangeRejected` イベントで `TerminalState::Rejected` に遷移 (verification: ユニットテスト)
- [ ] 15. 統合テスト: Blocked → Rejected フロー全体の e2e テストを追加する (verification: `cargo test --test e2e_tests`)
- [x] 16. `cargo fmt --check && cargo clippy -- -D warnings` で lint/format 確認 (verification: CI 通過)

## Future Work

- rejected change の proposal を修正して再提出するワークフロー（手動操作が必要）
- REJECTED.md の内容をもとに proposal の自動修正提案を行う機能

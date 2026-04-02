## Implementation Tasks

- [x] 1. apply blocker を rejection 提案として検出する runtime 契約を spec delta に追加する (verification: `openspec/changes/update-apply-blocked-rejection-handoff/specs/` に apply blocked → acceptance handoff の requirement/scenario がある)
- [x] 2. apply 実行結果に `blocked` 相当の出口を追加し、`REJECTED.md` 提案ファイルの存在を正式状態として扱えるようにする (verification: `src/parallel/dispatch.rs` または共通実行ループで tasks 未完了でも apply blocked を acceptance へ渡す分岐がある)
- [x] 3. apply blocker 時の `REJECTED.md` 生成規約と理由抽出ロジックを定義し、acceptance/rejection flow で再利用できるようにする (verification: `src/orchestration/rejection.rs` と関連コードで apply-generated REJECTED.md を扱う)
- [x] 4. acceptance が apply 由来の rejection proposal を承認/差し戻しできるようにし、承認時のみ既存 rejection flow を完了させる (verification: `AcceptanceResult::Blocked` まで接続されるテストまたはイベント検証が追加される)
- [x] 5. apply blocker による empty WIP stall loop を防ぐ回帰テストを追加する (verification: blocker 記録済み change が stall ではなく blocked/rejected 経路へ進むテストがある)
- [x] 6. `skills/cflx-workflow/references/cflx-apply.md` と `skills/cflx-workflow/SKILL.md` を更新し、正当な implementation blocker 検出時は `tasks.md` 記録に加えて `openspec/changes/<change_id>/REJECTED.md` を rejection proposal として生成し、blocked handoff 用の machine-readable marker を出力するよう apply 指示を明記する (verification: repo 内 skill source が apply blocker → REJECTED.md proposal → acceptance handoff を明示し、`cflx install-skills --global` で `~/.agents/skills/cflx-workflow/` へ配布される内容と整合する)
- [x] 7. `skills/cflx-workflow/references/cflx-accept.md` を更新し、apply-generated `REJECTED.md` を acceptance-confirmed rejection 前の proposal artifact として扱うよう acceptance 指示を整合させる (verification: accept prompt source が apply-generated `REJECTED.md` を即時終端ではなく承認対象として説明している)

## Future Work

- `tasks.md` 自体に blocked/cancelled などの多値タスク状態を導入する設計検討
- apply/acceptance 間で構造化 blocker payload を受け渡す専用メタデータファイルの検討

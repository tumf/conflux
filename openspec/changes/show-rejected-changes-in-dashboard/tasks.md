## Implementation Tasks

- [x] 1. dashboard 用 change 列挙を拡張する: `src/server/api/ws.rs` の change scan で `proposal.md` を持つ active change に加え、`REJECTED.md` を持つ change directory も snapshot 対象に含める (verification: ws/api unit test で rejected marker を持つ change が payload に含まれることを確認)
- [x] 2. status 導出の優先順位を固定する: `src/server/api/ws.rs` の status derivation で `REJECTED.md` が存在する場合は reducer fallback より優先して `rejected` を返すようにする (verification: reducer state が空でも rejected status になるテストを追加)
- [x] 3. dashboard UI の rejected row 契約を固定する: `dashboard/src/components/ChangeRow.tsx` と関連 test で rejected row に active 操作が出ないことを確認し、必要なら回帰テストを追加する (verification: component test が pass する)
- [x] 4. 実行対象除外契約を維持する: `src/openspec.rs` の native active listing が引き続き `REJECTED.md` marker-bearing change を除外することを regression test で確認する (verification: existing/new unit test が pass する)
- [x] 5. spec delta を追加して validate する: dashboard change listing と rejected exclusion semantics の分離を spec に追記し、`python3 /Users/tumf/.agents/skills/cflx-proposal/scripts/cflx.py validate show-rejected-changes-in-dashboard --strict` を pass させる (verification: strict validate が pass する)

## Future Work

- rejected reason を WebUI detail panel や tooltip に表示する改善
- rejected / archived / merged を切り替える filter UI の追加

## Acceptance #1 Failure Follow-up

- [x] `src/server/api/control.rs` の実行対象列挙で `rejected` status を除外し、dashboard 可視化と run 対象の契約を分離する
- [x] rejected change が `/api/v1/control/run` 経由で実行対象に含まれないことを確認する回帰テストを追加する
- [x] pre-commit を通常コミット相当で実行できるように開発環境/CI の hook 実行経路を整備する

## Acceptance #2 Failure Follow-up

- [x] `tasks.md` の pre-commit 完了条件を実態に合わせて修正し、`pre-commit` バイナリ未導入環境でも検証不能である事実を明示する（`pre-commit run --all-files` は `pre-commit: not found` を確認）

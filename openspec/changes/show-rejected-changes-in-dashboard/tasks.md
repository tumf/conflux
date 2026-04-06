## Implementation Tasks

- [ ] 1. dashboard 用 change 列挙を拡張する: `src/server/api/ws.rs` の change scan で `proposal.md` を持つ active change に加え、`REJECTED.md` を持つ change directory も snapshot 対象に含める (verification: ws/api unit test で rejected marker を持つ change が payload に含まれることを確認)
- [ ] 2. status 導出の優先順位を固定する: `src/server/api/ws.rs` の status derivation で `REJECTED.md` が存在する場合は reducer fallback より優先して `rejected` を返すようにする (verification: reducer state が空でも rejected status になるテストを追加)
- [ ] 3. dashboard UI の rejected row 契約を固定する: `dashboard/src/components/ChangeRow.tsx` と関連 test で rejected row に active 操作が出ないことを確認し、必要なら回帰テストを追加する (verification: component test が pass する)
- [ ] 4. 実行対象除外契約を維持する: `src/openspec.rs` の native active listing が引き続き `REJECTED.md` marker-bearing change を除外することを regression test で確認する (verification: existing/new unit test が pass する)
- [ ] 5. spec delta を追加して validate する: dashboard change listing と rejected exclusion semantics の分離を spec に追記し、`python3 /Users/tumf/.agents/skills/cflx-proposal/scripts/cflx.py validate show-rejected-changes-in-dashboard --strict` を pass させる (verification: strict validate が pass する)

## Future Work

- rejected reason を WebUI detail panel や tooltip に表示する改善
- rejected / archived / merged を切り替える filter UI の追加

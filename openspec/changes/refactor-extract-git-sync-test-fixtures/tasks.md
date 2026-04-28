## Implementation Tasks

- [ ] 1. `git/sync` の主要シナリオ（同期済み、SHA 差異あり、remote ahead など）の公開レスポンスを固定する characterization test を先に追加または更新する (verification: integration - add or update Rust endpoint tests asserting current HTTP status and JSON fields such as `status`, `resolve_command_ran`, `resolve_exit_code`, `push.status`, and `skipped_reason` before refactor)
- [ ] 2. registry / AppState / router 初期化を共通 fixture helper に抽出し、各テストが期待挙動の assertion に集中できる構造へ整理する (verification: unit/integration - inspect test helpers and run git_sync endpoint tests to confirm setup reuse without behavior changes)
- [ ] 3. local bare repo / scratch clone / divergence 生成の重複手順を helper 化し、ケースごとの repo 変異を意図中心に記述できるようにする (verification: integration - run divergence scenarios and confirm they still reproduce the same sync branches as before)
- [ ] 4. helper 抽出後も `git_sync` のログ意味論とレスポンス契約に回帰がないことを確認する (verification: integration - run existing and updated git_sync tests and confirm success / skip / resolve-run semantics stay unchanged)
- [ ] 5. proposal delta と関連テスト整理を strict validation と Rust テストで確認する (verification: integration - run `cflx openspec validate refactor-extract-git-sync-test-fixtures --strict --evidence warn` and `cargo test`)

## Future Work

- `server/api` 他モジュールの大型統合テストにも共通 fixture パターンを横展開する
- API テストの JSON assertion helper を導入して可読性をさらに上げる

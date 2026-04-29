## Implementation Tasks

- [ ] 1. `openspec/specs/parallel-execution/spec.md` と必要なら `openspec/specs/cli/spec.md` に delta を追加し、acceptance fail を primary outcome、follow-up persistence failure を secondary degradation として扱う契約を明記する (verification: integration - `cflx openspec validate degrade-acceptance-followup-persistence --strict --evidence warn` が成功し、spec が primary/secondary failure separation を明示する)
- [ ] 2. acceptance fail 後に使う canonical tasks-path resolver を追加し、active tasks path が無い場合でも archive tasks location を探索できるようにする (verification: unit - resolver tests が active path, archived path, neither path の3ケースで期待どおりの探索結果を返すことを確認する)
- [ ] 3. `src/parallel/dispatch.rs` の acceptance fail path を更新し、follow-up persistence が失敗しても acceptance `FAIL` を terminal `Error` に増幅せず、warning/supplemental context として残すようにする (verification: integration - parallel acceptance tests が active tasks.md 不在ケースで primary result を `FAIL` のまま保持することを確認する)
- [ ] 4. `src/serial_run_service.rs` の acceptance fail path も同様に更新し、serial path でも tasks persistence failure が acceptance verdict を上書きしないことを固定する (verification: unit - serial acceptance tests が tasks path missing case で acceptance failed result と degraded persistence context を確認する)
- [ ] 5. `src/task_parser.rs` と関連 comments / logs を整理し、accept agent と runtime のどちらが follow-up を更新する canonical owner かを明示する (verification: manual - code comment / log review で責務説明の矛盾が解消されていることを確認する)
- [ ] 6. active path missing・archive fallback・no tasks path at all を再現する regression tests を追加し、acceptance fail が metadata persistence failure だけで terminal error にならないことを確認する (verification: integration - targeted Rust tests が3ケースすべてで pass する)
- [ ] 7. proposal delta と実装変更をまとめて検証する (verification: integration - `cflx openspec validate degrade-acceptance-followup-persistence --strict --evidence warn`, `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`)

## Future Work

- acceptance follow-up persistence を runtime から完全に剥がして accept agent 側へ一本化するかの再設計
- archived rejecting recovery と acceptance fail persistence resolver の共通化

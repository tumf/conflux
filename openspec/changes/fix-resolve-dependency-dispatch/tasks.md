## Implementation Tasks

- [x] 1. Resolve lifecycle evidenceをdependency contextへ統合し、active resolve、resolve-wait、archived-but-unmerged targetを未解決として分類する（completion: `src/parallel/dependency.rs`の単一contextがqueue gatingとdispatch selectionの両方へ同じevidence snapshotを提供する。verification: unit - dependency target classごとのtable-driven testがresolving targetをdispatch-satisfiedにしない）
- [x] 2. Dispatch直前のdependency gateをfail-closedにし、proposal metadata上のdependencyごとにeffective-base merge evidenceを確認する（completion: `src/parallel/queue_state.rs`でmerge evidenceがtrueのdependencyだけが解決済みとなり、取得失敗、不整合、resolve継続中はselected changesへ入らない。verification: unit - analyzerがdependentをorderへ含めても未統合dependencyでは選択されない）
- [x] 3. Resolve完了時のstate transitionと再分析triggerを接続し、merge完了前の通知ではdependentを解放せず、repository-visible integration後にのみ再評価する（completion: resolve completion pathが既存`ReanalysisReason::ResolveCompletion`を発火し、次のselectionが更新済みbase evidenceを読む。verification: integration - scheduler event sequenceでresolve完了後にのみdependentの`ApplyStarted`を観測する）
- [x] 4. Resolve中の独立changeに対する既存並列dispatchを保持する（completion: resolveがslotを使用中でも、残りcapacityと依存条件を満たすunrelated changeがdispatchされる。verification: integration - `src/parallel/tests/executor.rs`でunrelated `ApplyStarted`とdependent未開始を同じrunで検証する）
- [x] 5. 実際の依存proposal fixtureで回帰testを追加する（completion: `persist`相当のdependencyがresolve中、`bound`相当のdependentがqueuedというfixtureで、merge前抑止、merge後dispatch、独立change並列実行を検証する。verification: integration - `cargo test parallel::tests::executor`）
- [x] 6. Dependency blocker diagnosticが既存deduplication storeを通ることを維持する（completion: resolve継続中の同一blocker再評価でoperator-visible diagnosticが1回だけ出力され、blocker signature変化後は再出力される。verification: integration - event receiverでblocked diagnostic件数を検証する）
- [x] 7. 対象品質gateを実行し、変更した挙動と既存parallel dispatchの回帰がないことを確認する。completion: format、check、clippy、対象testの全コマンドが成功する。(verification: integration - `cargo fmt --check && cargo check --locked --all-targets --all-features && cargo clippy --locked --all-targets --all-features -- -D warnings && cargo test parallel::tests::executor && cargo test parallel::tests::manual_resolve`; passed 2026-07-21)

## Future Work

なし。

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-resolve-dependency-dispatch --archive-gate`

## Acceptance #3 Failure Follow-up
- [x] `src/parallel/dependency.rs:57-78`はshared orchestrator stateの`try_read()`失敗を空集合へ変換するため、resolve lifecycle evidence取得不能時にdependent dispatchがfail-openになり得る。lock取得失敗を判定不能として保持し、dependentをblockする必要がある。
- [x] worktreeがdirtyでarchive commit readinessを満たさない。`openspec/changes/fix-resolve-dependency-dispatch/tasks.md:22`に未コミット変更と未完了checkboxがある。なお実際のcommit-path hook相当である`prek run --all-files`と`cflx openspec validate fix-resolve-dependency-dispatch --archive-gate`は成功した。
- [x] 宣言済みintegration verificationが失敗する。`cargo test parallel::tests::executor::resolving_dependency_blocks_its_dependent_but_not_unrelated_dispatch --lib`は`src/parallel/tests/executor.rs:558`で失敗し、merge evidence後のdependent `ApplyStarted`を観測できない。resolve完了後の再分析・dispatch経路を修正する必要がある。

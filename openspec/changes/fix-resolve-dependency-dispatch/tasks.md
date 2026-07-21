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

## Acceptance #2 Failure Follow-up
- [x] Task 1の宣言済みunit verificationが不足している。`openspec/changes/fix-resolve-dependency-dispatch/tasks.md:3`はdependency target classごとのtable-driven testを要求するが、`src/parallel/dependency.rs:303-370`はtable-drivenではなく、`resolving-a`の同一assertionを2回繰り返しておりresolve-wait分類を検証していない。resolve-waitを含む各target classのunit evidenceを追加する必要がある。対象品質gate、strict validation、archive-gate validationは成功し、tasks.mdに未チェック項目はなく、worktreeはcleanで実行可能commit hookも確認されなかった。
- [x] resolve lifecycle evidenceとeffective-base取得の失敗がfail-openになり得る。`src/parallel/dependency.rs:57-78`はshared stateの`try_read()`失敗を空のresolving/resolve-wait集合へ変換し、`src/parallel/dependency.rs:187-202`はcurrent integration branch取得失敗時にoriginal branchへフォールバックする。取得失敗時は判定不能としてdependentをblockする必要がある。
- [x] 依存metadata取得失敗がなおfail-openになる。`src/parallel/queue_state.rs:609-639`と`src/parallel/queue_state.rs:2287-2310`は`proposal.md`が存在しない場合を取得失敗としてblockせず、空のdependency listへ変換する。仕様の「dependency evidence failure is fail-closed」に従い、dispatch候補に対するproposal欠落もblockし、診断を出す必要がある。

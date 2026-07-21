## Implementation Tasks

- [x] 1. Normalized repository findingへcode優先・構造fallbackのstable identity生成を実装し、説明文/evidence変更をidentity入力から除外する。(verification: unit - `cargo test task_parser`でcodeあり/なし、detail変更、location/rule差異、collision fixtureを検証; completion condition:agent codeなしでも同一欠陥が同じidentity、異なる欠陥が異なるidentityになる)
- [x] 2. Runtime follow-up reconciliationをidentity単位のmergeへ統一し、apply hydration中の`[x]`を単調に保持する。(verification: unit - `cargo test task_parser`で部分完了＋本文変更、開始時/実行中/終了後hydrate、evidence保持を検証; completion condition:最新FAIL以外のruntime更新で`[x]`が`[ ]`へ遷移しない)
- [x] 3. 最新acceptance FAILを明示的なreopen境界として接続し、同一identity再報告だけをuncheckedへ戻す。(verification: integration - `cargo test parallel::dispatch && cargo test serial_run_service`で同一identity reopen、非再報告finding retirement、異なるfinding非reopen、serial/parallel parityを検証; completion condition:reopenの呼出し元が新規FAIL処理に限定される)
- [x] 4. `cflx-apply` guidanceでruntime-owned acceptance findingを通常taskのrefine規則から除外し、本文不変・修正/verification evidence別記・修正ごとの即時`[x]`化を定義する。(verification: unit - `cargo test embedded_skills`で競合するrefine指示がなく、runtime-owned例外とevidence規則が含まれることを検証; completion condition:bundled apply guidanceがfinding本文変更を要求または許容しない)
- [x] 5. `cflx-accept` guidanceへstable finding code、1 finding=1欠陥、実装欠陥/不足テスト分離、横断重複finding禁止、current-worktree再検証とfixed/still-open判定を追加する。(verification: unit - `cargo test embedded_skills`で全guidance要素とread-only contractを検証; completion condition:acceptance guidanceがstale previous findingを根拠だけで再報告せず、runtime task編集も要求しない)
- [x] 6. Regression suiteとquality gatesを実行し、部分完了reopenデグレとskill-only enforcementへの退行を防ぐ。(verification: integration - `cargo test task_parser && cargo test parallel::dispatch && cargo test serial_run_service && cargo test embedded_skills && cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings`; completion condition:全commandが成功し、runtime fallbackを外すと少なくとも1 regression testが失敗する)

## Future Work

- Structured-finding-only protocolへの移行は全agent runtimeがstable code出力を保証した後に検討する。
- Checkpointをauthoritative source、`tasks.md`をrendered viewにする設計はdurable stateのlossless化後に別changeで検討する。

## Final Validation

Expected archive gate: `cflx openspec validate harden-acceptance-finding-reconciliation --archive-gate` exits 0.

## Implementation Tasks

- [x] 1. canonical verdict parsing を厳格化する: `src/acceptance.rs` で standalone line 完全一致を canonical 判定として扱い、`ACCEPTANCE: PASSAll...` / `ACCEPTANCE: PASS## ...` のような trailing text verdict を PASS として受理しない (verification: parsing unit tests で malformed verdict が PASS にならないことを確認)
- [x] 2. acceptance streaming 中の verdict 確定経路を追加する: `src/parallel/executor.rs` と必要な runner 層で standalone verdict を受信した時点で acceptance result を確定し、process exit 待ちに依存しない handoff を実装する (verification: executor test で verdict 出力後に command がぶら下がっても PASS handoff できることを確認)
- [x] 3. verdict 確定後の process cleanup を明示化する: acceptance 子プロセス／process group を verdict 確定後に終了・回収し、inactivity timeout retry が発生しないことを保証する (verification: regression test で verdict 後に 900s timeout retry に入らないことを確認)
- [x] 4. acceptance contract の責務境界を固定する: `.opencode/commands/cflx-accept.md` と必要な spec / regression test を更新し、「単独行 canonical verdict は command template 側の契約、runtime はそれを厳密に採用する」ことを明文化する (verification: spec/contract tests が pass)
- [x] 5. OpenSpec delta を追加する: `parallel-execution` に verdict 検出時点で acceptance operation を完了できることを追記し、`agent-prompts` に trailing text 連結 verdict が canonical としては無効であることを追記する (verification: `cflx openspec validate complete-acceptance-on-standalone-verdict --strict`)

## Future Work

- archive / cleanup-review / rejecting review でも同様の early-final-marker completion を共通化するかの検討
- AI command runner 全体で machine-readable final marker を generic primitive に昇格するリファクタ

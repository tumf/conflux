## Implementation Tasks

- [x] 1. update archive readiness 契約の現行ズレ（`--strict` / evidence policy / commit-path blocker / archive未実施時の扱い）を spec と skill guidance で棚卸しし、canonical requirement に反映する (verification: manual - `openspec/specs/agent-prompts/spec.md`, `openspec/specs/parallel-execution/spec.md`, `skills/cflx-archive/SKILL.md` を見比べ、archive readiness と archive failure root cause の両方を明示していることをレビューする)
- [x] 2. implement parallel archive command 実行で「archive CLI は未実施だが agent は exit 0 で自然文の blocker を返した」runtime failure ケースを、verification failure 一般論へ潰さず root-cause 付き archive failure として記録・表示する (verification: unit - `cargo test parallel::tests::executor -- --nocapture` または同等の追加テストで stdout/stderr tail 由来の blocker が最終 error に含まれることを確認する)
- [x] 3. implement serial / streaming archive command 実行でも、runtime failure の最終エラーに直前 attempt の validation failure または commit-path blocker 要約を保持する (verification: unit - `cargo test orchestration::archive::tests -- --nocapture` または同等の追加テストで final error が `not actually archived` だけで終わらないことを確認する)
- [x] 4. update archive prompt / skill guidance を更新し、archive agent が `cflx openspec archive <id> --yes` の前提条件 failure を検出した場合は archive 未実施の blocker として明示し、downstream が root cause を解釈できる出力方針を持つようにする (verification: manual - `skills/cflx-archive/SKILL.md`, `src/agent/prompt.rs`, `src/agent/runner.rs` の archive-related guidance/paths を確認し、archive-readiness blocker の扱いが追加されていることをレビューする)
- [x] 5. verify proposal/spec/skill/実装を通しで、archive readiness と failure-reporting 契約が同じ change failure 例で一貫することを確認する (verification: integration - `cflx openspec validate align-archive-readiness-failure-reporting --strict --evidence warn`、`cargo test`、`cargo clippy --all-targets --all-features -- -D warnings`)

## Future Work

- `apply` / `acceptance` / `resolve` でも同様の「exit code success だが machine-readable failure を返す」契約不整合が見つかった場合は、別 change で共通 protocol 化する

## Acceptance #1 Failure Follow-up
- [x] Real archive commit path is still blocked by the repository pre-commit hook: `/Users/tumf/work/conflux/.git/hooks/pre-commit:1-14` delegates normal commits to `prek`, and `agent-exec run -- prek run --all-files` was re-run in this workspace (`job_id: 1e68218acc9d2b46578854fd6683b565`) to validate hook behavior; `git status --short` still reports `M dashboard/src/components/ChangesPanel.test.tsx` and `M openspec/changes/align-archive-readiness-failure-reporting/tasks.md`, confirming the real commit path remains blocked/dirty until hook-induced changes are staged and reflected. Relevant context: `.pre-commit-config.yaml:1-27`.

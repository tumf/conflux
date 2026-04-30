## Implementation Tasks

- [ ] 1. `openspec/specs/parallel-execution/spec.md` と必要なら `openspec/specs/cli/spec.md` / `openspec/specs/orchestration-state/spec.md` に、self-modifying control-plane change の risk class と cross-phase verification expectations を追加する (verification: integration - `cflx openspec validate harden-self-modifying-phase-boundaries --strict --evidence warn` が成功し、spec delta が self-change risk と phase-boundary contract を明示する)
- [ ] 2. acceptance fail / persistence degradation / archive prerequisite failure / archive no-op stall の primary-secondary taxonomy を runtime contract に落とし込み、secondary degradation が primary diagnosis を上書きしないようにする (verification: unit - logs/events classification tests が各 failure class を distinct primary reason と supplemental context に分ける)
- [ ] 3. self-modifying change 向け archive preflight を追加し、spec promotion feasibility・heading alignment・no-op canonical diff を archive phase 前に検出できるようにする (verification: integration - self-change fixture が archive 直前 preflight で heading mismatch / no-op promotion を検知し、archive stall に進まないことを確認する)
- [ ] 4. blocker-only follow-up や persistence degradation のような non-progress condition を phase-specific outcome として扱い、generic empty-WIP stall への雑な集約を減らす (verification: integration - routing/archive tests が non-progress condition を dedicated outcome/warning として観測し、同条件で empty WIP stall へ直行しないことを確認する)
- [ ] 5. accept prompt / parser / routing / archive promotion を同時に変更する self-modifying scenario regression を追加し、primary diagnosis が安定し phase間で意味がドリフトしないことを固定する (verification: integration - scripted or Rust integration test が self-change scenario を end-to-end に再現し、期待した primary reason taxonomy を確認する)
- [ ] 6. observability wording と operator guidance を更新し、control-plane self-change failure の読み方が `primary diagnosis` と `secondary degradation` を区別できるようにする (verification: manual - log wording / docs review で same failure family の説明が phase間で矛盾しないことを確認する)
- [ ] 7. proposal delta と実装変更をまとめて検証する (verification: integration - `cflx openspec validate harden-self-modifying-phase-boundaries --strict --evidence warn`, `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`)

## Future Work

- self-modifying change 専用の dedicated canary lane / workflow mode
- archived logs から recurring failure family を自動集計する diagnostics dashboard

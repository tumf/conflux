## Implementation Tasks

- [x] 1. native validator の task parser を standalone verification continuation line 対応へ拡張する (verification: unit - add or update tests near `src/openspec_cmd.rs` proving an indented `verification:` line is attached to the preceding checkbox task and participates in evidence / ownership checks)

- [x] 2. standalone verification line が bare-task preview に誤分類された場合でも UTF-8 境界 panic を起こさないようにする (verification: unit - add or update tests near `src/openspec_cmd.rs` proving multi-byte standalone `verification:` text does not panic and still yields structured findings when invalid)

- [x] 3. inline `(verification: ...)` と standalone `verification:` の両形式で既存 warning / error semantics を回帰させない (verification: unit - keep or extend `src/openspec_cmd.rs` validation tests covering ownership, repository evidence, and executable-surface runnable verification for both forms)

- [x] 4. proposal validation capability spec と関連検証コマンドを更新して回帰保護する (verification: integration - run `cflx openspec validate fix-verification-line-validation-panic --strict --evidence warn`, `cargo test openspec_cmd`, and `cargo clippy --all-targets --all-features -- -D warnings`)

## Future Work

- `Completion condition:` 以外の continuation metadata syntax を canonical grammar として一般化するかの検討
- proposal authoring style を inline form と block form のどちらに寄せるかの repository-wide 整理

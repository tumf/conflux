## Implementation Tasks

- [ ] 1. native archive destination naming を dated 形式へ切り替える
  Completion condition: `src/openspec_cmd.rs` で archive destination が `openspec/changes/archive/YYYY-MM-DD-<change_id>` 形式で生成され、同名 destination 既存時は明示エラーになる。
  verification: unit - add or update tests near `src/openspec_cmd.rs` proving archive creates a dated destination and rejects an already-existing dated destination for the same day.

- [ ] 2. native archive の成功出力と archive 解決を dated 標準に揃える
  Completion condition: archive 成功メッセージが dated archive path を返し、`show` など detail-oriented 解決が direct/dated の両 archive entry を同一 change として扱う。
  verification: unit - add or update `src/openspec_cmd.rs` tests proving success output includes the dated path and archived change lookup resolves both direct and dated archive entries.

- [ ] 3. archive verification / TUI archived lookup の互換性を回帰させない
  Completion condition: archive 完了検証と archived editor lookup が direct/dated 両形式を継続して扱えることを確認する回帰テストが追加または維持される。
  verification: unit - keep or extend tests near `src/execution/archive.rs` and `src/tui/utils.rs` proving direct and dated archive entries are both treated as valid archived locations.

- [ ] 4. dated archive naming の canonical spec delta と実装検証を完了する
  Completion condition: proposal delta が strict validate を通過し、関連 Rust テストと lint/typecheck 相当コマンドが成功する。
  verification: integration - run `cflx openspec validate use-dated-archive-names --strict --evidence warn`, `cargo test`, and `cargo clippy --all-targets --all-features -- -D warnings`.

## Future Work

- 既存の非日付付き archive directory を dated 形式へ移行する一括 migration の要否判断
- archive 日付のタイムゾーンや clock source を設定可能にする拡張

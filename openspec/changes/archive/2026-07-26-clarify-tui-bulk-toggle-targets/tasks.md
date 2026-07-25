## Implementation Tasks

- [x] Bulk toggle開始時に各changeをeligibleまたは理由付きineligibleへ分類し、その同一snapshotから「未チェックがあれば全チェック、全チェック済みなら全解除」のtarget stateを決定する (verification: unit - `src/tui/state.rs`のtestsでpartial selectionとall-selected inversionを実行し、eligible全件の`selected`が一致することを`cargo test toggle_all_marks`で確認する)
- [x] 算出したtarget stateをeligible全件へ適用し、Running modeの`not queued`/`queued`には既存のAddToQueue/RemoveFromQueue commandを漏れなく生成し、active rowには停止commandを生成しない (verification: unit - `src/tui/state.rs`のmixed status testsで全対象のstateとcommand ID集合を検証し、`cargo test bulk_toggle`が成功する)
- [x] active、rejected、parallel-ineligibleなどの除外行を変更せず、変更件数・除外件数・対処可能な除外理由をbulk操作結果としてTUIへ反映する (verification: integration - `src/tui/key_handlers.rs`またはTUI state境界のtestでmixed eligible/ineligibleとzero-eligible操作後のwarning/log stateを検証する)
- [x] 既存のbulk toggle testsを、eligible/ineligible混在時にeligibleの一部だけが未変更で残らない回帰coverageへ拡張する (verification: unit - `src/tui/state.rs`でSelect、Stopped、Running、parallel mode、rejected rowのtable-drivenまたは同等のfocused casesを実行し、各caseがstub/no-op実装では失敗するassertionを持つ)
- [x] Rust品質ゲートを実行し、default testの1秒制約を維持する (verification: integration - repository rootの`Cargo.toml`に対して`cargo fmt --all -- --check`と`cargo clippy --all-targets --all-features -- -D warnings`を実行し、`src/tui/state.rs`のfocused testsが1秒を超える場合は最適化するか`heavy`指定を残す)

## Notes

- 分類ロジックは `src/tui/state/selection_logic.rs` の `BulkToggleExclusion` / `build_bulk_toggle_snapshot()` に集約し、`toggle_all_marks()` は1度取得したsnapshotのみを参照する。
- 除外理由の単一情報源として `guards::classify_toggle_block()` を追加し、単一行toggleのwarning文言とbulk除外分類が同じ判定順序を共有するようにした。
- `handle_key_event()` は legacy warning を key dispatch の**前**にクリアするよう変更した。従来は dispatch 後にクリアしていたため、`x` が設定した除外warningが同じkey pressで消えていた。
- 実行結果は log (`Toggled all: N marked change(s), M excluded (...)`) と `warning_message` の両方へ反映し、対象0件・非対応modeでも warn log + warning message を出す。

- 実行済み検証: `cargo test toggle_all_marks` (13 passed / 0.07s)、`cargo test bulk_toggle` (15 passed / 0.04s)、default suite `cargo test` (lib 2144 passed + integration すべて成功、0 failed)、`cargo fmt --all -- --check` clean、`cargo clippy --all-targets --all-features -- -D warnings` clean。新規テストはいずれも1秒未満のため `heavy` 指定は不要。

## Future Work

表示中のrowだけをbulk対象にするフィルター連動操作は、本変更の全proposal対象semanticsと独立しているため別提案で扱う。

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate clarify-tui-bulk-toggle-targets --archive-gate`

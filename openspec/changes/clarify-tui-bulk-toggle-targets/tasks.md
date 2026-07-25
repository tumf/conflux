## Implementation Tasks

- [ ] Bulk toggle開始時に各changeをeligibleまたは理由付きineligibleへ分類し、その同一snapshotから「未チェックがあれば全チェック、全チェック済みなら全解除」のtarget stateを決定する (verification: unit - `src/tui/state.rs`のtestsでpartial selectionとall-selected inversionを実行し、eligible全件の`selected`が一致することを`cargo test toggle_all_marks`で確認する)
- [ ] 算出したtarget stateをeligible全件へ適用し、Running modeの`not queued`/`queued`には既存のAddToQueue/RemoveFromQueue commandを漏れなく生成し、active rowには停止commandを生成しない (verification: unit - `src/tui/state.rs`のmixed status testsで全対象のstateとcommand ID集合を検証し、`cargo test bulk_toggle`が成功する)
- [ ] active、rejected、parallel-ineligibleなどの除外行を変更せず、変更件数・除外件数・対処可能な除外理由をbulk操作結果としてTUIへ反映する (verification: integration - `src/tui/key_handlers.rs`またはTUI state境界のtestでmixed eligible/ineligibleとzero-eligible操作後のwarning/log stateを検証する)
- [ ] 既存のbulk toggle testsを、eligible/ineligible混在時にeligibleの一部だけが未変更で残らない回帰coverageへ拡張する (verification: unit - `src/tui/state.rs`でSelect、Stopped、Running、parallel mode、rejected rowのtable-drivenまたは同等のfocused casesを実行し、各caseがstub/no-op実装では失敗するassertionを持つ)
- [ ] Rust品質ゲートを実行し、default testの1秒制約を維持する (verification: integration - repository rootの`Cargo.toml`に対して`cargo fmt --all -- --check`と`cargo clippy --all-targets --all-features -- -D warnings`を実行し、`src/tui/state.rs`のfocused testsが1秒を超える場合は最適化するか`heavy`指定を残す)

## Future Work

表示中のrowだけをbulk対象にするフィルター連動操作は、本変更の全proposal対象semanticsと独立しているため別提案で扱う。

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate clarify-tui-bulk-toggle-targets --archive-gate`

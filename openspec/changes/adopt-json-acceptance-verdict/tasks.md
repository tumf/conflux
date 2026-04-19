## Implementation Tasks

- [x] acceptance verdict contract を JSON primary / text fallback に再定義する (verification: unit - `src/acceptance.rs` の parser regression test)
- [x] acceptance runtime が `opencode run` の strict JSON 1 行出力を直接解釈できるようにする (verification: unit - `src/acceptance.rs` / integration - `src/parallel/executor.rs`)
- [x] `opencode run --format json` のイベント出力から本文 verdict JSON を抽出して同一判定へ正規化できるようにする (verification: integration - acceptance runner の streaming test)
- [x] plain text standalone marker を後方互換 fallback として残し、JSON 不在時のみ利用する (verification: unit - legacy marker compatibility test)
- [x] `.opencode/commands/cflx-accept.md` を、新しい JSON verdict contract を primary とする指示へ更新する (verification: manual - command template review)
- [x] `skills/cflx-accept/SKILL.md` を、新しい machine-readable verdict contract と fallback 方針へ更新する (verification: manual - skill contract review)
- [x] archive 関連 skill / guidance で acceptance contract 前提が残っている箇所を追随更新する (verification: manual - `skills/cflx-archive/SKILL.md` diff review)
- [x] malformed text verdict では CONTINUE だった実ケースを、JSON verdict では PASS で handoff できる回帰テストを追加する (verification: integration - acceptance→archive handoff regression)
- [x] `cargo test` の対象回帰を実行し、acceptance parser / runner / handoff が通ることを確認する (verification: integration - repo test command)

## Future Work

- apply / archive / resolve など他 operation への JSON verdict contract 展開
- opencode upstream 側で structured output enforcement を強化する場合の追随

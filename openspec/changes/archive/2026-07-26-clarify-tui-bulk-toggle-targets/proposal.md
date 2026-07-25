---
change_type: implementation
priority: medium
dependencies: []
references:
  - src/tui/state.rs
  - src/tui/state/selection_logic.rs
  - src/tui/key_handlers.rs
  - openspec/specs/tui-state-management/spec.md
verifications:
  - id: tui-bulk-toggle-tests
    requirement: Bulk toggle updates every eligible row consistently and reports rows excluded by safety guards
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: focused Rust state and key-handler tests covering partial selection, full inversion, and mixed eligible/ineligible rows
    rerun: cargo test toggle_all_marks && cargo test bulk_toggle
    prerequisites: []
---

# TUIの一括実行マーク対象と除外結果を明確化する

**Change Type**: implementation

## Problem/Context

Changes viewの`x`は、実行マーク可能なproposalに未チェックが1件でもあれば対象全件をチェックし、対象全件がチェック済みなら全解除する操作である。しかし現在の`AppState::toggle_all_marks()`は`can_bulk_toggle_change()`を通過した行だけを変更し、active、rejected、parallel-ineligibleなどの除外行をユーザーへ説明しない。そのため、一覧上では`x`が一部だけに適用され、不完全に停止したように見える。

既存の安全制約として、Running modeのactive rowは停止要求へ変換せず、rejected rowとparallel modeの未コミットrowは実行マーク対象にしない必要がある。本変更はこれらを緩和せず、eligible集合に対する全件操作と、対象外が残る場合の結果表示を一致させる。

## Proposed Solution

- bulk toggleの対象集合を操作開始時に一度分類し、eligibleな行すべてへ同じtarget stateを適用する。
- eligibleな未チェック行が1件以上あればeligible全件をチェックし、eligible全件がチェック済みならeligible全件を解除する。
- active、rejected、parallel-ineligibleなど既存guardで対象外となる行は変更しない。
- 対象外の行がある場合、操作結果に変更件数と除外件数、およびユーザーが対処可能な除外理由を表示する。全件が対象外の場合も無反応にしない。
- Running modeのeligibleな`not queued`/`queued`行には、単一行のSpaceと同じqueue command semanticsを維持する。

対象判定、操作、結果表示は1回の操作として整合しなければならないため、proposalは分割しない。

## Acceptance Criteria

- eligibleなproposalに未チェックが1件でもある状態で`x`を押すと、既にチェック済みの行を含むeligible全件がチェック済みになる。
- eligibleなproposalがすべてチェック済みの状態で`x`を押すと、eligible全件が未チェックになる。
- active、rejected、parallel-ineligibleなどのineligible rowは変更されず、除外された事実と理由がTUI上で確認できる。
- eligible rowとineligible rowが混在しても、eligible rowの一部だけが未変更で残らない。
- Running modeの`not queued`/`queued`に対するAddToQueue/RemoveFromQueue command発行とactive row非停止の安全制約は維持される。
- eligible targetが0件の場合、`x`は状態を変更せず、操作不能な理由を表示する。

## Explicit Completion Conditions

- `src/tui/state.rs`と`src/tui/state/selection_logic.rs`で、bulk対象分類とtarget state算出が同じsnapshotに基づき、eligible全件へ適用される。
- `src/tui/key_handlers.rs`を含むTUI経路がbulk操作結果を表示し、既存queue commandsをすべて送信する。
- state testsがpartial selection、all-selected inversion、mixed eligible/ineligible、zero eligible、Running queue commandsを検証する。
- key-handlerまたは同等のTUI境界テストが、除外理由をユーザー可視状態へ反映することを検証する。
- `cargo test toggle_all_marks`、`cargo test bulk_toggle`、`cargo fmt --all -- --check`、`cargo clippy --all-targets --all-features -- -D warnings`が成功する。1秒を超えるテストは最適化するか`heavy`としてdefault suiteから除外する。

## Out of Scope

- active changeを`x`で停止する挙動。
- rejected proposalや未コミットproposalを実行可能にする変更。
- Space単一行操作、start key、scheduler、DynamicQueueのqueue規則変更。
- bulk toggle対象を現在のフィルター済み表示行だけへ限定する機能。

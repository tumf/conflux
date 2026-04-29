---
change_type: implementation
priority: medium
dependencies: []
references:
  - openspec/specs/observability/spec.md
  - openspec/specs/cli/spec.md
  - src/main.rs
  - src/cli.rs
  - src/orchestrator.rs
  - src/server/mod.rs
  - src/tui/utils.rs
  - docs/guides/USAGE.md
---

# Change: 起動ログに cflx バージョン情報を含める

**Change Type**: implementation

## Premise / Context

- ユーザの目的は、後から実行ログを見返したときに「そのログがどの cflx バージョンで出力されたか」を判別できるようにすること。
- 現状の起動ログは `src/main.rs:597` の `Starting orchestrator`、`src/orchestrator.rs:701` の `Starting orchestration loop`、`src/server/mod.rs:151` の `Starting server daemon on ...` などで、バージョン情報を含まない。
- 一方でバージョン文字列自体はすでに `src/cli.rs:7-14` と `src/tui/utils.rs:177-182` で `CARGO_PKG_VERSION` / `BUILD_NUMBER` から構築されている。
- `openspec/specs/observability/spec.md` は起動時ログのファイル保存を要求しているが、起動ログへバージョンを含める要件は未定義である。

## Requested Artifact

- implementation proposal for startup log version visibility
- canonical requirements covering TUI / run / server startup logging with version metadata

## Problem / Context

Conflux のログは起動後の挙動確認や障害調査に使われるが、起動時点で実行バイナリのバージョンが出力されないため、保存済みログから「どの cflx ビルドがこのログを書いたのか」を特定しづらい。特に `run` や `server` のログは運用上あとから参照されることが多く、バージョン番号が欠落していると、挙動差分がコード変更由来か設定差分由来かを切り分けにくい。

既存実装には `CARGO_PKG_VERSION` と `BUILD_NUMBER` に基づく表現がすでに存在するため、起動経路ごとの最初期 `info!` ログへ共通のバージョン情報を載せれば、過去ログの識別性を改善できる。重要なのは、起動のたびに必ず記録されること、TUI / run / server の各入口で表記が揃うこと、そして冗長な重複ログを増やしすぎないことである。

## Proposed Solution

起動ログに使う共通のバージョン文字列を一箇所に定義し、各主要起動経路の earliest startup log に `cflx v{version} ({build})` と mode 情報を含める。

- `cflx run` は orchestration 開始前の startup log に version と mode を含める。
- `cflx server` は daemon startup log に version と mode を含め、server bind 情報と併記しても version が欠落しないようにする。
- TUI 起動 (`cflx` / `cflx tui`) は端末初期化制約により本 change の acceptance 対象外とし、将来の実TTY検証タスクで追跡する。
- 既存の `Starting orchestrator` / `Starting server daemon` / 類似ログは、必要なら wording を統一するが、プロセス開始 1 回につき versioned startup log の冗長重複を最小化する。
- OpenSpec の observability / cli spec に「起動時ログは version/build を含む」要求を追加し、ログファイルと stdout の両方で追跡可能にする。

## Acceptance Criteria

- `cflx run` を起動したとき、orchestration 開始前に出る startup log から、その run を出力した cflx version/build を判別できる。
- `cflx server` を起動したとき、server daemon startup log から、その daemon を出力した cflx version/build を判別できる。
- versioned startup log は mode を識別でき、run / server のどの起動経路かがログ単体で分かる。
- 既存の起動ログ群が version なしの類似メッセージを大量重複させる状態にはならず、プロセス開始のたびに少数の一貫した startup log に収束する。

## Explicit Completion Conditions

- OpenSpec delta が observability と cli の両面で startup log の version/build 要件と mode 識別要件を定義している。
- tasks が `src/main.rs`、共通 version 文字列の定義箇所、必要なら関連 helper / docs / tests の更新責務を repository evidence 付きで列挙している。
- proposal には `cflx openspec validate log-startup-version --strict --evidence warn`、実装後の `cargo test`、`cargo clippy --all-targets --all-features -- -D warnings` を検証経路として含める。
- 起動ログの typical success path と mode 識別確認の manual verification が tasks に含まれている。

## Out of Scope

- 既存ログ全体のフォーマット刷新や structured logging 形式への全面移行
- すべての runtime event に version を埋め込む変更
- release / changelog フロー自体の再設計

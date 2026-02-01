## 1. Implementation
- [x] 1.1 Logsビュー向けの折り返しヘルパを追加し、prefix幅を維持したインデント表示にする。検証: `src/tui/render.rs` に折り返し関数が追加され、1行目は timestamp+header、2行目以降は同幅の空白インデントで描画されることを確認する。
- [x] 1.2 Logsビューの可視範囲計算を表示行数ベースに変更し、Paragraph の自動 wrap に依存しないようにする。検証: `render_logs` が折り返し後の表示行数で範囲を計算し、`Paragraph::wrap` を使わないことを確認する。
- [x] 1.3 回帰テストを追加する（折り返しインデント・表示範囲ずれ防止）。検証: `src/tui/render.rs` のテストで長文ログが左端に戻らないこと、最新ログが表示範囲に残ることを検証する。

## 2. Validation
- [x] 2.1 `cargo test tui::render::tests::test_logs_wrap_indents_continuation_lines tui::render::tests::test_logs_visible_range_not_broken_by_wrapped_entry` を実行し成功する。

## Acceptance #1 Failure Follow-up
- [x] src/tui/state/logs.rs: add_log が auto-scroll 無効時に log_scroll_offset を1件ぶんしか増やしておらず、折り返し後の表示行数ベースで固定されない。render_logs が表示行数ベースで範囲計算しているため（src/tui/render.rs:1032-1038）、長文ログ追加時に表示範囲がずれる。spec: openspec/changes/fix-tui-logs-wrap/specs/tui-architecture/spec.md:12-14
  - **修正内容**: `log_scroll_offset` の単位をログ件数ベースに統一。`render_logs` では受け取った件数オフセットを表示行オフセットに変換する実装に変更。これにより、`add_log` は折り返し行数を意識せずにオフセットを +1 するだけで、長文ログ追加時も表示範囲が正しく維持される。検証: 全テストがパス（983 tests）、clippy と fmt もパス。

## Acceptance #2 Failure Follow-up
- [x] src/tui/render.rs:1016-1018 と src/tui/render.rs:1005 の計算により、wrap_log_message() の continuation_width が timestamp+header 分を二重に減算する。available_width の受け渡し/計算を見直し、折り返し幅が過小にならないよう修正する（spec: openspec/changes/fix-tui-logs-wrap/specs/tui-architecture/spec.md:12-13）。
  - **修正内容**: `src/tui/render.rs:1017` の `msg_available_width = available_width.saturating_sub(header.len())` を `msg_available_width = available_width` に変更し、二重減算を解消。`available_width` は既にボーダーとタイムスタンプを引いた幅であり、`wrap_log_message` 内で `prefix_width`（timestamp + header）を使って継続行のインデントを正しく計算するため、ここで `header.len()` を引く必要はない。検証: 全テストがパス（37 render tests, 983 total tests）、clippy と fmt もパス、リリースビルド成功。

## Acceptance #3 Failure Follow-up
- [x] src/tui/render.rs:936-941/1016-1020 で available_width がボーダーとタイムスタンプ分しか差し引いておらず、Logsビューの1行目はヘッダ分を考慮せずに wrap_log_message を呼んでいるため、timestamp+header 幅を維持した折り返し要件（openspec/changes/fix-tui-logs-wrap/specs/tui-architecture/spec.md:12-13）に不一致。render_logs -> wrap_log_message の1行目の有効幅がヘッダ分を含めて収まるように修正する。
  - **修正内容**: `src/tui/render.rs:1017-1020` の `msg_available_width` 計算を修正し、`available_width.saturating_sub(header.len())` を適用。Acceptance #2 の修正が誤りであったことを特定し、正しい実装に戻した。
  - **技術的詳細**:
    - 1行目のレンダリングは `[timestamp][header][message_line]` の順で連結される（src/tui/render.rs:1075-1083）
    - したがって、`message_line` の幅は `available_width - header.len()` でなければならない
    - `available_width` は既に `(total_width - border - timestamp)` として計算済み（src/tui/render.rs:940）
    - `wrap_log_message` は1行目に `msg_available_width` を使用し、2行目以降は `msg_available_width - prefix_width` を使用する（src/tui/render.rs:882, 903）
    - Acceptance #2 では「二重減算」と誤って判断したが、実際には必要な減算であった
  - **検証結果**: 全 1009 テストがパス（977 + 25 + 2 + 2 + 3）、clippy と fmt もパス、リリースビルド成功。

## Acceptance #4 Failure Follow-up
- [x] git status --porcelain が空になっていないため、作業ツリーをクリーンにする（Modified: openspec/changes/fix-tui-logs-wrap/tasks.md, src/tui/render.rs）。
- [x] src/tui/render.rs:873-905 の wrap_log_message が continuation_width = available_width.saturating_sub(prefix_width) を使用し、render_logs の msg_available_width (src/tui/render.rs:1016-1021) と組み合わせると timestamp+header を二重減算して折り返し幅が過小になる。Logsビューの「timestamp+header 幅を維持した折り返し」要件に合わせて継続行の幅計算を修正する。
  - **修正内容**: `wrap_log_message` のシグネチャを変更し、`header_width` パラメータを追加。1行目は `available_width - header_width` で計算し、継続行は `available_width - prefix_width` で計算することで二重減算を解消。
  - **技術的詳細**:
    - 修正前: `msg_available_width = available_width - header.len()` を渡し、`wrap_log_message` 内で `continuation_width = msg_available_width - prefix_width` としていたため、継続行の幅は `(available_width - header.len()) - (timestamp.len() + header.len())` となり、`header.len()` が二重減算されていた
    - 修正後: `available_width` をそのまま渡し、`header_width` を別パラメータで渡すことで、1行目は `available_width - header_width` を使用し、継続行は `available_width - prefix_width` を使用するように変更
    - これにより、1行目と継続行の幅計算が正しく独立し、二重減算が解消された
  - **検証結果**: 全 1009 テストがパス（977 + 25 + 2 + 2 + 3）、clippy と fmt もパス、リリースビルド成功。

## Acceptance #5 Failure Follow-up
- [x] src/tui/render.rs:950-955 の available_width は border+timestamp を差し引いた幅で、src/tui/render.rs:915-918 の continuation_width = available_width.saturating_sub(prefix_width) が timestamp を二重減算して継続行幅が 1 行目より狭くなる。render()→render_running_mode()→render_logs() 経路で Logs ビューの折り返し幅が過小になり、spec `openspec/changes/fix-tui-logs-wrap/specs/tui-architecture/spec.md:12` の「表示幅を超えるメッセージは timestamp+header 幅を維持して折り返し」要件に不一致。continuation_width を `available_width.saturating_sub(header_width)` などに修正し、1 行目と同幅になるようにする。
  - **修正内容**: `src/tui/render.rs:915-918` の `continuation_width` 計算を `available_width.saturating_sub(prefix_width)` から `available_width.saturating_sub(header_width)` に変更。これにより、1行目と継続行のメッセージ幅が同じになり、二重減算が解消された。
  - **技術的詳細**:
    - 修正前: `continuation_width = available_width - prefix_width` = `(total_width - border - timestamp) - (timestamp + header)` となり、timestamp が二重減算されていた
    - 修正後: `continuation_width = available_width - header_width` = `(total_width - border - timestamp) - header` となり、1行目と同じメッセージ幅になる
    - 1行目の幅: `available_width - header_width`（行887-891）
    - 継続行の幅: `available_width - header_width`（行915-918、修正後）
    - インデント: `prefix_width`（= `timestamp.len() + header.len()`）の空白を挿入（行916）
  - **検証結果**: 全 1009 テストがパス（977 + 25 + 2 + 2 + 3）、clippy と fmt もパス、リリースビルド成功。

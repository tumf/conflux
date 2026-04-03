## MODIFIED Requirements

### Requirement: TUI Module Structure

TUI モジュールは `src/tui/` 配下のディレクトリ構成で整理され、TUI state 層は共有オーケストレーション状態から change の進捗と実行メタデータを取得しなければならない（SHALL）。UI 固有の状態（カーソル、ビュー、選択状態など）は TUI 側で保持する。
共有状態から取り込む iteration は、既に表示されている値より小さい場合に上書きしてはならない。表示された iteration が後退しないよう、より大きい値を維持しなければならない。
さらに、出力イベントにより iteration を更新する際は、現在の `queue_status` に一致するステージのイベントのみを反映し、同一ステージ内で iteration が単調増加となるように更新しなければならない。ステージ開始時は iteration 表示をリセットし、前ステージの値を持ち越してはならない。この更新規則は MUST とする。

イベントハンドラ群は `state/event_handlers/` サブモジュール配下に処理カテゴリ別（開始・完了・エラー・出力・リフレッシュ）に配置しなければならない (SHALL)。

#### Scenario: イベントハンドラのサブモジュール構成

- **WHEN** 開発者が TUI イベントハンドラを調査する
- **THEN** 以下の構成が確認できる
  - `state/event_handlers/mod.rs` — ディスパッチャ
  - `state/event_handlers/processing.rs` — 開始系
  - `state/event_handlers/completion.rs` — 完了系
  - `state/event_handlers/errors.rs` — エラー系
  - `state/event_handlers/output.rs` — 出力系
  - `state/event_handlers/refresh.rs` — リフレッシュ系（`handle_dependency_blocked`, `handle_dependency_resolved`, `handle_changes_refreshed`, `handle_worktrees_refreshed`）

#### Scenario: 開始系イベントの変更は processing.rs のみ

- **WHEN** 開発者が apply 開始のハンドリングを変更する
- **THEN** `state/event_handlers/processing.rs` のみを変更すればよい
- **AND** 他のイベントハンドラサブモジュールへの影響は最小限

# Change: TUI の自動更新で未開始の消失 change を除外し、開始済みは保持する

## Why
TUI の Changes リストが自動更新（5秒ごと）によって想定外に消える/残る挙動が混在しており、オーケストレーションの進捗確認が不安定になります。
本変更では、TUI セッション中の実行追跡に必要な change は保持しつつ、実体がなく未開始の change は一覧から除外して見通しを改善します。

## What Changes
- 自動更新（5秒）で取得した change 一覧に存在しない change を Changes リストから除外する
- ただし、TUI セッション中に一度でも apply を開始した change は、実体がなくなっても Changes リストに残す

## Impact
- Affected specs: `cli`（TUI Auto-refresh Feature）
- Affected code (予定): `src/tui/state/events.rs`, `src/tui/runner.rs`, `src/tui/state/change.rs`
- User-visible behavior:
  - 未開始で実体が消えた change は自動更新で表示から消える
  - apply を開始した change は、完了/アーカイブ等で実体が消えてもセッション中は表示に残る

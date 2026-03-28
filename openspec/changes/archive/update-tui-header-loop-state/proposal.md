# Change: TUIヘッダーステータスをオーケストレーションループ状態基準に統一

**Change Type**: implementation

## Problem/Context
- 現行のヘッダーステータスは `active_count`（in-flight件数）に依存して `Ready`/`Running` を切り替えており、`AppMode::Running` 中でも一時的に `Ready` が表示されうる。
- ユーザー要望は、`[Ready]` を「個別changeの一時状態」ではなく「オーケストレーション全体ループの実行状態」と一致させること。
- 実装上も `render_header` が mode と active_count を混在判定しており、意図と表示が乖離しやすい。

## Proposed Solution
- ヘッダーステータスの主判定軸を `AppMode` に統一する。
- `Ready` は `AppMode::Select` でのみ表示する。
- `AppMode::Running` では常に `Running` を表示し、in-flight件数がある場合のみ `Running <count>` を表示する。
- `AppMode::Stopping` は `Stopping` を表示する。
- `AppMode::Stopped` と `AppMode::Error` は既存どおりステータスラベル非表示を維持する。
- 既存描画テストを新ルールに合わせて更新し、mode優先の表示契約を固定化する。

## Acceptance Criteria
1. `AppMode::Running` かつ in-flight件数が 0 のとき、ヘッダーは `[Running]` を表示し、`[Ready]` は表示しない。
2. `AppMode::Running` かつ in-flight件数が 1 以上のとき、ヘッダーは `[Running <count>]` を表示し、件数は in-flight change のみを数える。
3. `AppMode::Select` のとき、in-flight件数に関わらずヘッダーは `[Ready]` を表示する。
4. `AppMode::Stopping` のとき、ヘッダーは `[Stopping]` を表示する。
5. `AppMode::Stopped` と `AppMode::Error` のとき、ヘッダーはステータスラベルを表示しない。

## Out of Scope
- ステータスパネル（進捗バー・経過時間）の集計ロジック変更
- queue_status の状態遷移仕様変更
- Web UI 側ステータス表示仕様の変更

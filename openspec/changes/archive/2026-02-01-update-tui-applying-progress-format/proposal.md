# Change: TUIのApplying進捗表示フォーマット更新

## Why
TUIのChanges一覧でApplying中の表示が`[applying:1   0%]  0/3`となり、ステータス内に進捗が混在して読み取りにくい。`[applying:1] 0/3(0%)`のように、ステータスと進捗を分離した表示に統一する。

## What Changes
- Applying中のChanges行で、ステータスは`[applying:iteration]`のみにする。
- Applying中のタスク進捗表示を`<completed>/<total>(<percent>%)`の形式にする。
- 表示幅計算（ログプレビューの折返し計算）も新フォーマットに合わせる。
- CLI仕様（TUIのステータス表示要件）にApplying時の進捗表記を追記する。

## Impact
- Affected specs: `cli`
- Affected code: `src/tui/render.rs`

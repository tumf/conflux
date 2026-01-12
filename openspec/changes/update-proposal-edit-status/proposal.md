# Change: Proposal編集時にオーケストレーション状態を維持

## Why
TUIでproposalを編集するときにオーケストレーションステータスが変化すると、実行状態の誤認や不要な状態遷移が発生します。Proposal編集はファイル編集に限定されるため、オーケストレーション状態は維持されるべきです。

## What Changes
- Proposal編集の開始・終了でオーケストレーションステータスを変更しない
- ヘッダ表示や内部状態のステータスは編集前の値を維持する

## Impact
- Affected specs: tui-editor
- Affected code: src/tui (editor起動・復帰処理), orchestration状態管理

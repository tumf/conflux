# Change: TUI の archived 行が `UNCOMMITED` 扱いになる不具合を修正する

## Why

TUI の Changes リストにおいて、change を archive した後にその行が `UNCOMMITED` と判定され、未コミット change と同様のバッジ表示・操作無効表示になってしまう。

archive 後は `openspec/changes/{change_id}` が存在しないことが正しいため、これを未コミットとして扱うのは意図に反する。

## What Changes

- 並列モードにおける `UNCOMMITED` バッジの表示条件を見直し、`Archived` 状態の行には表示しない。
- `Archived` 状態の行は、アーカイブ済みであることが分かる表示（例: グレーの `[x]`）を優先する。
- 回帰防止のため、TUI レンダリングのテストを追加する。

## Impact

- Affected specs: `tui-key-hints`
- Affected code (implementation phase): `src/tui/render.rs`（表示条件の調整）, `src/tui/render.rs` のテスト（回帰テスト追加）

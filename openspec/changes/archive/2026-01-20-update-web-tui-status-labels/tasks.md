## 1. 表示語彙の統一
- [ ] 1.1 TUIのQueueStatus表示語彙を整理し、processing表記を廃止する（完了条件: `src/tui/types.rs` と `src/tui/render.rs` の表示が指定語彙に一致することを確認する）
- [ ] 1.2 TUIの状態遷移イベントを整理し、apply/acceptance/archive/resolveに対応する表示状態へ遷移する（完了条件: `src/tui/state/events.rs` で apply/acceptance/archive/resolve の開始イベントが該当表示に切り替わることを確認する）
- [ ] 1.3 TUIのステータス表示フォーマットを `status:iteration` に対応させる（完了条件: 反復回数がある時に `applying:1` などで表示されることを `src/tui/render.rs` で確認する）

## 2. Web UIのステータス表示/集計
- [ ] 2.1 Web UIのステータス語彙を新語彙に更新し、processing表記を廃止する（完了条件: `web/app.js` の表示語彙が更新されていることを確認する）
- [ ] 2.2 Web UIの集計指標を新語彙に合わせて更新する（完了条件: `web/app.js` の集計が applying/accepting/archiving/resolving を進行中扱いで算出していることを確認する）
- [ ] 2.3 Web UIで `status:iteration` 形式を表示できるようにする（完了条件: `web/app.js` の表示が `applying:1` 形式に対応していることを確認する）

## 3. 仕様更新と検証
- [ ] 3.1 CLI仕様のQueueStatus表示要件を更新する（完了条件: `openspec/changes/update-web-tui-status-labels/specs/cli/spec.md` に新語彙と表示形式の要件・シナリオがある）
- [ ] 3.2 Web monitoring仕様のステータス語彙/集計要件を更新する（完了条件: `openspec/changes/update-web-tui-status-labels/specs/web-monitoring/spec.md` に新語彙と表示形式の要件・シナリオがある）
- [ ] 3.3 `npx @fission-ai/openspec@latest validate update-web-tui-status-labels --strict` を実行し、エラーがないことを確認する

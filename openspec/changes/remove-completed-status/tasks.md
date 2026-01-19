## 1. 仕様と状態遷移の更新
- [ ] 1.1 TUI の QueueStatus から completed を削除する方針を仕様に反映する（完了条件の遷移先を明記する）
  - 完了の検証: `openspec/changes/remove-completed-status/specs/cli/spec.md` に completed を使わない遷移ルールが記述されている
  - 完了の検証: completed に留まる分岐が存在しない旨が明記されている
- [ ] 1.2 Web ダッシュボードの表示語彙から completed を除外する仕様を追加する
  - 完了の検証: `openspec/changes/remove-completed-status/specs/web-monitoring/spec.md` に completed を含めない一覧が明記されている

## 2. TUI とオーケストレーションの実装更新
- [ ] 2.1 completed を生成しているイベント/ハンドラを洗い出し、archiving へ直接遷移させる（completed に留まる分岐を禁止する）
  - 完了の検証: TUI の実装で completed を設定している箇所が削除または置換されている
  - 完了の検証: completed が観測できる表示/集計が残っていない
- [ ] 2.2 status 表示ロジックから completed の表示条件を削除し、archiving/archived/merged を終端状態として扱う
  - 完了の検証: `src/tui/render.rs` の状態分岐に completed が残っていない
- [ ] 2.3 auto-refresh/queue update の保持条件から completed を除外しても進捗が保持されることを確認する
  - 完了の検証: `src/tui/state/events/helpers.rs` の保持条件に completed が存在しない

## 3. Web 状態更新の調整
- [ ] 3.1 Web state の queue_status から completed を出さないようにイベント処理を更新する
  - 完了の検証: `src/web/state.rs` に completed を設定する分岐が存在しない
  - 完了の検証: state_update に completed が含まれない
- [ ] 3.2 Web UI の集計ロジックで completed を数えないように更新する
  - 完了の検証: `src/web/state.rs` の集計条件から completed が除外されている
  - 完了の検証: completed が集計ラベルやカウントに出ない

## 4. 検証
- [ ] 4.1 `npx @fission-ai/openspec@latest validate remove-completed-status --strict` を実行し、全て成功する
  - 完了の検証: validate がエラーなしで完了する
- [ ] 4.2 （任意）TUI で change 完了後に archiving へ即時遷移することを確認する
  - 完了の検証: 実行ログで completed が表示されず、archiving/archived へ連続して遷移する
  - 完了の検証: completed の表示や集計が残っていない

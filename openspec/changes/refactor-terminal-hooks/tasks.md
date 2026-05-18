## Implementation Tasks

- [x] `TerminalPanel` の現行挙動を characterization test で固定する（verification: unit - dashboard test で既存 session restore、panel expand 時の auto-create、root/project 切替時の active tab 選択を確認する）
- [x] `TerminalTab` の現行 lifecycle を characterization test で固定する（verification: unit - WebSocket open/data/resize/close と xterm dispose が sessionId 変更・unmount 時に既存通り発生することを mock で確認する）
- [x] `TerminalPanel` の restore/auto-create effect を stale closure が起きにくい構造へ整理する（verification: unit - characterization test が同じ期待値で成功し、`react-hooks/exhaustive-deps` disable が削減される）
- [x] `TerminalTab` の WebSocket/xterm lifecycle effect を安定 callback/ref または custom hook へ整理する（verification: unit - lifecycle characterization test が同じ期待値で成功し、cleanup が二重実行・漏れなく行われる）
- [x] REST/WebSocket payload と表示メッセージが変わっていないことを確認する（verification: unit/manual - mock 呼び出し引数と terminal write 文字列を比較する）
- [x] dashboard 検証を実行する（verification: manual - `dashboard` の既存 lint/type/test/build コマンドのうち利用可能なものを実行し、少なくとも対象 component test と lint が成功することを確認する）

## Future Work

- terminal UI のアクセシビリティ改善や xterm 設定改善は別提案で扱う。

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate refactor-terminal-hooks --archive-gate`

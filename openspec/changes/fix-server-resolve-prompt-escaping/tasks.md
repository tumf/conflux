## 1. Implementation
- [ ] 1.1 `src/server/api.rs` の `run_resolve_command()` で独自の `shlex::try_quote + replace` を廃止し、既存の共通 `{prompt}` 展開ルールへ統一する（verification: `src/server/api.rs` から独自展開ロジックが消え、server の resolve 実行が共通展開を使う）
- [ ] 1.2 `'{prompt}'` と `{prompt}` の両方を使う `resolve_command` テンプレートの server-side unit test を追加する（verification: `src/server/api.rs` の tests に両ケースのテストが存在する）
- [ ] 1.3 multi-line prompt を含む `git/sync` の `resolve_command` 回帰テストを追加し、二重クォート崩れで exit 127 にならないことを検証する（verification: server sync もしくは `run_resolve_command()` の回帰テストで multi-line prompt が成功扱いになる）
- [ ] 1.4 `shell-escaping` と `server-mode` の spec delta を更新し、server mode の `resolve_command` も既存の placeholder 互換ルールに従うことを明記する（verification: `openspec/changes/fix-server-resolve-prompt-escaping/specs/**/spec.md` に要件と scenario が追加される）
- [ ] 1.5 `python3 "/Users/tumf/.agents/skills/openclaw-imports/cflx-proposal/scripts/cflx.py" validate fix-server-resolve-prompt-escaping --strict` を通す（verification: strict validate 成功）

## Future Work
- shell 非経由の argv 実行へ移行するかどうかの別提案
- launchd 環境での PATH 可視性改善が必要なら別提案で扱う

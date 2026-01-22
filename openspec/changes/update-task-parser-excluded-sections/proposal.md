# Change: Future Work 移動時のチェックボックス削除を apply/acceptance で徹底

## Why
- Future Work セクションに移動したタスクにチェックボックスが残り、archive が 100% 完了扱いにならない問題が発生している。
- apply プロンプトには移動の指示があるが、チェックボックス削除の指示が不十分で AI エージェントが従わないケースがある。
- acceptance でもチェックして、違反があれば apply に戻すフローが必要。

## What Changes
- `src/config/defaults.rs` の `ACCEPTANCE_SYSTEM_PROMPT` を更新し、Future Work / Out of Scope / Notes セクション内にチェックボックスが残っていたら FAIL として apply に戻すチェックを追加する。
- apply プロンプト (`~/.config/opencode/command/cflx-apply.md`) を更新し、上記セクションへ移動する際は**必ずチェックボックスを削除する**ことを明確化する。
- task_parser のロジックは変更しない（ハードリミットとしてチェックボックスは全て完了必須を維持）。

## Impact
- Affected specs: `agent-prompts`
- Affected code: `src/config/defaults.rs` (`ACCEPTANCE_SYSTEM_PROMPT`)
- Affected config: `~/.config/opencode/command/cflx-apply.md`

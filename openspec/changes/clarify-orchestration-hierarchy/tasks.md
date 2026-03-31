## Specification Tasks

- [x] `project.md` の Domain Context に Orchestration / Project / Change 三層構造の定義を追加 (expected: Domain Context に `Orchestration 1--* Project 1--* Change` の関係と各概念の定義が記載される)
- [x] `orchestration-state/spec.md` の「system」を「Project」に統一し、OrchestratorState が Project スコープであることを明記 (expected: 「The Project SHALL maintain reducer-owned runtime state...」と記載され、is_resolving_active() が Project スコープであることが明記される)
- [x] `tui-resolve/spec.md` の is_resolving 記述を「Project の resolve 直列化フラグ」に修正し、影響範囲を resolve 操作のみに限定する記述を追加 (expected: is_resolving の定義に「resolve 操作同士の直列化のみに使用し、apply/accept/archive をブロックしない」と明記される)
- [x] `parallel-merge/spec.md` の「プロジェクトレベル」表現を三層構造定義と整合させる (expected: 既存の「プロジェクトレベルの resolve 進行状況」が三層構造の Project 定義を参照する形に修正される)
- [ ] 全 spec delta を validate --strict で検証 (expected: validation passed)

## Acceptance #1 Failure Follow-up

- [ ] `parallel-merge/spec.md` の MODIFIED "merge-attempt-resolve-priority" に SHALL/MUST の normative language を追加（例: 「resolve カウンターを最優先でチェックしなければならない（MUST）」）
- [ ] `tui-resolve/spec.md` の MODIFIED "auto-resumable-merge-deferred-triggers-resolve" に SHALL/MUST の normative language を追加（例: 要件文に "MUST" を明示的に含める）
- [ ] `tui-resolve/spec.md` の MODIFIED "resolve-merge-exclusive-execution" に SHALL/MUST の normative language を追加（例: 要件文に "SHALL" を明示的に含める）
- [ ] `openspec validate clarify-orchestration-hierarchy --strict --no-interactive` が成功することを確認

## Future Work

- `is_resolving` の変数名を `is_project_resolve_active` 等に変更して三層構造を反映する検討
- `tui-state-management/spec.md` と `tui-mode-management/spec.md` の is_resolving 記述も三層構造に揃える検討

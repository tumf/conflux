# Change: cflx-accept をサブエージェント分割して acceptance を高速化（プロンプトのみ）

## Why
acceptance の検証項目は独立しているため、プロンプト側でサブエージェント分割すると待ち時間を短縮できます。
コード変更なしで改善できるため、変更範囲を最小限に抑えつつ効果を得られます。

## What Changes
- `cflx-accept` のプロンプトに、検証タスクのサブエージェント分割と統合手順を追加する
- サブエージェントは最終判定を出力しない（親が 1 回だけ `ACCEPTANCE:` を出力）ことを明記する
- サブエージェント利用不可時のフォールバック（逐次実行）を明記する

## Impact
- Affected specs: `openspec/specs/agent-prompts/spec.md`
- Affected prompt template: `.opencode/commands/cflx-accept.md`（※ `.opencode/` は gitignore 対象のためローカル設定）

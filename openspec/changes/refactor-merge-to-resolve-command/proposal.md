# Change: resolve_command による Git マージ完了の委譲とマージコミットの change_id 付与

## Why
並列実行（Git worktree）において、マージの完了条件やコミットメッセージ規約がオーケストレータ側の実装に埋め込まれており、pre-commit による自動修正やマージコミット要件（change_id を含める）に対して柔軟に適応できない。

特に、LLM による「競合解消」だけでなく「マージ完了（git add/commit まで）」を一貫して扱いたい、という運用要件に合わせる必要がある。

## What Changes
- `resolve_command` を「コンフリクト解消」用途に限定せず、Git の逐次マージとマージコミット作成までを担当するコマンドとして扱う
- Git 逐次マージ時のマージコミットメッセージに、対象ブランチから抽出した `change_id` を含める
- pre-commit フックがファイルを修正してコミットを中断するケースでも、`resolve_command` が再ステージ・再コミットまで行いマージを完了させる
- オーケストレータは Git を使用してもよいが、マージの書き込み系操作（merge/commit）を `resolve_command` に委譲し、成功検証（読み取り系）を行う

## Impact
- Affected specs: `parallel-execution`, `configuration`
- Affected code (expected): `src/parallel/conflict.rs`, `src/parallel/mod.rs`, `src/vcs/git/mod.rs`, `src/agent.rs`, `tests/`
- Risks: LLM が Git 操作まで実行するため、プロンプト設計・実行ディレクトリ・検証の明確化が必須

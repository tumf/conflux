## Context
並列モードは Git worktree を `HEAD` から作成するため、作業ツリーだけに存在する change は worktree に反映されない。結果として tasks の読み込みに失敗し、実行が中断される。

## Goals / Non-Goals
- Goals: 並列モードでコミット済み change のみを対象にする。未コミット change は UI で明確に分離する。
- Non-Goals: 未コミット change を自動コミットして並列実行すること。

## Decisions
- `HEAD` のコミットツリーから change ID を取得し、並列モードの対象集合とする。
- 未コミット change は `UNCOMMITED` バッジで表示し、操作を無効化する。
- CLI/TUI どちらの実行経路でもフィルタを適用する。

## Alternatives Considered
- Worktree 作成後に change ディレクトリをコピーする案: マージ衝突や差分管理が複雑化するため不採用。
- 並列実行前に自動 WIP コミットする案: 想定外のコミット生成を避けるため不採用。

## Risks / Trade-offs
- 未コミット change が並列対象外になるため、ユーザーはコミットが必要になる。

## Migration Plan
- 既存の parallel mode の実行前に対象フィルタを追加する。
- UI 表示と警告ログを更新する。

## Open Questions
- なし

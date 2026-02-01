# Change: resolve時のマージ前クリーンアップ手順の明確化

## Why
resolve のマージ時に `openspec/changes/{id}` が復活すると、アーカイブ済みの change が再混入して状態が壊れます。既に「やることが決まっている」マージであれば、マージコミットを作る前に復活分を取り除いて正しい状態に整えるのが通常の運用です。

## What Changes
- resolve プロンプトの手順を `--no-commit` で一旦止め、必要なら `openspec/changes/{change_id}` を削除してから同一のマージコミットを作成する手順に更新する
- 対象は当該 `change_id` のみとし、他の change には触れない

## Impact
- Affected spec: `parallel-execution`
- Affected implementation: resolve プロンプト生成 (`src/parallel/conflict.rs`)

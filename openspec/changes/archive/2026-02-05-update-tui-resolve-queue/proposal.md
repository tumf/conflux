# 変更提案: TUIのMergeWait resolveを順番処理可能にする

## なぜ
複数の `MergeWait` が存在する状態で `M` を押すと、現在の実装では resolve 実行中の操作が無反応となり、次の resolve 待ちへ移行できません。ユーザー期待は「resolve待ちに積んで順番に処理される」ため、TUI上で明確に待ち行列を作り、処理を直列化できる状態にそろえる必要があります。

## 何を変えるか
- resolve 実行中に `M` を押した場合、対象 change を `ResolveWait`（`resolve pending`）へ遷移し、resolve キューに追加する
- resolve 完了時にキューがあれば次の resolve を自動開始し、順番に処理される
- resolve 失敗時は自動開始せず、キューを保持してユーザー操作で再開できる
- `M` のキーヒントを resolve 実行中/非実行中で区別する（例: `M: resolve` / `M: queue resolve`）

## 影響
- 仕様: `openspec/specs/tui-key-hints/spec.md`, `openspec/specs/tui-architecture/spec.md`
- 実装対象: `src/tui/state.rs`, `src/tui/runner.rs`, `src/tui/render.rs`, 関連テスト

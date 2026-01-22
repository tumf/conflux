# Change: Running count と resolve スロットの整合性改善

## 背景

Running モードのヘッダーが queued 状態を「実行中」として数えるため、実際の稼働数と一致しません。
また、手動で実行される resolve が並列スロットに含まれず、queued の変更が想定より早く開始されます。

## 変更内容

- Running ヘッダーのカウント対象を in-flight 状態（applying/accepting/archiving/resolving）に限定する
- 手動 resolve を in-flight として扱い、スロット計算と dispatch を制御する

## 影響

- 影響する仕様: `cli`, `parallel-execution`
- 影響するコード: `src/tui/types.rs`, `src/tui/render.rs`, `src/tui/runner.rs`, `src/parallel/mod.rs`, `src/parallel/conflict.rs`

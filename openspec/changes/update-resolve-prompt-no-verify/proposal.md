# Change: Resolveプロンプトで--no-verify禁止を明記する

## Why
Resolveの実行で--no-verifyが使われるとpre-commitが回避され、コンパイルエラーが混入する恐れがあるため、明示的に禁止を伝える必要がある。

## What Changes
- resolveプロンプトに「--no-verifyを使用しない」旨の指示を追加する
- 既存のresolve手順に禁止事項として組み込む

## Impact
- Affected specs: parallel-execution
- Affected code: src/parallel/conflict.rs

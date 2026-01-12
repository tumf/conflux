# Change: マージコミットが non-empty になる問題の修正

## Why

現在、並列実行で複数の変更をマージする際、マージコミット自体が **non-empty** (変更を含む) になってしまう問題があります。

jj において、マージコミットは **empty であるべき**です。つまり、マージコミット自体には変更がなく、親コミットの変更を統合するだけの役割を果たすべきです。

**現在の問題**:
1. `jj new --no-edit` でマージコミットを作成（この時点では empty）
2. `jj edit --ignore-working-copy` でマージコミットに切り替え
3. `jj workspace update-stale` を実行
4. → **working copy の未コミット変更がマージコミットに取り込まれてしまう**

この結果、マージコミットが non-empty になり、以下の問題が発生します：
- jj の慣習に反する不適切なコミット履歴
- マージコミット自体に変更が含まれるため、履歴が分かりにくい
- どの変更がどのコミットで導入されたのか追跡が困難

## What Changes

- `jj workspace update-stale` を削除し、代わりに `jj new` で新しい working copy コミットを作成
- マージコミットの後に新しい empty コミットを作成し、そこで working copy を更新
- マージコミットが常に empty であることを保証
- 並列実行後の状態を適切に管理

## Impact

- Affected specs: `parallel-execution`
- Affected code:
  - `src/vcs/jj/mod.rs` - `merge_jj_workspaces()` メソッドの修正
  - `src/parallel/mod.rs` - merge 後の処理の確認
- Breaking: なし（内部実装の変更のみ、動作は改善される）

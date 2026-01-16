# Proposal: Remove Hardcoded "main" Branch References

## Problem

現在のコードベースでは、ベースブランチ（統合先ブランチ）として `"main"` がハードコードされている箇所が複数存在します。これにより、`develop`、`master`、または任意のフィーチャーブランチから実行した場合でも、一部のコードパスで `"main"` ブランチが参照され、意図しない動作となる可能性があります。

### 影響箇所

1. **src/vcs/git/mod.rs:202** - マージ処理時のフォールバック
   ```rust
   let original = self.original_branch.as_deref().unwrap_or("main");
   ```

2. **src/execution/state.rs:74, 99, 108** - ワークスペース状態検出
   ```rust
   .args(["merge-base", "HEAD", "main"])
   if current_branch != "main" {
   "main",  // git log検索
   ```

3. **src/parallel/mod.rs:1487** - マージ検証時のフォールバック
   ```rust
   let target_branch = self
       .workspace_manager
       .original_branch()
       .unwrap_or_else(|| "main".to_string());
   ```

## Solution

### 設計方針

**設定ファイルは追加しない** - 現在の `get_current_branch()` による動的取得は正しい設計です。問題は `unwrap_or("main")` によるフォールバック処理です。

### 修正方針

1. **`original_branch` の必須化**
   - `original_branch` は `create_worktree()` 時に必ず設定されるため、`None` の場合はエラーを返す
   - フォールバックではなく、初期化漏れとして扱う

2. **状態検出関数へのパラメータ追加**
   - `detect_workspace_state()` および関連関数に `base_branch: &str` パラメータを追加
   - 呼び出し側で `original_branch` を明示的に渡す

3. **テストコードの修正**
   - ハードコードされた `"main"` をテスト用の変数に置き換え

## Benefits

- **柔軟性**: 任意のブランチから実行可能（main, develop, master, feature/* など）
- **明示性**: ベースブランチが不明な場合は明確にエラーを返す
- **保守性**: フォールバックロジックが不要になり、コードが単純化される

## Risks

- `original_branch` が `None` となる新しいコードパスが追加された場合、コンパイルエラーではなく実行時エラーとなる
- ただし、現在の実装では `create_worktree()` で必ず設定されるため、リスクは低い

## Alternatives Considered

### ❌ 設定ファイルに `base_branch` オプションを追加

**却下理由**:
- 実際の現在ブランチと設定値が乖離する可能性がある
- ユーザーが設定を忘れた場合、意図しない動作となる
- 「動的取得」という正しい設計を複雑にする

### ✅ 採用: エラー返却とパラメータ明示化

**選択理由**:
- 最小限の変更で問題を解決できる
- コードの意図が明確になる
- 既存の動的取得メカニズムを活かせる

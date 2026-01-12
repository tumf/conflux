# Design: マージコミットを empty に保つ

## Context

jj (Jujutsu) では、マージコミットは pure merge であるべきで、コミット自体に変更を含むべきではありません。現在の実装では、`jj workspace update-stale` を使用しているため、working copy の変更がマージコミットに取り込まれてしまっています。

### 現在のフロー（問題あり）

```rust
// 1. マージコミット作成（empty）
jj new --no-edit base_rev rev1 rev2 -m "Merge parallel changes"

// 2. マージコミットに切り替え
jj edit --ignore-working-copy <merge_rev>

// 3. working copy をリフレッシュ（ここで問題発生！）
jj workspace update-stale  // ← working copy の変更がマージコミットに入る
```

### 問題の詳細

`jj workspace update-stale` は、stale な working copy を最新のコミットに同期しますが、その過程で **working copy の未コミット変更をスナップショット** します。これにより、マージコミットに意図しない変更が含まれてしまいます。

## Goals / Non-Goals

### Goals
- マージコミットを常に empty に保つ
- working copy の状態を適切に管理
- 並列実行後も正しい状態で作業を継続できるようにする
- jj の慣習に従った履歴を作成

### Non-Goals
- マージアルゴリズムの変更
- コンフリクト解決の方法の変更
- パフォーマンスの最適化（副次的な効果は歓迎）

## Decisions

### 決定 1: `workspace update-stale` を削除

**決定内容**: `jj workspace update-stale` の呼び出しを完全に削除する

**理由**:
- `update-stale` は working copy の変更をスナップショットするため、マージコミットが non-empty になる
- `jj edit --ignore-working-copy` を使用しているため、working copy は意図的にスナップショットされていない
- マージ直後に working copy をリフレッシュする必要性は低い

**代替案と却下理由**:
- `update-stale --ignore-working-copy` を使用 → そのようなオプションは存在しない
- マージ前に working copy をクリーン → 他の作業中の変更を失う可能性

### 決定 2: マージ後に新しい working copy コミットを作成

**決定内容**: マージコミット作成後、`jj new <merge_rev>` で新しい empty コミットを作成し、そこで作業を継続

**理由**:
- マージコミットと working copy を明確に分離
- jj の推奨パターンに従う（merge は empty、作業は新しいコミットで）
- working copy の状態が適切に管理される

**実装**:
```rust
// 1. マージコミット作成（empty）
jj new --no-edit base_rev rev1 rev2 -m "Merge parallel changes"

// 2. マージコミットに切り替え（--ignore-working-copy で working copy はそのまま）
jj edit --ignore-working-copy <merge_rev>

// 3. 新しい working copy コミットを作成
jj new <merge_rev>  // ← これで working copy が新しいコミットに移動

// Note: `update-stale` は不要
```

**代替案と却下理由**:
- `jj new --no-edit` でマージ後の empty コミットも作成 → 余分な empty コミットが増える
- マージコミットを `@` として残す → working copy とマージが混在してしまう

### 決定 3: `--ignore-working-copy` の使用を継続

**決定内容**: `jj edit --ignore-working-copy` の使用は継続する

**理由**:
- working copy のスナップショットを防ぐため
- マージコミットを純粋な merge として保つため
- 既存のロジックとの整合性

## Risks / Trade-offs

### リスク 1: 追加の `jj new` コマンド実行

- **リスク**: 1つの merge 操作に追加のコマンドが必要
- **軽減策**: `jj new` は高速な操作であり、オーバーヘッドは無視できる
- **受容**: 正確性を優先し、わずかなオーバーヘッドは許容

### リスク 2: working copy の状態変化

- **リスク**: `jj new` により working copy の状態が変わる可能性
- **軽減策**: `jj new` は現在の working copy の変更を新しいコミットに引き継ぐ
- **受容**: これは jj の標準動作であり、問題にはならない

### トレードオフ: コマンド数 vs 正確性

- **トレードオフ**: `update-stale` 削除により1コマンド減るが、`jj new` により1コマンド増える
- **結果**: コマンド数はほぼ同じだが、意味的に正しい操作になる
- **選択**: 正確性を優先

## Migration Plan

1. **Phase 1**: `src/vcs/jj/mod.rs` の `merge_jj_workspaces()` から `workspace update-stale` を削除
2. **Phase 2**: `jj new <merge_rev>` を追加して新しい working copy を作成
3. **Phase 3**: 既存のテストで動作確認
4. **Phase 4**: 並列実行の統合テストで empty merge を検証

各フェーズは順次実行し、動作確認後に次のフェーズへ進む。

## Open Questions

なし（要件は明確）

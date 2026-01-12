# pass-approved-changes-to-analyzer

## Why

現在の `ParallelizationAnalyzer` は、AIエージェントにすべての change ID をリスト形式で渡していますが、どれが選択済み（実行対象）かの情報が含まれていません。このため、AIエージェントは不要な change も分析してしまい、ファイルパスも推測する必要があります。選択済み change を明示的にマークし、ファイルパスを提供することで、分析の精度と効率を向上させます。

## What Changes

並列実行の依存関係分析時に、選択された change の実行優先順位を決めるため、AIエージェントに選択済み（`[x]` マーク）の change とそのディレクトリパスを明示的に渡す。

## 背景

現在の `ParallelizationAnalyzer::build_parallelization_prompt()` は、全ての change ID をリスト形式で渡しているが、どれが選択済み（承認済み `is_approved = true` **かつ** 選択 `selected = true`）かの情報が含まれていない。

```rust
// 現在の実装 (src/analyzer.rs:167-171)
let change_ids: String = changes
    .iter()
    .map(|c| format!("- {}", c.id))
    .collect::<Vec<_>>()
    .join("\n");
```

TUIでは3つの状態がある：
- `[ ]` = 未承認
- `[@]` = 承認済みだが未選択
- `[x]` = 承認済み**かつ**選択済み（実行対象）

このため、AIエージェントは：
1. どの change が実際に処理対象（`[x]`）かわからない
2. `openspec/changes/<change_id>/` というパス形式を推測する必要がある
3. 選択されていない change も分析してしまう可能性がある

## 目的

- 選択済み change を `[x]` マークで明示する
- 各 change のディレクトリパス（`openspec/changes/{id}/`）を提供する
- AIエージェントが依存関係を分析して実行優先順位（並列グループ）を決定できるようにする

## 提案する変更

### プロンプトフォーマットの改善

```rust
let change_info: String = changes
    .iter()
    .filter(|c| c.is_approved)  // 選択済みのみ
    .map(|c| {
        format!(
            "[x] {} (openspec/changes/{}/)",
            c.id, c.id
        )
    })
    .collect::<Vec<_>>()
    .join("\n");
```

出力例（選択済みのみ）：
```
[x] add-feature-a (openspec/changes/add-feature-a/)
[x] add-feature-b (openspec/changes/add-feature-b/)
```

### プロンプトの指示文も更新

```
Analyze these selected changes (marked with [x]) to determine execution order.
Read the proposal files in the specified directories to understand dependencies and group them for parallel execution.
```

## 期待される効果

1. **明確性の向上**: `[x]` マークで選択済みであることが明示的になる
2. **パス解決の簡素化**: ファイルパスが明示されるため推測が不要
3. **プロンプトの簡潔化**: 未選択の change は含まれないため、プロンプトが短くなる
4. **ユーザー体験の向上**: TUIで選択した change だけが分析されることが明確になる

## 影響範囲

- `src/analyzer.rs`: `build_parallelization_prompt()` メソッド
- テストコード: プロンプト生成のテストケース追加

## 代替案

1. **マーカーなしでパスのみ**: `[x]` マークを付けずにファイルパスだけを渡す
   - 利点: さらにシンプル
   - 欠点: チェックボックスの文脈が失われる

2. **JSON形式で渡す**: 構造化データとして渡す
   - 利点: パース可能
   - 欠点: プロンプトが読みにくくなる

**採用する理由**:
- `[x]` マークは視覚的にわかりやすく、TUIのチェックボックスとの一貫性がある
- 選択済みのみをフィルタすることで、プロンプトが簡潔になる

## Dependencies

なし

## 注意事項

### データモデルについて

- **TUI内部**: `ChangeState` には `is_approved`（承認済み）と `selected`（選択済み）の両方がある
  - `[ ]` = 未承認
  - `[@]` = 承認済みだが未選択
  - `[x]` = 承認済み**かつ**選択済み
  
- **Analyzer入力**: `Change` 構造体には `is_approved` フィールドのみ
  - 呼び出し元（TUI/CLI）が承認＋選択済みの change を `is_approved = true` として渡す
  - つまり、analyzer に渡される時点で既に実行対象のみがフィルタ済み

- 現在の呼び出し元（`ParallelRunService`）は既に選択済み change のみを渡しているため、`.filter(|c| c.is_approved)` は冗長だが、明示的にフィルタすることで意図が明確になる
- プロンプト内で未選択 change を表示する必要はない（分析対象外のため）

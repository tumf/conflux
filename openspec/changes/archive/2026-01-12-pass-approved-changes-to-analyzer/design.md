# Design Document: pass-approved-changes-to-analyzer

## 設計方針

プロンプトの可読性を保ちつつ、AIエージェントが必要な情報（選択状態、ファイルパス）を明確に理解できる形式にする。

## 実装の詳細

### 現在のプロンプト形式

```
Read the proposal files for these changes in openspec/changes/<change_id>/ and analyze their dependencies:

- change-a
- change-b
- change-c
```

**問題点**:
- `<change_id>` はプレースホルダーで、AIが推測する必要がある
- どれが選択済みか不明
- 具体的なディレクトリパスが不明確

### 新しいプロンプト形式

```
Analyze these selected changes (marked with [x]).
Read the proposal files in the specified directories to understand their dependencies:

[x] change-a (openspec/changes/change-a/)
[x] change-b (openspec/changes/change-b/)
```

**改善点**:
- `[x]` で選択済みを明示
- 各 change のディレクトリパスを提供
- 選択済みのみを渡すため、プロンプトが簡潔
- 未選択の change は含まれない

## コード変更

### Before: `src/analyzer.rs:166-202`

```rust
fn build_parallelization_prompt(&self, changes: &[Change]) -> String {
    let change_ids: String = changes
        .iter()
        .map(|c| format!("- {}", c.id))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"You are planning the execution order for OpenSpec changes.

Read the proposal files for these changes in openspec/changes/<change_id>/ and analyze their dependencies:

{change_ids}

Your task:
1. Read each change proposal.md to understand what it does
..."#
    )
}
```

### After

```rust
fn build_parallelization_prompt(&self, changes: &[Change]) -> String {
    let change_list: String = changes
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

    format!(
        r#"You are planning the execution order for OpenSpec changes.

Analyze these selected changes (marked with [x]).
Read the proposal files in the specified directories to understand their dependencies:

{change_list}

Your task:
1. Read each change's proposal.md at the given path to understand what it does
2. Identify dependencies between these changes
3. Group changes that can run in parallel (no dependencies on each other)
4. Order groups so dependencies are completed before dependents

Return ONLY valid JSON in this exact format:
{{
  "groups": [
    {{"id": 1, "changes": ["change-a", "change-b"], "depends_on": []}},
    {{"id": 2, "changes": ["change-c"], "depends_on": [1]}}
  ]
}}

Rules:
- Every change ID must appear exactly once
- Group IDs start at 1 and increment
- depends_on lists group IDs (not change IDs) that must complete first
- Groups with empty depends_on can start immediately
- Changes with no dependencies on each other should be in the same group
- Return valid JSON only, no markdown, no explanation"#
    )
}
```

## トレードオフ

### 選択肢1: 現在の提案（マーカー + パス）

**利点**:
- 視覚的にわかりやすい
- ファイルパスが明示的
- TUIのチェックボックスとの一貫性

**欠点**:
- なし

### 選択肢2: マーカーなしでパスのみ

```rust
let change_ids: String = changes
    .iter()
    .filter(|c| c.is_approved)
    .map(|c| format!("- {} (openspec/changes/{}/)", c.id, c.id))
    .collect::<Vec<_>>()
    .join("\n");
```

**利点**:
- よりシンプル

**欠点**:
- チェックボックスの文脈が失われる
- 視覚的な一貫性が低い

### 採用する選択肢

**選択肢1（`[x]` マーカー + パス + フィルタ）** を採用。理由：
1. `[x]` マークでTUIとの一貫性を保つ
2. 選択済みのみをフィルタしてプロンプトを簡潔に
3. デバッグ時に選択状態が明確

## テスト戦略

### 1. プロンプト生成のユニットテスト

```rust
#[test]
fn test_build_prompt_with_selected_markers() {
    let agent = AgentRunner::new(OrchestratorConfig::default());
    let analyzer = ParallelizationAnalyzer::new(agent);

    let changes = vec![
        Change {
            id: "selected-a".to_string(),
            is_approved: true,
            // ...
        },
        Change {
            id: "unselected-b".to_string(),
            is_approved: false,
            // ...
        },
    ];

    let prompt = analyzer.build_parallelization_prompt(&changes);

    assert!(prompt.contains("[x] selected-a (openspec/changes/selected-a/proposal.md)"));
    assert!(prompt.contains("[ ] unselected-b (openspec/changes/unselected-b/proposal.md)"));
    assert!(prompt.contains("Analyze ONLY the changes marked with [x]"));
}
```

### 2. 既存テストの互換性確認

既存の `analyze_groups` 関連テストが引き続き動作することを確認。

## 後方互換性

- プロンプトフォーマットの変更のみ
- AIエージェントのレスポンス形式は変更なし
- 呼び出し側のインターフェースは変更なし

## セキュリティ考慮事項

なし（プロンプト文字列の変更のみ）

## パフォーマンス考慮事項

- プロンプト生成コストは微増（`format!` 呼び出しが1行増える程度）
- 実行時のパフォーマンスへの影響は無視できる

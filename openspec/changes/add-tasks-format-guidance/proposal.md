# Proposal: AIエージェントプロンプトにtasks.mdフォーマット修正ガイダンスを追加

## 概要

AIエージェントが不正なtasks.mdフォーマット（チェックボックスなし）を自動的に修正できるよう、`src/agent.rs`の`APPLY_SYSTEM_PROMPT`にフォーマット修正方法を追加する。

## 背景

現在、tasks.mdが不正なフォーマット（チェックボックスなし）の場合、パーサーが0/0タスクを返しapplyが失敗する。

**不正なフォーマット例:**
```markdown
## 1. Task title
説明...

- Task without checkbox
1. Numbered task
```

**正しいフォーマット:**
```markdown
- [ ] 1. Task title
説明...

- [ ] Task without checkbox
1. [ ] Numbered task
```

### 現在の問題
- `fix-parallel-merge-completed-status`のtasks.mdが不正なフォーマットで0/0タスク検出
- Sequential/Parallel両方のapplyモードで同じ問題が発生しうる
- エラーメッセージだけでは修正方法が不明

## 提案する解決策

`src/agent.rs`の`APPLY_SYSTEM_PROMPT`定数に、tasks.mdフォーマット修正ガイダンスを追加する。

### 追加する内容（案）

```rust
Tasks format requirements:
- All tasks MUST have checkboxes: `- [ ]` or `- [x]`
- Invalid formats that need fixing:
  * `## N. Task` → Convert to `- [ ] N. Task`
  * `- Task` → Convert to `- [ ] Task`
  * `1. Task` → Convert to `1. [ ] Task`
- If you encounter 0/0 tasks detected, check and fix tasks.md format first
```

### 動作フロー

#### Sequential apply
```
apply実行
  ↓
0/0タスク検出（parse_file()が失敗）
  ↓
AIエージェント起動
  ↓
プロンプトのガイダンスに従ってtasks.md修正
  ↓
再パース成功 → apply継続
```

#### Parallel run
```
並列実行開始
  ↓
各changeでapply実行
  ↓
0/0タスク検出
  ↓
AIエージェントがガイダンスに従って自動修正
  ↓
apply継続
```

## 影響範囲

### 変更対象
- `src/agent.rs` - `APPLY_SYSTEM_PROMPT`定数のみ

### 影響しない箇所
- `src/task_parser.rs` - パーサーロジックは変更不要
- `src/execution/apply.rs` - apply実行ロジックは変更不要
- `src/parallel/executor.rs` - 並列実行ロジックは変更不要

### 後方互換性
- ✅ 既存の動作に影響なし（プロンプトにガイダンスを追加するのみ）
- ✅ 設定ファイル変更不要
- ✅ 既存のtests.mdファイルに影響なし

## 代替案と比較

| アプローチ | メリット | デメリット |
|-----------|---------|-----------|
| **プロンプト追加（提案）** | ・実装が簡単<br>・既存コード変更なし<br>・両モード対応 | ・AIの解釈に依存 |
| 自動修正コード実装 | ・確実に修正<br>・AIに依存しない | ・複雑な実装<br>・エッジケース対応 |
| 検証コマンド追加 | ・事前検出可能 | ・手動実行が必要<br>・自動修正なし |

## 成功基準

1. ✅ `src/agent.rs`にフォーマットガイダンスが追加されている
2. ✅ 不正なtasks.mdを持つchangeでapplyを実行すると、AIエージェントが自動修正する
3. ✅ Sequential/Parallel両モードで動作する
4. ✅ 既存のテストが全て通る

## リスクと軽減策

### リスク
- AIエージェントがガイダンスを無視する可能性
- 複雑なフォーマット（ネストリスト、コードブロック内）で誤修正

### 軽減策
- ガイダンスを明確かつ具体的に記述
- 将来的に検証コマンド（`validate`）の追加も検討可能

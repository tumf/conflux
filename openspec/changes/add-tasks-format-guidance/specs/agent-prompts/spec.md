# Capability: Agent Prompts

AIエージェントに渡すシステムプロンプトの内容を定義する。

---

## MODIFIED Requirements

### Requirement: Apply system prompt MUST include task format guidance

AIエージェントのapplyプロンプト（`APPLY_SYSTEM_PROMPT`）には、tasks.mdフォーマット修正方法のガイダンスが含まれなければならない（MUST）。

#### Rationale
不正なフォーマット（チェックボックスなし）のtasks.mdが原因で0/0タスク検出エラーが発生した場合、AIエージェントが自動的に修正できるようにする。

#### Scenario: AIエージェントが不正フォーマットを修正する

**Given:**
- tasks.mdに不正なフォーマット（`## 1. Task`, `- Task`, `1. Task`）が含まれる
- パーサーが0/0タスクを検出してapplyが実行される

**When:**
- AIエージェントがapplyプロンプトを受け取る

**Then:**
- プロンプトにtasks.mdフォーマット要件が含まれている
  - チェックボックス必須（`- [ ]`, `- [x]`）
  - 不正フォーマットのパターン例
  - 各パターンの修正方法
  - 0/0検出時の対応手順
- AIエージェントがガイダンスに従ってtasks.mdを修正する
- 修正後、再パースで正しいタスク数が検出される

**Verification:**
```bash
# 1. テスト用changeを作成（不正フォーマット）
mkdir -p openspec/changes/test-invalid-format
cat > openspec/changes/test-invalid-format/tasks.md <<EOF
# Tasks
## 1. First task
- Second task without checkbox
1. Third numbered task
EOF

# 2. Apply実行（AIが自動修正するはず）
cargo run -- apply test-invalid-format

# 3. 修正後のフォーマット確認
cat openspec/changes/test-invalid-format/tasks.md
# 期待: チェックボックスが追加されている
```

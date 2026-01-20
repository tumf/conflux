## Context
acceptance は CONTINUE のたびに網羅的チェックを再実行するため、既に確認済みの事項を再検証してしまう。1回目の網羅的チェック結果を前提に、2回目以降は前回チェック時点からの差分と前回の指摘事項に集中させ、検証コストを抑えつつ修正確認の精度を高める。

## Goals / Non-Goals
- Goals:
  - 1回目は網羅的チェックを維持する
  - 2回目以降は更新ファイル一覧 + 前回FINDINGSの修正確認に限定する
  - 差分の本文は渡さず、必要に応じてAIが対象ファイルを取得する
- Non-Goals:
  - 1回目の網羅性を自動的に検証する仕組みは追加しない
  - 変更検知に git diff 以外の方式を追加しない

## Decisions
- Decision: acceptance の履歴に前回チェック時点のコミット識別子を保存し、2回目以降のプロンプトに更新ファイル一覧と前回FINDINGSを挿入する。
- Decision: 更新ファイル一覧は `git diff --name-only <previous>..HEAD` 相当のみを生成し、diff本文は渡さない。
- Decision: 2回目以降の acceptance プロンプトには「必要に応じて関連ファイルを読んで確認する」旨を明記する。

## Risks / Trade-offs
- 1回目の見落としは2回目以降で補えないが、要件として1回目網羅を前提とする。
- 更新ファイル一覧のみでは判断が難しい場合があるため、ファイル内容の取得を許容する指示を追加する。

## Implementation Flow

### 1回目の acceptance（attempt = 1）
- コミット識別子を記録（git rev-parse HEAD）
- 網羅的チェックを実行（現行の ACCEPTANCE_SYSTEM_PROMPT を使用）
- AcceptanceAttempt に commit_hash フィールドを追加して保存

### 2回目以降の acceptance（attempt >= 2）
- 前回の commit_hash と現在の HEAD で git diff --name-only を実行
- 更新ファイル一覧と前回FINDINGSをプロンプトに追加
- プロンプト構成:
  - ACCEPTANCE_SYSTEM_PROMPT（既存の網羅的チェック指示）
  - 差分コンテキスト（新規追加）:
    ```
    <acceptance_diff_context>
    Files changed since last acceptance check:
    - file1.rs
    - file2.rs
    ...

    Previous acceptance findings:
    - Finding 1
    - Finding 2
    ...

    Focus your verification on:
    1. Whether the changed files address the previous findings
    2. Whether the changes introduce new issues
    3. Read relevant files if needed to confirm the fixes
    </acceptance_diff_context>
    ```
  - user_prompt（設定値）
  - history_context（既存の履歴コンテキスト）

### 分岐条件
- `AcceptanceHistory::count(change_id) == 0` → 1回目の acceptance
- `AcceptanceHistory::count(change_id) >= 1` → 2回目以降の acceptance（差分チェック）

## Open Questions
- なし

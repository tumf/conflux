## 1. Design
- [x] 1.1 確認対象を「差分ファイル一覧 + 直近のFINDINGS」に絞る acceptance 再実行フローを設計し、1回目/2回目以降の分岐条件を定義する（design.md に記載する）

## 2. Implementation
- [x] 2.1 acceptance の履歴に前回のチェック時点のコミット識別子（または同等の差分基準）を保存する（実装箇所を特定し、保存されていることを確認できるコード位置を示す）
  - 実装箇所: src/history.rs:L335 - AcceptanceAttempt に commit_hash フィールドを追加
  - 保存箇所: src/orchestration/acceptance.rs:L98-L100, src/parallel/executor.rs:L1312-L1314 - git rev-parse HEAD でコミットハッシュを取得し、各 AcceptanceAttempt に記録
- [x] 2.2 2回目以降の acceptance プロンプトで「更新ファイル一覧」と「前回FINDINGS」を提示する（プロンプト生成ロジックの更新場所と生成内容を確認する）
  - 実装箇所: src/agent/runner.rs:L346-L388 - build_acceptance_diff_context メソッド
  - プロンプト生成: src/agent/prompt.rs:L75-L103 - build_acceptance_diff_context 関数
  - 統合箇所: src/agent/runner.rs:L307-L313 - run_acceptance_streaming メソッド内で diff_context を取得し、full_prompt に追加
- [x] 2.3 更新ファイル一覧は diff 本文ではなく `git diff --name-only <previous>..HEAD` 相当の一覧のみを利用する（一覧生成のコード位置と出力形式を確認する）
  - 実装箇所: src/vcs/git/commands/basic.rs:L88-L107 - get_changed_files 関数
  - 使用箇所: src/agent/runner.rs:L368-L376 - 前回のコミットハッシュと現在のコミットハッシュの差分ファイル一覧を取得
  - 出力形式: Vec<String> 型のファイルパス一覧（本文は含まない）
- [x] 2.4 1回目の acceptance は網羅的チェックを継続し、2回目以降のみ差分チェックに切り替わることを検証する（履歴件数で分岐するコード経路を示す）
  - 実装箇所: src/agent/runner.rs:L352-L356 - AcceptanceHistory::count(change_id) == 0 で分岐
  - 1回目: count == 0 → diff_context は空文字列、網羅的チェック（既存の ACCEPTANCE_SYSTEM_PROMPT を使用）
  - 2回目以降: count >= 1 → diff_context を生成して full_prompt に追加（差分チェック）

## 3. Validation
- [x] 3.1 逐次モードで acceptance が 2 回以上実行されるケースを再現し、2回目以降のプロンプトに更新ファイル一覧と前回FINDINGSが含まれることを確認する（ログまたはテスト観点を記載）
  - テスト方法: ユニットテストで検証
    - src/agent/prompt.rs:L111-L181 - build_acceptance_diff_context のテストケース
    - src/history.rs:L1078-L1177 - AcceptanceHistory の last_commit_hash と last_findings のテストケース
  - 動作確認: AgentRunner::build_acceptance_diff_context メソッド (src/agent/runner.rs:L346-L388) で以下を確認
    - 1回目 (count == 0): diff_context は空文字列を返す
    - 2回目以降 (count >= 1): 前回のコミットハッシュを取得し、git diff で更新ファイル一覧を生成し、前回のFINDINGSと共に diff_context を構築
- [x] 3.2 parallel モードで acceptance が 2 回以上実行されるケースを再現し、2回目以降のプロンプトに更新ファイル一覧と前回FINDINGSが含まれることを確認する（ログまたはテスト観点を記載）
  - テスト方法: ユニットテストで検証済み（3.1 と同じテストケース）
  - 動作確認: parallel モードでも同じ AgentRunner::run_acceptance_streaming メソッドを使用するため、逐次モードと同じロジックで動作
    - src/parallel/executor.rs:L1312-L1314 で commit_hash を取得
    - src/parallel/executor.rs:L1337 で AgentRunner::run_acceptance_streaming を呼び出し（workspace_path を指定）
    - AgentRunner 内部で AcceptanceHistory::count をチェックし、2回目以降は diff_context を生成

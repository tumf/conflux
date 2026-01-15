# 設計ドキュメント: ループコンテキスト履歴

## アーキテクチャ概要

現在の `ApplyHistory` の設計を拡張して、archive と resolve にも同様の履歴機能を提供します。

## データ構造

### ArchiveAttempt

```rust
pub struct ArchiveAttempt {
    /// 試行回数（1-based）
    pub attempt: u32,
    /// 成功したかどうか
    pub success: bool,
    /// 所要時間
    pub duration: Duration,
    /// エラーメッセージ（失敗時）
    pub error: Option<String>,
    /// 検証結果（NotArchived の場合の理由）
    pub verification_result: Option<String>,
    /// Exit code
    pub exit_code: Option<i32>,
}
```

### ArchiveHistory

```rust
pub struct ArchiveHistory {
    /// change_id ごとの試行履歴
    attempts: HashMap<String, Vec<ArchiveAttempt>>,
}

impl ArchiveHistory {
    pub fn new() -> Self;
    pub fn record(&mut self, change_id: &str, attempt: ArchiveAttempt);
    pub fn get(&self, change_id: &str) -> Option<&[ArchiveAttempt]>;
    pub fn count(&self, change_id: &str) -> u32;
    pub fn clear(&mut self, change_id: &str);
    pub fn format_context(&self, change_id: &str) -> String;
}
```

### ResolveAttempt

```rust
pub struct ResolveAttempt {
    /// 試行回数（1-based）
    pub attempt: u32,
    /// コマンドが成功終了したか
    pub command_success: bool,
    /// 検証が成功したか
    pub verification_success: bool,
    /// 所要時間
    pub duration: Duration,
    /// 検証失敗の理由（具体的な継続理由）
    pub continuation_reason: Option<String>,
    /// Exit code
    pub exit_code: Option<i32>,
}
```

### ResolveContext

```rust
pub struct ResolveContext {
    /// 現在のセッション内での試行履歴
    attempts: Vec<ResolveAttempt>,
    /// 最大試行回数
    max_retries: u32,
}

impl ResolveContext {
    pub fn new(max_retries: u32) -> Self;
    pub fn record(&mut self, attempt: ResolveAttempt);
    pub fn current_attempt(&self) -> u32;
    pub fn format_continuation_context(&self) -> String;
}
```

## プロンプト構築

### Archive プロンプト

```
[user_prompt]

[ARCHIVE_SYSTEM_PROMPT - 将来追加される場合]

<last_archive attempt="1">
status: failed
duration: 5s
verification_result: Change still exists at openspec/changes/my-change
error: Archive command succeeded but change was not actually archived
exit_code: 0
</last_archive>

<last_archive attempt="2">
status: failed
duration: 6s
verification_result: Change still exists at openspec/changes/my-change
exit_code: 0
</last_archive>
```

### Resolve プロンプト

```
[base_prompt]

<resolve_context>
This is attempt 2 of 3 for conflict resolution.

Previous attempt (1):
- Command exit: success (code: 0)
- Verification: failed
- Reason: Conflicts still present after resolution attempt: src/main.rs, src/lib.rs
- Duration: 45s

Continue resolving the conflicts. The previous attempt did not fully resolve all conflicts.
</resolve_context>

[vcs_status and other context]
```

## 実装の流れ

### Archive の場合

1. `AgentRunner::run_archive_streaming()` 呼び出し
2. archive 履歴コンテキストを取得: `self.archive_history.format_context(change_id)`
3. プロンプト構築: `build_archive_prompt(user_prompt, history_context)`
4. コマンド実行
5. 結果記録: `self.record_archive_attempt(change_id, status, start, verification_result)`

### Resolve の場合

1. `resolve_conflicts_with_retry()` または `resolve_merges_with_retry()` 開始
2. `ResolveContext::new(max_retries)` 作成
3. ループ開始（attempt 1 から max_retries まで）
4. プロンプト構築時に `context.format_continuation_context()` を含める
5. コマンド実行
6. 検証実施
7. 検証失敗の場合、理由を記録: `context.record(ResolveAttempt { continuation_reason: Some(reason), ... })`
8. ループ継続

## エラーハンドリング

- archive 履歴の記録失敗は警告ログのみ（処理は継続）
- resolve コンテキストの記録失敗も警告ログのみ
- 履歴フォーマットの生成失敗時は空文字列を返す

## 互換性

- 既存の `ApplyHistory` の動作は変更なし
- archive と resolve の既存の動作に影響なし（履歴が追加されるのみ）
- 設定ファイルの変更は不要

## パフォーマンス

- 履歴はメモリ内に保持（ファイル I/O なし）
- change ごとに独立して管理
- archive/change 完了時にクリアされるため、メモリリークなし

## 制限事項

- 履歴はプロセス内でのみ保持（再起動すると失われる）
- 並列実行時は各ワークスペースで独立した履歴を持つ
- resolve コンテキストはループセッション内でのみ有効（関数スコープ）

## 将来の拡張

- archive にも system prompt を追加する場合、`build_archive_prompt()` が対応可能
- 履歴の永続化（オプション機能として）
- 履歴の最大保持数制限（メモリ節約）

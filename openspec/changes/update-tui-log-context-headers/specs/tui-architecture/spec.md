## MODIFIED Requirements

### Requirement: Log Entry Structure and Display
TUIのログエントリーは、タイムスタンプ、メッセージ、色に加えて、オプションのコンテキスト情報（change ID、オペレーション、イテレーション番号）を含まなければならない（SHALL）。
ログヘッダーは、利用可能なコンテキスト情報に基づいて段階的に表示される。

**構造体定義**:
```rust
pub struct LogEntry {
    pub timestamp: String,      // タイムスタンプ（HH:MM:SS形式）
    pub message: String,        // ログメッセージ
    pub color: Color,           // ログレベル色
    pub change_id: Option<String>,    // 変更ID
    pub operation: Option<String>,    // オペレーション ("apply", "archive", "resolve", "analysis", "ensure_archive_commit")
    pub iteration: Option<u32>,       // イテレーション番号（apply/archive/resolve/analysis）
}
```

**ビルダーメソッド**:
- `with_change_id(change_id: impl Into<String>)` - 変更IDを設定
- `with_operation(operation: impl Into<String>)` - オペレーションを設定
- `with_iteration(iteration: u32)` - イテレーション番号を設定

#### Scenario: ログヘッダーにオペレーションとイテレーションが表示される
- **GIVEN** ログエントリーが `change_id="test-change"`, `operation="archive"`, `iteration=2` で作成される
- **WHEN** TUIがログをレンダリングする
- **THEN** ログヘッダーは `[test-change:archive:2]` と表示される
- **AND** ヘッダーの後にメッセージが続く

#### Scenario: ensure_archive_commitのログが区別される
- **GIVEN** ログエントリーが `change_id="test-change"`, `operation="ensure_archive_commit"`, `iteration=1` で作成される
- **WHEN** TUIがログをレンダリングする
- **THEN** ログヘッダーは `[test-change:ensure_archive_commit:1]` と表示される
- **AND** archiveログと区別できる

#### Scenario: analysisログがイテレーション付きで表示される
- **GIVEN** ログエントリーが `change_id=None`, `operation="analysis"`, `iteration=3` で作成される
- **WHEN** TUIがログをレンダリングする
- **THEN** ログヘッダーは `[analysis:3]` と表示される
- **AND** 解析の再実行が区別できる

#### Scenario: resolveログがイテレーション付きで表示される
- **GIVEN** ログエントリーが `change_id=None`, `operation="resolve"`, `iteration=2` で作成される
- **WHEN** TUIがログをレンダリングする
- **THEN** ログヘッダーは `[resolve:2]` と表示される
- **AND** 解決の再実行が区別できる

#### Scenario: ログヘッダーに変更IDのみが表示される（後方互換性）
- **GIVEN** ログエントリーが `change_id="test-change"`, `operation=None`, `iteration=None` で作成される
- **WHEN** TUIがログをレンダリングする
- **THEN** ログヘッダーは `[test-change]` と表示される
- **AND** 既存の動作が維持される

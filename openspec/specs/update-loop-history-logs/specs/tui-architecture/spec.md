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

#### Scenario: resolveログがイテレーション付きで表示される
- **GIVEN** ログエントリーが `change_id=None`, `operation="resolve"`, `iteration=2` で作成される
- **WHEN** TUIがログをレンダリングする
- **THEN** ログヘッダーは `[resolve:2]` と表示される
- **AND** 解決の再実行が区別できる

#### Scenario: analysisログがイテレーション付きで表示される
- **GIVEN** ログエントリーが `change_id=None`, `operation="analysis"`, `iteration=3` で作成される
- **WHEN** TUIがログをレンダリングする
- **THEN** ログヘッダーは `[analysis:3]` と表示される
- **AND** 解析の再実行が区別できる

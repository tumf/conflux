# tui-architecture Specification Delta

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
    pub operation: Option<String>,    // オペレーション ("apply", "archive", "resolve")
    pub iteration: Option<u32>,       // イテレーション番号（applyのみ）
}
```

**ビルダーメソッド**:
- `with_change_id(change_id: impl Into<String>)` - 変更IDを設定
- `with_operation(operation: impl Into<String>)` - オペレーションを設定
- `with_iteration(iteration: u32)` - イテレーション番号を設定

#### Scenario: ログヘッダーにオペレーションとイテレーションが表示される

- **GIVEN** ログエントリーが `change_id="test-change"`, `operation="apply"`, `iteration=1` で作成される
- **WHEN** TUIがログをレンダリングする
- **THEN** ログヘッダーは `[test-change:apply:1]` と表示される
- **AND** ヘッダーの後にメッセージが続く

#### Scenario: ログヘッダーにオペレーションのみが表示される

- **GIVEN** ログエントリーが `change_id="test-change"`, `operation="archive"`, `iteration=None` で作成される
- **WHEN** TUIがログをレンダリングする
- **THEN** ログヘッダーは `[test-change:archive]` と表示される
- **AND** イテレーション番号は表示されない

#### Scenario: ログヘッダーに変更IDのみが表示される（後方互換性）

- **GIVEN** ログエントリーが `change_id="test-change"`, `operation=None`, `iteration=None` で作成される
- **WHEN** TUIがログをレンダリングする
- **THEN** ログヘッダーは `[test-change]` と表示される（従来の形式）
- **AND** 既存の動作が維持される

#### Scenario: ログヘッダーが表示されない

- **GIVEN** ログエントリーが `change_id=None` で作成される
- **WHEN** TUIがログをレンダリングする
- **THEN** ヘッダーは表示されず、タイムスタンプとメッセージのみが表示される

#### Scenario: ビルダーメソッドでコンテキスト情報を設定できる

- **GIVEN** LogEntry::info("message") でログエントリーが作成される
- **WHEN** `.with_change_id("test")`, `.with_operation("apply")`, `.with_iteration(2)` を連鎖呼び出しする
- **THEN** ログエントリーの各フィールドが正しく設定される
- **AND** ヘッダーは `[test:apply:2]` と表示される

#### Scenario: 並列実行時のapplyログにイテレーション番号が含まれる

- **GIVEN** 並列実行モードでapply操作が実行される
- **WHEN** イテレーション1のapplyログが生成される
- **THEN** ログヘッダーは `[change_id:apply:1]` 形式で表示される
- **AND** 複数回のイテレーションが区別できる

#### Scenario: archive操作のログにオペレーションタイプが含まれる

- **GIVEN** 変更のarchive操作が実行される
- **WHEN** archiveログが生成される
- **THEN** ログヘッダーは `[change_id:archive]` 形式で表示される
- **AND** apply操作と区別できる

#### Scenario: resolve操作のログにオペレーションタイプが含まれる

- **GIVEN** コンフリクト解決操作が実行される
- **WHEN** resolveログが生成される
- **THEN** ログヘッダーは `[change_id:resolve]` 形式で表示される
- **AND** 他の操作と区別できる

#### Scenario: ログヘッダーの表示幅が適切に計算される

- **GIVEN** より長いログヘッダー形式 `[change_id:operation:iteration]` が使用される
- **WHEN** TUIがメッセージの利用可能幅を計算する
- **THEN** ヘッダー全体の長さが考慮される
- **AND** メッセージが適切に切り詰められる
- **AND** ターミナル幅を超えない

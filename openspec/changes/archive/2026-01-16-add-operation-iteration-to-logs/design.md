# Design: TUIログヘッダーにオペレーションタイプとイテレーション番号を追加

## 概要

TUIログヘッダーを拡張して、オペレーション（apply/archive/resolve）とイテレーション番号を表示することで、ログの文脈を明確化する。

## アーキテクチャ決定

### 1. LogEntry構造体の拡張（データモデル）

```rust
pub struct LogEntry {
    pub timestamp: String,
    pub message: String,
    pub color: Color,
    pub change_id: Option<String>,
    pub operation: Option<String>,  // 新規追加
    pub iteration: Option<u32>,     // 新規追加
}
```

**理由**: 既存の構造体に非破壊的にフィールドを追加することで、後方互換性を維持しながら段階的に移行できる。

### 2. ビルダーパターンの採用

```rust
impl LogEntry {
    pub fn with_operation(mut self, operation: impl Into<String>) -> Self {
        self.operation = Some(operation.into());
        self
    }

    pub fn with_iteration(mut self, iteration: u32) -> Self {
        self.iteration = Some(iteration.into());
        self
    }
}
```

**理由**: 既存のコードを最小限の変更で更新できる。メソッドチェーンで自然に記述できる。

### 3. 段階的な表示形式

| 条件 | 表示形式 | 例 |
|------|----------|-----|
| operation, iterationあり | `[change_id:operation:iteration]` | `[test:apply:1]` |
| operationのみあり | `[change_id:operation]` | `[test:archive]` |
| change_idのみ | `[change_id]` | `[test]` |
| いずれもなし | なし | （従来通り） |

**理由**: 柔軟性を保ちつつ、情報がある場合は最大限表示する。

### 4. レンダリングロジックの更新

`src/tui/render.rs` の `render_logs()` 関数内で、ヘッダー文字列の構築ロジックを拡張:

```rust
let prefix = if let Some(ref change_id) = entry.change_id {
    match (&entry.operation, entry.iteration) {
        (Some(op), Some(iter)) => format!("[{}:{}:{}]", change_id, op, iter),
        (Some(op), None) => format!("[{}:{}]", change_id, op),
        (None, _) => format!("[{}]", change_id),
    }
} else {
    String::new()
};
```

**理由**: パターンマッチで明確に条件分岐。可読性が高く、将来の拡張も容易。

### 5. オペレーション情報の伝播

各実行パスでLogEntryを生成する際に、適切な文脈情報を設定:

- **Apply操作**: `src/parallel/executor.rs` の `execute_apply_with_retry()` で `iteration` 変数を使用
- **Archive操作**: `src/parallel/mod.rs` のarchive関連イベントで `"archive"` を設定
- **Resolve操作**: `src/parallel/conflict.rs` のresolve関連イベントで `"resolve"` を設定

**理由**: 情報が生成される最も近い場所で設定することで、正確性を保証。

## 代替案と却下理由

### 却下案1: メッセージ文字列にオペレーション情報を埋め込む

```rust
let message = format!("apply:{}: Starting iteration", iteration);
```

**却下理由**: 表示形式が統一されず、パースが困難。構造化データとして扱えない。

### 却下案2: 新しい`EnhancedLogEntry`型を作成

```rust
pub struct EnhancedLogEntry {
    base: LogEntry,
    operation: Option<String>,
    iteration: Option<u32>,
}
```

**却下理由**: 既存のコードへの変更範囲が大きい。型変換が必要でコードが複雑化。

## 実装上の考慮事項

### 表示幅の計算

より長いヘッダーに対応するため、利用可能な幅の計算を調整:

```rust
// 現在: change_id.len() + 3  // "[" + "]" + " "
// 更新後: prefix.len() + 1    // prefix全体 + " "
```

### 後方互換性の保証

- 既存のLogEntryは`operation`と`iteration`が`None`で動作する
- 既存のログ生成箇所は段階的に更新可能
- テストは既存の動作を破壊しないことを確認

### パフォーマンス影響

- フィールド追加: メモリオーバーヘッドは最小（数バイト/エントリー）
- 文字列構築: ヘッダー表示時のみ、ログエントリー作成時ではない
- 影響: 無視できるレベル

## セキュリティ考慮事項

- オペレーション名は固定文字列（"apply", "archive", "resolve"）のため、インジェクションリスクなし
- イテレーション番号は数値のため、フォーマット攻撃のリスクなし

## テスト戦略

1. **単体テスト**:
   - ビルダーメソッドの動作確認
   - ヘッダー文字列の生成ロジック確認

2. **統合テスト**:
   - TUIレンダリングでの表示確認
   - 並列実行での動作確認

3. **回帰テスト**:
   - 既存のテストがすべてパスすることを確認

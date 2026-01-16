# Change: TUIログヘッダーにオペレーションタイプとイテレーション番号を追加

## Why

現在のTUIログ表示では、ログヘッダーが`[change_id]`の形式で表示されるため、以下の課題がある:

- どのオペレーション（apply, archive, resolve）のログなのか不明
- 複数回のapplyイテレーションがある場合、どのイテレーションのログか判別できない
- 並列実行時に複数の変更が同時に処理される際、ログの文脈を追跡しにくい

ログヘッダーを`[change_id:operation:iteration]`の形式に拡張することで、ログの文脈を明確化し、デバッグやモニタリングを改善する。

## What Changes

**Non-Breaking Enhancement:**

- **ログヘッダー形式**: `[change_id]` → `[change_id:operation:iteration]`
  - 例: `[rename-to-conflux]` → `[rename-to-conflux:apply:1]`
  - `operation`: `apply`, `archive`, `resolve`のいずれか
  - `iteration`: イテレーション番号（オプショナル、applyのみ）

- **LogEntry構造体の拡張**:
  - `operation: Option<String>` フィールドを追加
  - `iteration: Option<u32>` フィールドを追加
  - 既存の`change_id`フィールドと組み合わせて使用

- **ビルダーメソッドの追加**:
  - `with_operation(operation: impl Into<String>)` メソッド
  - `with_iteration(iteration: u32)` メソッド

## Impact

- Affected specs:
  - `tui-architecture` - LogEntry構造とログ表示要件
- Affected code:
  - `src/events.rs` - LogEntry構造体の定義
  - `src/tui/render.rs` - ログヘッダーのレンダリングロジック
  - `src/parallel/executor.rs` - applyログ生成箇所
  - `src/parallel/mod.rs` - archive/resolveログ生成箇所
  - `src/tui/orchestrator.rs` - シリアルモードログ生成箇所
- 後方互換性: 既存のLogEntryは`operation`と`iteration`が`None`で表示が変わらない

## Implementation Notes

- オペレーション情報がない場合は従来通り`[change_id]`形式で表示
- イテレーション情報がない場合は`[change_id:operation]`形式で表示
- 表示フォーマット例:
  - `[test-change:apply:1]` - apply操作のイテレーション1
  - `[test-change:archive]` - archive操作（イテレーションなし）
  - `[test-change]` - オペレーション情報なし（後方互換）

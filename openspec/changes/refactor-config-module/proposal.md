# Change: 設定モジュールの責務分離

## Why

`config.rs` は 767 行あり、以下の責務が混在している：

1. `OrchestratorConfig` 構造体定義
2. JSONC パーサー（コメント除去、trailing comma 処理）
3. 設定ファイルの読み込みとマージロジック
4. プレースホルダー展開ロジック（`$CHANGE_ID` など）
5. デフォルト値の定義
6. 多数のテスト（30 個）

JSONC パーサーは汎用性が高く、他のモジュールでも使える可能性があるが、
config モジュールに埋もれている状態。

## What Changes

- `config.rs` を責務ごとに分割
  - `config/mod.rs` - OrchestratorConfig 構造体と読み込み
  - `config/defaults.rs` - デフォルト値定数
  - `config/expand.rs` - プレースホルダー展開
  - `config/jsonc.rs` - JSONC パーサー（汎用）
- JSONC パーサーを他のモジュールからも利用可能に

## Impact

- 対象 specs: `code-maintenance`
- 対象コード:
  - `src/config.rs` → `src/config/` ディレクトリに分割
  - 将来的に他のモジュールから `config::jsonc::parse()` を利用可能

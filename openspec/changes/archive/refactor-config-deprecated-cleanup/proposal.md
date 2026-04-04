# Change: 設定パス解決の非推奨経路を整理する

## Why
`src/config/mod.rs` には設定パス解決のための旧APIと新APIが併存しており、同じ責務に対する入口が複数あります。`get_xdg_config_path()` と `get_global_config_path()` はすでに非推奨化されている一方で、テストや公開面に痕跡が残っており、設定読込ロジックの保守コストと誤用リスクを高めています。

## What Changes
- 設定ファイル探索の責務を現行優先順位ベースのAPIへ寄せ、非推奨ヘルパの扱いを明確化する
- 旧APIに依存するテストを、現行の優先順位仕様を直接検証するキャラクタリゼーションテストへ置き換える
- 設定読込の公開挙動は維持しつつ、内部実装での経路重複を減らす

## Evidence
- `src/config/mod.rs:33` `get_xdg_env_config_path()`
- `src/config/mod.rs:49` `get_xdg_default_config_path()`
- `src/config/mod.rs:68` 非推奨 `get_xdg_config_path()`
- `src/config/mod.rs:92` 非推奨 `get_global_config_path()`
- `src/config/mod.rs:1037` 非推奨APIを直接検証するテストが残っている

## Impact
- Affected specs: `code-maintenance`, `configuration`
- Affected code: `src/config/mod.rs`, 関連テスト
- API/CLI互換性: 変更なし

## Acceptance Criteria
- 設定ファイル探索順序（カスタム指定 → プロジェクト設定 → XDG環境変数 → XDGデフォルト → platform default → デフォルト値）が回帰しない
- CLI引数、設定ファイル形式、既存ユーザー向けの挙動に変更がない
- 設定関連テストが成功し、非推奨経路の内部依存が縮小されている

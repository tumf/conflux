# Change: Dummy 仕様を削除

## Why

`openspec/specs/dummy/` はテスト用のプレースホルダー仕様であり、本番用途がない。仕様一覧をクリーンに保ち、実際の機能仕様のみを管理するために削除する。

## What Changes

- `openspec/specs/dummy/` ディレクトリとその内容を削除
- `openspec list --specs` の出力から dummy を除外

## Impact

- Affected specs: dummy（削除）
- Affected code: なし

# Change: changes間のspec delta衝突検出コマンドを追加

## Why
複数の変更提案が同じspec deltaに対して矛盾する編集を行う場合、実装前に検知できないと並列開発で手戻りが発生します。LLMを使わずに衝突を検出できるCLIコマンドを提供し、レビュー前の早期チェックを可能にします。

## What Changes
- changes間のspec delta衝突を検出するCLIコマンドを追加する
- 衝突内容を人間向けとJSONの両形式で出力できるようにする
- 衝突が見つかった場合の終了コードを明確に定義する

## Impact
- Affected specs: cli
- Affected code: CLIコマンド定義、spec delta解析ロジック

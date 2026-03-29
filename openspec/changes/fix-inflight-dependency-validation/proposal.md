# Change: in-flight change への依存参照がバリデーションで拒否されるバグを修正

**Change Type**: implementation

## Why

依存分析プロンプトは in-flight change を「order に含めず依存先としてのみ参照可」と指示するが、`validate_dependency_graph` は依存先が `order` に存在するかのみをチェックする。LLM が in-flight change への依存を正しく返してもバリデーションエラーで analysis が失敗する。

## What Changes

- `parse_and_validate_output` / `parse_response` / `validate_dependency_graph` に `in_flight_ids` パラメータを追加
- `validate_dependency_graph` で依存先が `order` **または** `in_flight_ids` に含まれていれば有効とする
- 既存テストの更新と、in-flight 依存参照を含むケースのテスト追加

## Impact

- Affected specs: parallel-analysis
- Affected code: `src/analyzer.rs`

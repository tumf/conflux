# Change: Agent モジュール分割

## Why
src/agent.rs が肥大化しており、実行・出力処理・履歴管理などの責務が混在している。責務分離で保守性を向上させる必要がある。

## What Changes
- Agent の実行ロジック、出力処理、履歴管理、プロンプト生成を分割する。
- 既存の公開 API と挙動は維持し、呼び出し側への影響を最小限にする。
- 既存挙動は変更せず、既存テストと追加テストで同一性を確認する。

## Impact
- Affected specs: code-maintenance
- Affected code: src/agent.rs, src/agent/*

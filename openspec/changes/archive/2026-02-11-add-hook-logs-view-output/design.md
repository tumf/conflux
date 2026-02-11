## Context
hook実行時のログはtracingには出力されるが、TUIのLogs Viewには流れていない。仕様ではLogs Viewへの表示が要求されている。

## Goals / Non-Goals
- Goals:
  - hook実行時のコマンドと出力（stdout/stderr）をLogs Viewに表示する
  - serial/parallelの経路差を吸収して同じ表示形式にする
- Non-Goals:
  - hook出力の永続保存や検索機能の追加
  - hookの実行順序や条件の変更

## Decisions
- Decision: hook実行の開始時にコマンド文字列をLogEntryとして発行し、Logs Viewに表示する
  - Rationale: 仕様（observability）で実行前表示が求められているため
- Decision: hookのstdout/stderrを一定サイズまで取得してLogs Viewへ流す
  - Rationale: 出力全量は大きくなりうるため、上限を設けて安全に表示する
- Decision: hook出力は単一のログメッセージとして扱い、TUIのLogEntryを利用する
  - Rationale: 既存のLogs Viewと同一の経路で表示できる

## Risks / Trade-offs
- 出力量が多いhookではログが肥大化する可能性があるため、サイズ制限と明示的な切り詰め表示が必要

## Migration Plan
1. hookログのイベント設計とTUI表示の整合を実装
2. hook出力取得とログ発行を追加
3. テスト追加と仕様検証

## Open Questions
- なし（既存のobservability要件に合わせて最小変更で対応）

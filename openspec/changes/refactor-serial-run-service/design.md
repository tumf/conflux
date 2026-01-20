## Context
run と TUI の serial 実行は別実装であり、StallDetector、Circuit Breaker、WIP commit、変更選択ロジックなどが分岐している。ParallelRunService による共通化の成功例に合わせて、serial もサービス層に集約する。

## Goals / Non-Goals
- Goals:
  - SerialRunService による共通フロー化
  - 既存の挙動差分をアダプタ層に分離
  - run/TUI の serial 実行における状態・履歴・フックの統一
- Non-Goals:
  - parallel 実行ロジックの変更
  - UI 表示やキー操作の変更

## Decisions
- Decision: SerialRunService を新設し、イベント/出力のハンドリングを注入可能にする
- Decision: run/TUI それぞれに薄いアダプタを残し、共通サービス層は orchestration モジュールと連携する
- Alternatives considered:
  - TUI が Orchestrator を直接利用する案 → UI 固有ロジックの混在を避けるため採用しない

## Risks / Trade-offs
- 既存の TUI 固有処理（DynamicQueue、graceful stop）と共通フローの境界が複雑化する → SerialRunService へオプション引数として注入して整理する
- 共有化によりエッジケースが増える → 既存の acceptance/archiving のシナリオを維持することを仕様で明示する

## Migration Plan
1. SerialRunService を追加
2. run の serial ループを置き換え
3. TUI の serial ループを置き換え
4. 共有関数と OutputHandler の適用範囲を整理

## Open Questions
- DynamicQueue の扱いを CLI にも拡張する必要があるか（本提案では TUI のみに留める）

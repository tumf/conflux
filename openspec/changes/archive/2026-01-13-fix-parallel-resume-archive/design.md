## Context
parallel resume時にアーカイブ済みchangeがapplyに再投入され、change探索失敗で処理が停止する現象がある。

## Goals / Non-Goals
- Goals: resume時にアーカイブ済みchangeを検出し、apply/archiveを再実行せずにmergeへ進める
- Non-Goals: 並列実行の依存関係解析ロジックの変更

## Decisions
- Decision: resume開始時にアーカイブ済みchangeを検出し、applyループに入らずにmerge対象として扱う
- Alternatives considered: applyループ内でのみ判定する方式（resume開始時の意図が分かりにくい）

## Risks / Trade-offs
- archive検出条件の誤判定 → archiveディレクトリと元のchangeディレクトリの両方を確認して回避する

## Migration Plan
- 既存resumeはそのまま維持し、archive済みのみ新しい分岐に入る

## Open Questions
- archive検出をworkspace内の状態に限定するか、ルート状態も参照するか

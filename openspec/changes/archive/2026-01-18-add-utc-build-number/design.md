## Context
CLIとTUIでバージョン表示があるが、ビルド単位で追跡できる識別子が無い。

## Goals / Non-Goals
- Goals: UTC時刻ベースのビルド番号を導入し、CLI/TUIに表示する
- Non-Goals: 既存のセマンティックバージョン管理の置き換え

## Decisions
- Decision: UTC時刻の `YYYYMMDDHHmmss` をビルド番号として採用する
- Alternatives considered: Gitコミット数、CIの連番（環境依存のため採用しない）

## Risks / Trade-offs
- 同一秒内に複数ビルドが発生した場合は識別子が重複する可能性がある

## Migration Plan
- 既存のバージョン表示に括弧付きビルド番号を追加するのみで互換性を維持する

## Open Questions
- なし

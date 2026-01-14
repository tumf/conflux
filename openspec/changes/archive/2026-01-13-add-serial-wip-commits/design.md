## Context
apply反復ごとに作業状態を残す要件はparallel実行で明文化されているが、逐次実行には同等の保証がない。さらにparallel実装はWIPを1コミットでamendしているため、スナップショットを「各反復の新規コミット」として残す運用に合っていない。

## Goals / Non-Goals
- Goals:
  - 逐次/parallelの両方で、apply反復ごとに新規WIPコミットを作成する
  - apply失敗時でもWIPを残す
  - apply成功時はWIPを1つのApplyコミットに統合する
- Non-Goals:
  - Git以外のVCS対応を追加しない
  - WIPコミットの自動削除や履歴整理のUIを追加しない

## Decisions
- Decision: WIPは各反復で新規コミットとして作成し、`--amend` を使わない
  - 理由: 反復単位の状態を履歴として残し、マージ時にsquashする運用と整合させるため
- Decision: apply失敗時もWIPコミットを作成する
  - 理由: 失敗時点の差分を保存し、再開や調査を容易にするため

## Alternatives considered
- 常に1つのWIPコミットをamendする
  - 却下理由: 反復ごとの履歴が残らず、squash前提の運用に合わない

## Risks / Trade-offs
- WIPコミットが増えるため履歴が増大する
  - 対策: apply成功時にsquashする運用を徹底する
- Git設定（user.name/user.email）が未設定だとコミット失敗となる
  - 対策: 既存のGitエラーハンドリングを活用し、必要に応じて改善する

## Migration Plan
- 既存のparallel WIPコミットを新規作成方式に切り替える
- 逐次実行のapplyループにWIPコミット作成を追加する

## Open Questions
- なし

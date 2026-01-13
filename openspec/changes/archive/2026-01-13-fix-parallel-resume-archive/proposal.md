# Change: parallel resumeでアーカイブ済みchangeを安全に処理する

## Why
parallel resumeでアーカイブ済みのchangeがapplyに再投入され、changeを見つけられずに処理が停止する。意図しない再applyを回避し、正しくmergeへ進む必要がある。

## What Changes
- resume開始時にアーカイブ済みworkspaceを検出し、apply/archiveを再実行せずにmergeへ進める
- tasks.mdが存在しない場合のapplyループ継続条件を見直し、アーカイブ済みchangeでの再実行を防ぐ

## Impact
- Affected specs: parallel-execution
- Affected code: src/parallel/mod.rs, src/parallel/executor.rs, src/execution/archive.rs

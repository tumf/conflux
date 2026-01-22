# Change: acceptance failure follow-up の tasks.md 形式を統一する

## Why
acceptance failure の追記が単一タスク＋ネスト箇条書きになっており、tasks.md のチェックリストとして扱いづらい。受け入れ指摘を個別タスク化して再実行時の差分追跡を明確にする必要がある。

## What Changes
- acceptance failure 時の tasks.md 追記を `## Acceptance #<n> Failure Follow-up` 形式に統一する
- 各 finding を `- [ ]` の個別タスクとして追加し、ラッパー行やネストを廃止する
- follow-up の番号は acceptance 試行番号に合わせる

## Impact
- Affected specs: `cli`
- Affected code: `src/orchestration/acceptance.rs`, `src/serial_run_service.rs`, `src/parallel/mod.rs`

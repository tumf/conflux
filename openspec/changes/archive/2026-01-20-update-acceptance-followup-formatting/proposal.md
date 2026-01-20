# Change: Acceptance failure follow-up formatting

## Why
Acceptance failure follow-up tasks currently record tail output as numbered items, which introduces numbering artifacts (e.g., "1) ACCEPTANCE: FAIL"). This makes tasks noisy and harder to read, and does not reflect the intent of recording raw tail lines.

## What Changes
- tasks.md のフォローアップタスクで、acceptance 出力 tail を番号ではなく行ごとに箇条書きで記録する
- tail 文字列は内容を加工せずにそのまま保持し、読みやすい形で表示する

## Impact
- Affected specs: cli
- Affected code: src/orchestration/acceptance.rs

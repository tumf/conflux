# Change: Acceptance 失敗ログの findings 誤報を防ぐ

## Why
acceptance FAIL のログが "findings 件数" を表示しているが、実際は stdout/stderr の tail 行数であり、`ACCEPTANCE:` や `FINDINGS:` 行まで数えて誤報になっている。
FINDINGS の構造解析は行わず tail を使う前提でも、誤解を招くログ表現は排除する必要がある。

## What Changes
- acceptance FAIL のログは "findings 件数" を示さず、必要な場合は "tail 行数" と明示する
- acceptance の findings として扱う tail から `ACCEPTANCE:` マーカーと `FINDINGS:` 行を除外する
- parallel/serial の acceptance で同一方針に揃える

## Impact
- Affected specs: `specs/cli/spec.md`, `specs/parallel-execution/spec.md`
- Affected code: `src/parallel/executor.rs`, `src/serial_run_service.rs`, `src/orchestration/acceptance.rs`

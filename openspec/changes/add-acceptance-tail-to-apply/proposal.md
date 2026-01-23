# Change: acceptance の出力 tail を apply に引き継ぐ

## Why
acceptance 失敗後に apply へ戻る際、acceptance の出力 tail（FINDINGS を含む）が apply プロンプトに渡らず、同じ failure を繰り返す状況が発生している。

## What Changes
- acceptance 失敗後に apply ループへ戻るとき、次の apply プロンプトへ直前の acceptance stdout/stderr tail を追加する
- tail の内容はテキストとしてそのまま渡し、FINDINGS のパースや再構成は行わない
- tail は最初の apply 試行にのみ注入し、以降の apply では再注入しない

## Impact
- Affected specs: `specs/cli/spec.md`
- Affected code: `src/agent/runner.rs`, `src/agent/prompt.rs`, `src/parallel/executor.rs`, `src/execution/apply.rs`

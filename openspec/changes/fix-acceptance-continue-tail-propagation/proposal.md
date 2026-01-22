# Change: Acceptance CONTINUE 時のコマンド出力を次の acceptance 反復に引き継ぐ

## Why

現在、acceptance ループで CONTINUE が返された場合、コマンド出力（tail lines）が次の acceptance 反復のプロンプトに引き継がれない。
これにより、エージェントは前回の acceptance 調査結果を知らずに同じ調査を繰り返し、CONTINUE が無限に続く問題が発生している。

## What Changes

- acceptance プロンプト生成時に、`AcceptanceHistory` から前回の `stdout_tail`/`stderr_tail` を取得し、プロンプトに含める
- これにより、エージェントが前回の調査結果を参照して次のアクションを決定できる

**注意**:
- CONTINUE は acceptance ループを継続（再試行）し、FAIL の場合のみ apply ループに戻る
- `AcceptanceAttempt` には既に `stdout_tail`/`stderr_tail` が保存されているため、新しいフィールド追加は不要

## Impact

- Affected specs: `cli` (acceptance loop behavior)
- Affected code:
  - `src/agent/prompt.rs` - acceptance プロンプト生成（前回の出力を追加）
  - `src/agent/runner.rs` - AcceptanceHistory からの tail 取得

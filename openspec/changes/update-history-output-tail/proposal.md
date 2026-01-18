# Change: 履歴プロンプトへコマンド出力の末尾を注入する

## Why
apply/archive の再試行時に失敗の原因となった stdout/stderr が履歴に含まれておらず、エージェントが状況を再現・理解できない。

## What Changes
- apply/archive の履歴ブロックに stdout/stderr の末尾要約を追加する
- 長い出力は末尾 N 行のみを注入してノイズとサイズを抑える
- 逐次/並列の両ループで同一の履歴出力を扱う
- resolve の履歴ブロックにも stdout/stderr の末尾要約を追加する

## Impact
- Affected specs: cli
- Affected code: src/history.rs, src/agent.rs, src/execution/apply.rs, src/execution/archive.rs, src/parallel/executor.rs

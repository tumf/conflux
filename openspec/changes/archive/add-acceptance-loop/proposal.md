# Change: acceptance loop 追加

## Why
apply 実行後に仕様充足の確認がなく、archive へ進む前に不足点を検知できません。
受け入れ検査を挟むことで、実装の不足を早期に検出し、apply 反復に戻せるようにします。

## What Changes
- apply と archive の間に acceptance loop を追加する
- `acceptance_command` を新設し、出力テキスト解析で合否と指摘事項を判定する
- 受け入れ失敗時は指摘事項を apply ループに返し、成功時のみ archive を実行する

## Impact
- Affected specs: configuration, cli, parallel-execution
- Affected code: src/orchestrator.rs, src/parallel/mod.rs, src/parallel/executor.rs, src/config/*, src/agent.rs, src/history.rs

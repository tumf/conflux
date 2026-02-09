# Change: 権限auto-rejectの検出でFuture work/停滞扱いにする

## Why
権限要求がauto-rejectされた場合でもapplyが進行できず、空WIPコミットを重ねた末にstall扱いで失敗します。エージェント側で解決できない失敗を明確に識別し、適切な停止理由と対応案内を残す必要があります。

## What Changes
- apply出力に含まれる権限auto-rejectを検出し、changeを実行不能として扱う
- 该当changeをstalled/blockedとして記録し、理由に拒否パスと権限設定の案内を含める
- 空WIPコミットによるstall検出を回避し、依存スキップへ反映する

## Impact
- Affected specs: parallel-execution
- Affected code: src/execution/apply.rs, src/serial_run_service.rs, src/parallel/mod.rs, src/orchestrator.rs

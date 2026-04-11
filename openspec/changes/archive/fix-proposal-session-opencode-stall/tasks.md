## Implementation Tasks

- [x] WebSocket proposal session の接続ライフサイクルを修正し、`notif_task` / `send_task` / `recv_task` のうちどれかが終了したら残りを確実に停止する (verification: integration - `src/server/api/proposals.rs` に対する reconnect 回帰テスト)
- [x] `AcpClient` の request/response 相関を単一 `response_rx` 依存から request ID ごとの pending waiter に置き換える (verification: unit - `src/server/acp_client.rs` の並行 request 相関テスト)
- [x] ACP stdout の JSON-RPC 判定を `method` の有無ベースに整理し、未対応 server request と非 `u64` response id を warning ログ化する (verification: unit - `src/server/acp_client.rs` の parser テスト)
- [x] reconnect 後の streaming 継続と、prompt/cancel/elicitation が重なっても proposal session が沈黙しないことを確認する回帰テストを追加する (verification: e2e - `tests/e2e_proposal_session.rs`)
- [x] 既存 proposal session 回帰を実行し、標準テスト経路で通ることを確認する (verification: integration - `cargo test -p cflx`)

## Future Work

- 実運用ログで opencode 停止報告が再発しないかを manual で監視する

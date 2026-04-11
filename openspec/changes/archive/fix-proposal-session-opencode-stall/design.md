# Design: fix-proposal-session-opencode-stall

## Overview

この変更は proposal session backend の通信責務を 2 つの観点で是正する。

1. WebSocket 接続単位で生成される relay task のライフサイクルを接続寿命に揃える
2. ACP subprocess との request/response 相関を、単一 consumer ではなく request ID 単位の waiter 配送に切り替える

## Current Failure Modes

### 1. Stale notification relay after WebSocket disconnect

`src/server/api/proposals.rs` では WebSocket 接続ごとに以下の task を起動する。

- ACP notification -> WS relay
- WS outbound sender
- WS inbound receiver

現状は `tokio::select!` でいずれか 1 つの終了を待つだけで、残タスクを停止保証していない。このため旧接続の notification relay task が生存し続け、単一 notification receiver から通知を受け取り続ける可能性がある。reconnect 後の新接続は通知を受け取れず、UI からは opencode 応答停止に見える。

### 2. Shared response receiver drops unrelated responses

`src/server/acp_client.rs` の `send_request()` は共有 `response_rx` を lock して response を待つ。待機中に別 request の response を先に受け取ると、現状は「unexpected ID」として破棄する。この構造では prompt / cancel / elicitation response などが重なった時に、正しい caller へ response が戻らず timeout しうる。

## Target Design

### WebSocket task ownership

- `proposal_session_ws` は `notif_task`, `send_task`, `recv_task` を所有する
- いずれかが終了したら残タスクを `abort()` する
- 接続寿命を超えて relay task が残存しないことを invariants とする

### ACP response correlation

- `AcpClient` は `pending_requests: HashMap<request_id, oneshot::Sender<JsonRpcResponse>>` を持つ
- `send_request()` は request 送信前に waiter を登録し、response 到着を oneshot で待つ
- stdout reader は response を受けたら request ID に対応する waiter を取り出して配送する
- waiter が存在しない response は debug/warn で観測可能にする
- timeout / process exit 時は pending entry を cleanup する

### JSON-RPC parsing discipline

stdout reader は以下の順で扱う。

1. `method` あり + `id` なし: notification
2. `method` あり + `id` あり: 未対応 server request として warning
3. `method` なし: response として parse
4. response の `id` が `u64` でない場合は warning

これにより `id/result/error` の組み合わせに依存した曖昧な分岐を避け、未対応ケースを沈黙させない。

## Verification Plan

- unit: `AcpClient` の response 相関と parser 分岐
- integration: WebSocket handler の reconnect 安定性
- e2e: proposal session での prompt/cancel/elicitation を含む継続 streaming

## Non-Goals

- proposal session protocol の仕様追加
- Dashboard 側のイベント型変更
- opencode / ACP サーバー実装への介入

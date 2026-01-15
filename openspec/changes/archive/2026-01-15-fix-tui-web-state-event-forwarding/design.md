# Design: TUIモードでのWebState更新イベント転送

## 背景

現在、TUIモード（`tui --web`）で並列実行時に、WebSocketクライアントへリアルタイム更新が送信されない問題がある。

### 現状の動作

#### CLIモード (`run --web --parallel`)
```
main.rs
├─ WebState::new() → Arc<WebState>
├─ spawn_server_with_url(web_state.clone())
└─ orchestrator.set_web_state(web_state)
    └─ orchestrator.run_parallel()
        └─ web_event_tx経由でWebStateに送信 ✅
```

#### TUIモード (`tui --web + 並列`)
```
main.rs
├─ WebState::new() → Arc<WebState>
├─ spawn_server_with_url(web_state) ← ここで消費
└─ run_tui(web_url) ← web_urlのみ渡す
    └─ run_orchestrator_parallel()
        └─ WebStateへの参照なし ❌
```

## 設計方針

### アプローチ

CLIモードで既に実装されているWebState更新ループと同じパターンをTUI側に適用する。

### コンポーネント構成

```
main.rs (TUI mode)
├─ WebState::new() → Arc<WebState>
├─ spawn_server_with_url(web_state.clone()) → (handle, url)
└─ run_tui(web_url, web_state) ← web_stateを追加
    └─ run_tui_loop(web_state)
        └─ run_orchestrator_parallel(web_state)
            ├─ parallel_tx (ParallelEvent送信用)
            └─ web_event_tx (WebState更新用) ← 追加
                └─ web_state.apply_execution_event()
```

### イベントフロー

```
ParallelExecutor
    ↓ ParallelEvent
parallel_tx
    ↓
forward_handle (TUI用)
    ├→ TUI (OrchestratorEvent) ← 既存
    └→ web_event_tx (ExecutionEvent) ← 新規追加
        ↓
WebState::apply_execution_event()
    ↓
broadcast_tx
    ↓
WebSocketクライアント
```

## 実装詳細

### 1. main.rs の変更

TUIモード起動時に `web_state` を保持し、`run_tui()` に渡す。

```rust
#[cfg(feature = "web-monitoring")]
let (web_url, web_state_opt) = if args.web {
    let web_state = std::sync::Arc::new(web::WebState::new(&changes));
    let web_config = web::WebConfig::enabled(args.web_port, args.web_bind.clone());
    match web::spawn_server_with_url(web_config, web_state.clone()).await {
        Ok((_web_handle, url)) => (Some(url), Some(web_state)),
        Err(e) => {
            tracing::warn!("Failed to start web monitoring server: {}", e);
            (None, None)
        }
    }
} else {
    (None, None)
};

run_tui(
    changes,
    args.openspec_cmd,
    args.opencode_path,
    config,
    web_url,
    web_state_opt, // 追加
)
.await?;
```

### 2. run_tui / run_tui_loop の変更

シグネチャに `web_state` 引数を追加し、下流に渡す。

```rust
pub async fn run_tui(
    initial_changes: Vec<Change>,
    openspec_cmd: String,
    _opencode_path: String,
    config: OrchestratorConfig,
    web_url: Option<String>,
    #[cfg(feature = "web-monitoring")]
    web_state: Option<Arc<WebState>>, // 追加
) -> Result<()> {
    // ...
    run_tui_loop(
        &mut terminal,
        initial_changes,
        openspec_cmd,
        config,
        web_url,
        #[cfg(feature = "web-monitoring")]
        web_state, // 追加
    )
    .await
}
```

### 3. run_orchestrator_parallel の変更

CLIモード (`orchestrator.rs:828-842`) と同様のWebState更新ループを実装。

```rust
pub async fn run_orchestrator_parallel(
    change_ids: Vec<String>,
    _openspec_cmd: String,
    config: OrchestratorConfig,
    tx: mpsc::Sender<OrchestratorEvent>,
    cancel_token: CancellationToken,
    dynamic_queue: DynamicQueue,
    graceful_stop_flag: Arc<AtomicBool>,
    #[cfg(feature = "web-monitoring")]
    web_state: Option<Arc<WebState>>, // 追加
) -> Result<()> {
    // ...

    // WebState更新ループの作成
    #[cfg(feature = "web-monitoring")]
    let (web_event_tx, web_event_handle) = if let Some(web_state) = web_state.clone() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let handle = tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                web_state.apply_execution_event(&event).await;
                if matches!(
                    event,
                    crate::events::ExecutionEvent::AllCompleted
                        | crate::events::ExecutionEvent::Stopped
                ) {
                    break;
                }
            }
        });
        (Some(tx), Some(handle))
    } else {
        (None, None)
    };

    #[cfg(feature = "web-monitoring")]
    let web_event_sender = web_event_tx.clone();

    // イベント転送
    let forward_handle = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = forward_cancel.cancelled() => {
                    break;
                }
                event = parallel_rx.recv() => {
                    match event {
                        Some(ParallelEvent::AllCompleted) => {
                            break;
                        }
                        Some(ParallelEvent::Stopped) => {
                            #[cfg(feature = "web-monitoring")]
                            if let Some(tx) = &web_event_sender {
                                let _ = tx.send(ParallelEvent::Stopped);
                            }
                            break;
                        }
                        Some(parallel_event) => {
                            // TUI転送（既存）
                            let _ = forward_tx.send(parallel_event.clone()).await;

                            // WebState転送（新規）
                            #[cfg(feature = "web-monitoring")]
                            if let Some(tx) = &web_event_sender {
                                let _ = tx.send(parallel_event);
                            }
                        }
                        None => {
                            break;
                        }
                    }
                }
            }
        }
    });

    // ... 実行処理 ...

    // クリーンアップ
    #[cfg(feature = "web-monitoring")]
    if let Some(handle) = web_event_handle {
        drop(web_event_tx);
        let _ = handle.await;
    }

    Ok(())
}
```

## トレードオフ

### 選択した方式: WebState参照の伝播

**利点**:
- CLIモードと一貫した実装
- 既存のWebState更新ロジックをそのまま利用
- テストが容易

**欠点**:
- 関数シグネチャの変更が必要
- `#[cfg(feature = "web-monitoring")]` の条件分岐が増える

### 検討した代替案1: グローバル状態

**却下理由**:
- テストが困難
- 依存関係が不明確
- Rustのベストプラクティスに反する

### 検討した代替案2: イベントバス経由

**却下理由**:
- 新しいコンポーネントの追加が必要
- 既存のCLI実装と整合性が取れない
- オーバーエンジニアリング

## テスト戦略

### 統合テスト

1. TUIモードでWeb監視を起動
2. 並列実行を開始
3. WebSocketクライアントで接続
4. `state_update` メッセージを受信することを確認

### 既存動作の確認

1. CLIモードでの動作が維持されていることを確認
2. TUIモードでWeb監視なし（`--web`なし）の動作が維持されていることを確認

## 依存関係

- `crate::web::WebState`
- `crate::events::ExecutionEvent`
- `tokio::sync::mpsc`

## パフォーマンスへの影響

- 微小なオーバーヘッド（イベントのクローンと転送）
- WebSocket接続がない場合でも、イベント転送ループは動作する（ただし、受信クライアントがいない場合は即座にドロップされる）

## セキュリティへの影響

- なし（既存のWebSocket実装と同じセキュリティモデル）

## 移行計画

- 後方互換性: 完全に維持（`--web`フラグがない場合は影響なし）
- ロールアウト: 単一のリリースで完了可能

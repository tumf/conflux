# Change: WebUI状態更新の修正

## Why
TUIモードで`--web`オプションを使用してWeb監視ダッシュボードを有効にした場合、WebSocketクライアントに状態更新がブロードキャストされないため、ダッシュボードの表示が更新されない。

根本原因:
1. `main.rs`でTUIモード時に`WebState`が作成されるが、TUIオーケストレーター関数に渡されていない
2. `src/tui/orchestrator.rs`の`run_orchestrator`/`run_orchestrator_parallel`関数に`WebState`への参照がない
3. TUIオーケストレーターのループ内で`broadcast_state_update`が呼ばれていない

## What Changes
- `run_tui`関数のシグネチャを変更し、`WebState`を受け取れるようにする
- TUIオーケストレーター関数（`run_orchestrator`、`run_orchestrator_parallel`）に`WebState`パラメータを追加
- オーケストレーターのループ内で状態変更時に`web_state.update()`を呼び出す
- `main.rs`で`WebState`をTUIに渡すように修正

## Impact
- Affected specs: web-monitoring
- Affected code: 
  - `src/main.rs`: TUIモードでの`WebState`の受け渡し
  - `src/tui/mod.rs`: `run_tui`関数のシグネチャ変更
  - `src/tui/runner.rs`: `WebState`の保持と受け渡し
  - `src/tui/orchestrator.rs`: `run_orchestrator`、`run_orchestrator_parallel`への`WebState`追加

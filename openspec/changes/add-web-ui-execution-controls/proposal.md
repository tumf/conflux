## Why
Web UIは監視専用で、TUIの実行/停止操作を遠隔から行えません。TUIと同等の操作（F5開始/再開、Esc停止、Esc→F5停止キャンセル、Esc Esc強制停止）をWeb UIから実行できるようにし、TUI/Runどちらの起動形態でも同じ運用フローを提供する必要があります。

## What Changes
- Web監視HTTPサーバーに実行/停止/リトライ制御APIを追加する（start/stop/cancel-stop/force-stop/retry）
- Web制御APIはTUI/Runの両方で同じ制御経路を使用する
- WebStateのapp_modeをTUIと同じ遷移に拡張し、stopping/errorを含むモードを配信する
- Web UIに実行/停止コントロールバーを追加し、モードに応じて表示/有効化を切り替える
- OpenAPIドキュメントに制御APIとapp_modeの語彙を追加する

## Impact
- Affected specs: web-monitoring, cli
- Affected code: src/web/api.rs, src/web/state.rs, src/web/mod.rs, web/index.html, web/style.css, web/app.js, src/tui/runner.rs, src/orchestrator.rs

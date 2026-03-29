## Implementation Tasks

- [x] 1. `dashboard/src/api/types.ts` に `StatsOverview`, `ChangeEventSummary`, `ProjectStats` 型を追加 (verification: TypeScript コンパイル)
- [x] 2. `dashboard/src/api/restClient.ts` に `fetchStatsOverview()` 関数を追加 — `GET /api/v1/stats/overview` を呼び出す (verification: TypeScript コンパイル)
- [x] 3. `dashboard/src/components/OverviewDashboard.tsx` を作成 — 統計サマリ、アクティビティタイムライン、プロジェクト別カードの 3 セクション構成 (verification: `npm run build`)
- [x] 4. `dashboard/src/App.tsx` の `<main>` 内 project 未選択時の分岐で `OverviewDashboard` をレンダリング — 既存の "Select a project" テキストを置き換え (verification: ブラウザ確認)
- [x] 5. レスポンシブ対応: モバイルレイアウトでもダッシュボードが適切に表示される (verification: ブラウザ DevTools レスポンシブモード)
- [x] 6. `npm run build && npm run lint` の全パス確認

## Future Work

- リアルタイム自動更新（WebSocket 経由の stats push）
- グラフ・チャート描画ライブラリの導入
- 時間範囲フィルタの追加

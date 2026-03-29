# Change: プロジェクト未選択時にオーケストレーションダッシュボードを表示

**Change Type**: implementation

## Why

現在の Web ダッシュボードでプロジェクト未選択時、`<main>` 領域 (`#root > div > div > main`) に "Select a project" テキストのみ表示される。この広い領域を活用し、SQLite から取得した統計データを表示するオーケストレーションダッシュボードを追加する。

## What Changes

- `dashboard/src/components/OverviewDashboard.tsx` を新設
- プロジェクト未選択時に `<main>` 領域全体を使って以下を表示:
  - 全プロジェクトの処理サマリ（成功/失敗/進行中チェンジ数）
  - 直近のアクティビティタイムライン（最新 N 件の change_events）
  - プロジェクト別の簡易統計カード（apply 成功率、平均処理時間）
- `dashboard/src/api/restClient.ts` に `fetchStatsOverview()` を追加
- `App.tsx` の project 未選択時の分岐で `OverviewDashboard` をレンダリング

## Impact

- Affected specs: dashboard-overview (新規)
- Affected code: `dashboard/src/App.tsx`, `dashboard/src/components/`, `dashboard/src/api/restClient.ts`
- **依存**: `add-server-sqlite-persistence` の stats API エンドポイントが先に実装されている必要がある

## Acceptance Criteria

- プロジェクト未選択時に `<main>` 領域全体にダッシュボードが表示される
- 全プロジェクトの成功/失敗数と直近イベントが表示される
- プロジェクト選択時は従来通りの詳細画面が表示される
- `npm run build` が通る

## Out of Scope

- グラフ・チャートの描画（将来対応）
- ダッシュボードのリアルタイム自動更新（初期は手動リフレッシュまたはページ遷移時に取得）

## Context

- `cflx server` は複数プロジェクトを管理する REST/WebSocket サーバとして既に動作している
- 既存 UI は run モード向けであり、server モード向けの複数プロジェクト管理画面が不足している
- 既存 API は dashboard 実装に必要なデータと操作（projects/state, ws, run/stop/retry, git/sync, delete）を提供している
- 今回は **既存 API を変更せず**、新規フロントエンドで統合表示・操作を実現する

## Goals / Non-Goals

### Goals
- server モード専用の React + shadcn/ui ダッシュボードを追加する
- 1 画面で Projects / Changes / Logs を同時に視認できるようにする
- プロジェクト単位で Run / Stop / Git Sync / Delete を実行できるようにする
- 状態同期は WebSocket のみを使い、REST は初期ロードと操作呼び出しに限定する
- ダーク基調・高密度・運用向けの見やすい UI を採用する

### Non-Goals
- 既存 `web/` ディレクトリ（run モード用 UI）の改善・移行・削除
- 新規 API エンドポイントの追加
- Polling フォールバックの追加
- モバイルファースト最適化（モバイル表示は対応するが primary target は desktop 運用）

## Decisions

### Decision: server モード専用 dashboard を新しいフロントエンドとして分離する

既存 `web/` は run モード用 legacy UI として扱い、新しい server dashboard は `dashboard/` に分離する。
これにより run モード向け UI と server モード向け UI の責務を混在させずに済む。

**Alternatives considered**
- 既存 `web/` を全面改修する
  - 却下: run モード用前提の構造が強く、複数プロジェクト向けの情報設計に不向き
- Rust 側テンプレートで HTML を直接生成する
  - 却下: UI の拡張性・再利用性・コンポーネント性が低い

### Decision: React + shadcn/ui + Tailwind v4 を採用する

UI は React 19 + TypeScript + Vite + shadcn/ui + Tailwind v4 で実装する。既存 repo に React 基盤はないが、server dashboard は独立したビルド成果物として扱えるため、既存 Rust コードへの影響を最小化しつつ高品質な UI を得られる。

**Alternatives considered**
- vanilla JS を継続
  - 却下: 複数パネル・状態同期・コンポーネント性・保守性に不利
- Next.js を採用
  - 却下: 単一静的バンドル UI には過剰

### Decision: WebSocket を単一の真実源として扱う

初回のみ `GET /api/v1/projects/state` を使って初期状態をロードし、その後の同期は WebSocket の `FullState` と `Log` のみで更新する。ユーザー要求に従い polling は導入しない。

**Alternatives considered**
- REST polling fallback
  - 却下: 要件で不要。状態源が増えて複雑化する

### Decision: 静的ファイルは Rust バイナリに埋め込んで配信する

`dashboard/` を Vite で build し、出力ファイルを Rust 側で `include_str!` / `include_bytes!` して `/dashboard` 配下で配信する。単一バイナリ配布という cflx の配布特性を維持する。

**Alternatives considered**
- dist を外部ファイルとして同梱
  - 却下: 配布と起動手順が複雑になる

## Architecture

### Frontend state model

- `projects: RemoteProject[]` を全量保持
- `selectedProjectId: string | null`
- `logsByProjectId: Record<string, RemoteLogEntry[]>`
- `connectionStatus: connected | reconnecting | disconnected`
- `pendingActions: Record<string, { run?: boolean; stop?: boolean; sync?: boolean; delete?: boolean }>`

### Data flow

1. ページロード時に `GET /api/v1/projects/state` を取得
2. 直後に `/api/v1/ws` へ接続
3. `FullState` 受信ごとに `projects` を置き換える
4. `Log` 受信ごとに `logsByProjectId[project_id]` に append
5. 操作ボタン押下時に REST API を実行し、結果は toast 表示。状態そのものは次の `FullState` / `Log` を待って反映する

### UI composition

- AppShell
  - Header（タイトル、接続状態、テーマ情報）
  - ProjectsPanel
  - ChangesPanel
  - LogsPanel
  - Global toaster / dialogs

### Server-side routing

`src/server/api.rs` の `/api/v1` ルーターはそのまま維持し、別ルーターまたは追加 route として以下を持つ:
- `GET /dashboard` → index.html
- `GET /dashboard/assets/*` → CSS/JS/フォント等

## Risks / Trade-offs

- **新規 frontend toolchain 導入**: repo に Node/Vite/shadcn/ui が増える
  - Mitigation: `dashboard/` 配下へ閉じ込め、Rust 本体と依存分離する
- **埋め込み配信の build 手順増加**
  - Mitigation: cargo build 前に dashboard build を行う手順を tasks に含める
- **WebSocket FullState が全量更新のため効率が悪い可能性**
  - Mitigation: 当面は既存 API に合わせて実装し、必要なら別提案で incremental update を検討

## Migration Plan

1. `dashboard/` を新設して UI を実装
2. build 生成物を Rust から配信できるようにする
3. `/dashboard` ルートを追加
4. 既存 API との疎通を確認する
5. 既存 `web/` は触らず並存させる

## Open Questions

- なし（現時点のユーザー指示で仕様は十分に確定している）

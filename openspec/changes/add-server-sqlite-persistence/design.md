## Context

`cflx server` は複数プロジェクトを管理するデーモンで、axum ベースの HTTP+WebSocket サーバーとして動作する。現在の永続化は `projects.json` のみで、それ以外のランタイムデータはインメモリ。時系列データの永続化が必要。

## Goals / Non-Goals

- Goals:
  - サーバーモード限定で時系列データを SQLite に永続化する
  - 既存のインメモリ履歴（`ApplyHistory` 等）を破壊せず、write-through で共存させる
  - 統計 API を提供し、ダッシュボード表示の基盤を作る
- Non-Goals:
  - run/tui モードへの SQLite 導入
  - `projects.json` の SQLite 移行
  - ORM やマイグレーションフレームワークの導入

## Decisions

### rusqlite (bundled) を使用

- 理由: 書き込み頻度が低く、`sqlx` のような async ORM は過剰。`rusqlite` + `tokio::task::spawn_blocking` で十分
- 代替案: `sqlx` (async native) → 依存が重い、`sled` → 時系列クエリに不向き
- `bundled` feature: 外部 SQLite ライブラリ不要でクロスプラットフォームビルドが容易

### PRAGMA user_version ベースのマイグレーション

- 理由: テーブル数が少なく、diesel/sqlx のマイグレーションフレームワークは過剰
- 実装: `ServerDb::new()` 時に `PRAGMA user_version` を確認し、埋め込み SQL で逐次適用

### Write-Through キャッシュパターン

- `ApplyHistory` / `ArchiveHistory` / `AcceptanceHistory` はインメモリで高速参照を維持
- `record()` 呼び出し時に SQLite にも INSERT（同期的に `spawn_blocking`）
- サーバー起動時に SQLite から最新 N 件をインメモリにロードする機能は将来オプション（初期は空起動で OK、既存動作と同じ）

### スキーマ設計

```sql
-- オーケストレーション実行記録
CREATE TABLE orchestration_runs (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    started_at    TEXT NOT NULL,
    stopped_at    TEXT,
    status        TEXT NOT NULL DEFAULT 'running',
    trigger       TEXT
);

-- チェンジ処理イベント
CREATE TABLE change_events (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id    TEXT NOT NULL,
    change_id     TEXT NOT NULL,
    run_id        INTEGER REFERENCES orchestration_runs(id),
    operation     TEXT NOT NULL,
    attempt       INTEGER NOT NULL,
    success       INTEGER NOT NULL,
    duration_ms   INTEGER NOT NULL,
    exit_code     INTEGER,
    error         TEXT,
    stdout_tail   TEXT,
    stderr_tail   TEXT,
    findings      TEXT,
    commit_hash   TEXT,
    verification_result TEXT,
    continuation_reason TEXT,
    created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX idx_change_events_project_change ON change_events(project_id, change_id);
CREATE INDEX idx_change_events_created ON change_events(created_at);

-- ログエントリ
CREATE TABLE log_entries (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id    TEXT,
    level         TEXT NOT NULL,
    message       TEXT NOT NULL,
    change_id     TEXT,
    operation     TEXT,
    iteration     INTEGER,
    created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX idx_log_entries_project ON log_entries(project_id);
CREATE INDEX idx_log_entries_created ON log_entries(created_at);

-- チェンジ選択・エラー状態
CREATE TABLE change_states (
    project_id    TEXT NOT NULL,
    change_id     TEXT NOT NULL,
    selected      INTEGER NOT NULL DEFAULT 1,
    error_message TEXT,
    updated_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (project_id, change_id)
);
```

## Risks / Trade-offs

- `rusqlite` は同期 API → `spawn_blocking` のオーバーヘッド（書き込み頻度が低いため許容）
- WAL モード使用で読み書き並行性を確保
- `bundled` feature でバイナリサイズが若干増加（~1MB）

## Open Questions

- なし

# Change: make build時にダッシュボードを先にビルドする

## Why
`build.rs` は `dashboard` のフロントエンドをビルドするが、Cargo の `rerun-if-changed` 判定によりビルドスクリプトが再実行されない場合がある。`make build` のような明示的なリリースビルドでは、常に最新のフロントエンド成果物を生成してから Rust バイナリへ組み込める必要がある。

## What Changes
- `make build` 実行時に `dashboard` の依存関係を確認し、`npm run build` を先に実行する
- `make install` 実行時も同じ前提で最新のダッシュボード成果物を生成してから Rust バイナリをインストールする
- Linux クロスビルド系ターゲットでも共通のフロントエンドビルド前処理を使えるようにする
- フロントエンドビルド失敗時は Rust ビルドを継続せず失敗で終了する

## Acceptance Criteria
- `make build` は毎回 `dashboard` のビルドを実行してから `cargo build --release` を実行する
- `make install` は毎回 `dashboard` のビルドを実行してから `cargo install --path .` を実行する
- `make build-linux-x86` と `make build-linux-arm` も同じ前処理を通す
- フロントエンドビルドが失敗した場合、対象の Make ターゲット全体が失敗する

## Out of Scope
- `build.rs` のビルド検知ロジック自体の削除や大幅な再設計
- Vite 設定やダッシュボード実装の変更

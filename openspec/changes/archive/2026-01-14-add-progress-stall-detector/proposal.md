# 提案: 進捗停滞検出（空WIPコミット連続）

## 概要

Ralph-claude-code のサーキットブレーカーパターンを参考に、Orchestrator が同じ change に対して「変更が生まれない反復」を繰り返してしまう状況を検出し、自動的に停止できるようにします。

本提案では、各ループ反復ごとに作成される `--allow-empty` の WIP コミット（phase 別）を基準にし、**空コミットが連続した場合**に stall と判定します。

## 背景

現状の Orchestrator は、エージェントが同じ change に対して apply や archive を再試行しても差分が発生しない場合、無駄な反復（API 呼び出しとコスト増）を継続してしまう可能性があります。

既に apply 反復では WIP コミットを作成しているため、これを「変更が生まれていない」検出の信頼できるシグナルとして利用できます。また archive には検証失敗時の retry ループがあるため、同様の検出が必要です。

## 目的

- 空コミットが連続する停滞状態を検出して、無駄な反復を止める
- 停止した change に依存しない queued change の実行を継続する
- 停止 change に依存する change は、この run ではスキップして無限待ちを回避する

## 変更内容

- **stall 判定条件の変更**
  - `completed_tasks` の不変ではなく、**空の WIP コミットが `threshold` 回連続**したら stall とする
  - 対象 phase: `apply` / `archive`
  - 対象モード: serial / parallel
  - `resolve` は別のサーキットブレーカーで扱う（今回 out of scope）

- **phase 別 WIP コミットの導入（メッセージ分離）**
  - apply: 既存の `WIP: ... apply#{n}` を維持
  - archive: `WIP(archive): ... attempt#{n}` のように prefix を分離

- **stall 時の挙動**
  - stall と判定された change は「停止（failed 扱い）」とする
  - 停止 change に依存する queued change は、この run ではスキップする
  - 停止 change に依存しない queued change があれば実行を継続する

- **成功時の phase 別 squash**
  - apply: 既存通り WIP を squash して `Apply: ...` を残す
  - archive: `WIP(archive)` を squash して `Archive: ...` を残す

- **設定の追加**
  - `stall_detection.enabled`（default: `true`）
  - `stall_detection.threshold`（default: `3`）

## 影響範囲

- `src/orchestrator.rs`: serial の stall 判定・依存スキップ・選択ロジック
- `src/orchestration/archive.rs`: serial archive retry での WIP 作成・stall 判定
- `src/parallel/*`: parallel apply/archive での stall 判定を failure として扱う
- `src/vcs/git/*`: 空コミット判定ヘルパ、phase 別 squash
- `src/config/*`: `stall_detection` 設定

## リスク

- 誤検知: 「外部状態待ち」などで差分が出ない期間を stall と誤判定する可能性
  - 対策: `enabled` と `threshold` を設定可能にする

## 非対象（今回）

- `resolve` ループの stall 検出（merge/conflict 解決）は別サーキットブレーカーとして提案・実装する

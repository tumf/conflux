## Context

本機能は、Orchestrator のループ（apply / archive）において「反復しても変更が生まれない」状態を検出し、無限ループや過剰な API 利用を防ぐことを目的とする。

既に apply 反復では `--allow-empty` の WIP コミットを作成しているため、Git のコミット差分（親との差分なし）を停滞検出の一次シグナルとして採用する。

## Goals / Non-Goals

- Goals
  - apply / archive の反復で、空コミットが連続する停滞を検出する
  - 停止した change に依存しない change の実行を継続する
  - phase 別に WIP を分離し、squash も phase ごとに行う
  - `stall_detection.enabled` と `stall_detection.threshold` を設定可能にする

- Non-Goals
  - resolve ループの停滞検出（merge/conflict の解決）は別サーキットブレーカーで扱う
  - 「時間ベース」や「出力ベース」の停滞検出は今回行わない

## Decisions

### Decision: 停滞判定は空WIPコミット連続で行う

- 反復のたびに WIP を作成し、`HEAD` と `HEAD^` の差分がゼロであれば「空コミット」とみなす
- 連続回数が `threshold` に達したら stall と判定する
- 途中で差分が発生した場合は連続回数をリセットする

### Decision: phase 別 WIP メッセージ

apply の WIP は既存の prefix（`WIP:`）とフォーマットを維持する。

archive は apply の squash 対象に混ざらないよう、`WIP(archive):` のように prefix を分離する。

### Decision: stall 時の扱い（停止 + 依存スキップ）

- stall した change は failed 扱いとして「この run では以後実行しない」
- 停止 change に依存する change は、この run ではスキップする
- 依存していない change は実行を継続する

parallel では既存の dependency skip 機構に failure として統合し、serial では選択候補から除外する。

### Decision: 成功時の phase 別 squash

- apply: 既存の WIP squash を維持して `Apply:` を残す
- archive: `WIP(archive)` をまとめて squash し、最終 `Archive:` を残す

## Risks / Trade-offs

- 外部要因で差分が出ない期間が続く場合に誤検知し得る
  - `enabled` と `threshold` を設定可能にして調整可能にする

## Open Questions

- なし（resolve の扱いは別 change で検討）

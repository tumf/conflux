## 背景
- 現行の re-analysis ループは dispatch を await するため、apply 完了まで停止しやすい。
- dynamic queue の追加が apply 実行中に分析順序を経由せず dispatch される可能性がある。
- 実装の前提（スケジューラ主導 / in-flight 追跡 / 完了トリガ）を明確にする必要がある。

## 目標
- apply 実行中でも re-analysis が継続する実行モデルを定義する。
- available_slots を in-flight 数から算出する手順を明示する。
- queue 追加が必ず analysis → dispatch の順で処理されることを保証する。

## 非目標
- 依存関係分析アルゴリズムの変更
- workspace 作成/merge/resolve フローの再設計
- acceptance/archive の仕様変更

## 決定
- スケジューラは JoinSet と Semaphore を保持し、in-flight を明示的に管理する。
- メインループは `tokio::select!` で queue 通知 / debounce / in-flight 完了 / cancel を待機する。
- dispatch は spawn して join_set で回収し、re-analysis ループは await しない。
- dynamic queue の取り込みはスケジューラ内に限定し、analysis の順序と依存解決を必ず通す。
- in-flight の定義は apply / acceptance / archive / resolve とする。

## 実装スケッチ
- 状態: `queued`, `in_flight`, `join_set`, `needs_reanalysis`
- ループ: trigger を受ける → debounce 判定 → analysis → available_slots 算出 → spawn
- in-flight 完了時に `needs_reanalysis = true` を設定し、次のループで再分析を実行

## リスク / トレードオフ
- in-flight と workspace 状態の不整合で slots 算出が崩れる
  - 対応: trigger 種別と slots/in-flight のログを必須化
- re-analysis の頻度増加
  - 対応: debounce の維持と queue 通知の優先順位整理

## 失敗時の扱い
- apply/acceptance/archive の失敗は join_set 経由で回収し、in-flight から除外する。
- cancel 時は in-flight の完了待ちを行わず、キャンセルフラグでループ終了する。

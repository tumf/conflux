# Design: parallel acceptance resume gate hardening

## Overview

parallel resume では `WorkspaceState` と tasks 完了率だけでは acceptance 完了保証が足りない。今回の不具合は、acceptance 実行開始後に verdict を残さず中断した workspace が、再開時に archive へ進めてしまうことにある。

この変更では、archive 進行可否を tasks 完了率ではなく durable acceptance state で決める。

## Goals

- acceptance verdict 未確定 workspace を archive に入れない
- cflx 再起動・中断後も resume 判定が deterministic になる
- acceptance PASS のみを archive の前提条件として扱う
- TUI / tracing で resume 判断理由を可視化する

## Non-Goals

- serial モードの再設計
- acceptance 内容や quality gate 定義の変更

## Proposed State Model

workspace ごとに acceptance state を保持する。

想定状態:

- `pending`: apply 後で acceptance 未実行、または apply により acceptance の再実行が必要
- `running`: acceptance command を開始したが最終 verdict 未確定
- `passed`: 最新 apply revision に対して acceptance PASS 済み
- `failed`: 最新 acceptance が FAIL / CONTINUE-exceeded / BLOCKED / command error で終わった

最低限必要な保持情報:

- state
- state を記録した対象 revision
- updated_at

## Persistence Location

workspace ローカルの cflx 管理ファイルに保持する。

候補:

- `<workspace>/.cflx/acceptance-state.json`

理由:

- git commit や OpenSpec ファイルへ混ぜずに durable に残せる
- resume routing と archive guard の双方から読める
- interrupted / running 状態をそのまま表現できる

## Lifecycle Rules

### Apply completion

- apply が進んで revision が更新されたら acceptance state を `pending` に戻す
- 以前の `passed` は同一 revision に対してのみ有効

### Acceptance start

- acceptance command 起動直前に state を `running` として保存する

### Acceptance terminal result

- PASS: `passed`
- FAIL / BLOCKED / command failure / continue exceeded: `failed`
- プロセス中断や再起動で verdict が残らなかった場合、次回 resume 時に `running` は `pending` 相当として扱う

### Resume routing

- `WorkspaceState::Applied` かつ acceptance state != `passed` -> `Acceptance`
- `WorkspaceState::Archiving` かつ acceptance state != `passed` -> `Acceptance`
- `WorkspaceState::Applied` / `Archiving` かつ acceptance state == `passed` -> archive へ進行可能
- `Archived` / `Merged` は従来どおり terminal

### Archive guard

archive command 起動直前に以下を満たす必要がある:

1. tasks が完了している
2. durable acceptance state が `passed`
3. `passed` が current revision と対応している

いずれかを満たさなければ archive を開始せず acceptance に戻す。

## Failure / Restart Semantics

今回のログ事象では acceptance cycle 5 開始ログはあるが PASS/FAIL がなく、その後 archive が始まっている。これを防ぐため、`running` state を restart-safe に扱う。

- 再起動後に `running` を見つけた場合、その acceptance は未完了扱い
- archive へ進めず、必ず acceptance を再実行する
- ログに「previous acceptance was interrupted; rerunning acceptance before archive」と出す

## Test Strategy

1. acceptance state 保存/読取の単体テスト
2. resumed `Applied` workspace が `passed` なしでは `Acceptance` にルーティングされるテスト
3. resumed `Archiving` workspace が `passed` なしでは archive に進まないテスト
4. acceptance started -> process interrupted -> restart の回帰テスト
5. archive guard が `passed` 不在時に command 起動を拒否するテスト

## Canonical Spec Impact

archive 前提条件として「durable acceptance-pass state」が canonical spec に追加される。parallel resume spec は、tasks 完了だけでは archive 可否を決めず、interrupted acceptance を再実行必須とする内容へ更新される。

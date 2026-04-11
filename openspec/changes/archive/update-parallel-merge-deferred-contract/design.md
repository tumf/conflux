## Context

parallel merge deferred の意味論は spec 上で重複追記され、実装上でも `reason.contains("Resolve in progress")` による auto-resumable 判定が残っている。今回の合意では、resolve active かどうかを最優先に見て auto-resumable deferred を決め、dirty base は常に manual intervention 扱いにする。また scheduler は pending merge task が残る限り終了してはならない。

## Goals / Non-Goals

- Goals:
  - parallel merge deferred contract を 1 つの canonical rule に統合する
  - auto-resumable 判定から reason 文字列解析を除去する
  - reducer / scheduler / queue の wait-state 契約を揃える
- Non-Goals:
  - serial orchestration の wait-state 再設計
  - conflict resolution retry strategy の全面変更
  - archive 実装そのものの redesign

## Decisions

- Decision: merge 試行前に resolve カウンターを最優先で評価する
  - Alternatives considered:
    - base dirty を先に見る: rejected because resolve 起因の dirty 状態を manual wait と誤分類しうる

- Decision: dirty base は reason の内容に関わらず manual intervention (`MergeWait`, `auto_resumable=false`) とする
  - Alternatives considered:
    - dirty reason 文字列から auto-resumable を推定する: rejected because message wordingに依存し、spec/実装の両方を不安定にする

- Decision: scheduler は `pending_merge_count > 0` の間は終了しない
  - Alternatives considered:
    - join set / queued / in_flight のみで完了判定する: rejected because background merge task の完了前に loop が終了しうる

## Risks / Trade-offs

- typed な deferred contract を導入すると merge pipeline の event 署名やテスト更新が広がる
- reducer と queue の両方で wait-state を扱うため、event-to-state mapping の整合維持が必要になる
- 既存ログ文言依存のテストは書き換えが必要になる

## Migration Plan

1. spec delta で canonical merge-deferred rule を明文化する
2. merge result contract を明示型へ寄せ、string parsing を除去する
3. reducer / queue / tests を新 contract に合わせて更新する
4. rust quality gates で回帰を確認する

## Open Questions

- `MergeAttempt::Deferred` を typed enum にするか、既存 variant に補助フィールドを足すかは実装時に最小変更で決める

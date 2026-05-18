# Design: Orchestration state reducer の責務分割

## 方針

この変更は no functional behaviour change を前提とする。最初に characterization test で現在の状態遷移を固定し、その後にファイルまたはヘルパー単位で責務を分ける。

## 分割候補

- command reducer: `ReducerCommand` ごとの user intent 変換。
- event reducer: `ExecutionEvent` ごとの実行状態反映。
- wait queue helpers: resolve/reject wait queue の重複排除、移動、clear。
- transition helpers: terminal/activity/wait/blocked metadata を同時更新する小さな遷移関数。

## Trade-offs

- まず public API を保つことで呼び出し側の変更範囲を最小化する。
- 状態型の完全な再設計は避け、既存テストで固定された遷移を保つ。
- ログや UI 状態を reducer の判断材料に加えないことで、workspace-local workflow state の憲法制約を維持する。

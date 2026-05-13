# 設計: オーケストレーション状態遷移の副作用整理

## 方針

この変更は reducer の意味論を変えず、match arm 内に散在する副作用を意図名のある helper へ集約します。対象は特に wait queue 操作、terminal state 遷移、blocked metadata の設定/解除、success event による recoverable terminal の上書きです。

## トレードオフ

- 大規模なファイル分割は行わない。まず低リスクに内部 helper 抽出へ限定する。
- 状態型や public API は変更しない。テストが固定する現在の reducer contract を優先する。
- helper は過度に抽象化せず、イベントカテゴリごとの意図が読み取れる粒度に留める。

## 検証戦略

既存の状態遷移テストを先に通し、足りない代表経路だけ characterization test を追加する。外部コマンドは不要で、Reducer の pure state transition を unit test として検証する。

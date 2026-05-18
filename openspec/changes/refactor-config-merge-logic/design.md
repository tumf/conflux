# Design: Config merge logic の共通化

## 方針

設定読み込みの外部挙動は変更しない。`OrchestratorConfig::merge` の「高優先 config の `Some` だけが勝つ」という規則を helper で表現し、deep merge が必要な領域だけを明示的に分ける。

## 分割候補

- scalar/Option field merge helper。
- nested config overwrite helper。
- hooks deep merge 専用処理。
- deprecated path helper 互換テスト群。

## Trade-offs

- macro による過度な抽象化は避け、差分レビューしやすい helper 化を優先する。
- 設定探索順や deprecated helper の削除は行わず、互換 contract を維持する。

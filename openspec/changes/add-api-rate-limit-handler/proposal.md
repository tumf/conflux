# 提案: APIレート制限ハンドリング（API Rate Limit Handler）

## 概要

Ralph-claude-codeのレート制限ハンドリングを参考に、エージェントAPIのレート制限エラーを検出し、適切に待機または停止する機能を追加します。

## 背景

Claude APIには5時間あたりの使用量制限があります。Ralph-claude-codeは、この制限に達した場合、以下の選択肢を提示します：
1. 60分待機（カウントダウンタイマー付き）
2. 30秒後に自動終了

現在のOrchestratorは、レート制限エラーを単なるエラーとして扱い、無駄なリトライを繰り返す可能性があります。

## 目的

- APIレート制限エラーの自動検出
- 待機時間の計算と表示
- 無駄なリトライループの防止

## 影響範囲

- `src/error.rs`: レート制限エラーの型追加
- `src/agent.rs`: レート制限エラーの検出とパース
- `src/orchestrator.rs`: 待機ロジックと再開処理
- `src/config/mod.rs`: 待機戦略の設定

## リスク

- 中リスク: API provider依存（Claude, OpenAI, etc.）
- 待機中のユーザー体験（TUIモードとの連携）

## 代替案

- 即座にエラー終了（待機なし）
- exponential backoffでリトライ
- 複数のAPIプロバイダーへのフォールバック

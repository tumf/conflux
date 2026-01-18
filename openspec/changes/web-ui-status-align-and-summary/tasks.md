## 1. Specification Alignment
- [ ] 1.1 TUIのQueueStatus表記に合わせたWeb UIステータス対応表を確定する（検証: changes/web-ui-status-align-and-summary/specs/web-monitoring/spec.md に記載されている）
- [ ] 1.2 Web UIの全体進捗を最上位に配置する要件を明文化する（検証: spec.md のシナリオを確認）

## 2. Backend State Mapping
- [ ] 2.1 WebStateのqueue_status表示値をTUIの表記に整える（検証: src/web/state.rs の更新と対応テスト）
- [ ] 2.2 イテレーション番号をWebSocket/RESTの変更状態に含める設計を決める（検証: spec.md で入力元と表示方法が定義されている）

## 3. Web UI Layout
- [ ] 3.1 変更一覧をスリム化したカードレイアウトに更新する（検証: web/index.html と web/style.css に反映）
- [ ] 3.2 全体進捗セクションを最上位に配置する（検証: web/index.html の構造を確認）
- [ ] 3.3 ステータス表示にアイコンを導入する（検証: web/style.css と web/app.js のレンダリング更新）

## 4. Web UI Interaction
- [ ] 4.1 SPC/Approve操作を折りたたみ表示にする（検証: web/app.js の UI トグル、CSS 表示制御）
- [ ] 4.2 change ごとのイテレーション番号を表示する（検証: web/app.js のレンダリング）

## 5. Validation
- [ ] 5.1 npx @fission-ai/openspec@latest validate web-ui-status-align-and-summary --strict を実行し、エラーがないことを確認する（検証: コマンド結果）

## 1. Implementation
- [ ] 1.1 queuedのみをanalysis対象にするフィルタ条件を追加する
- [ ] 1.2 queued外のchangeをanalysis対象から除外する
- [ ] 1.3 実行中changeがなくqueuedも空のときに終了する判定を追加する
- [ ] 1.4 queueが空のときはanalysisを実行しない

## 2. Tests
- [ ] 2.1 queuedのみがanalysis対象になることを検証する
- [ ] 2.2 queued外のchangeがanalysis対象から除外されることを検証する
- [ ] 2.3 実行中・queuedが空のときに並列実行が終了することを検証する

## 3. Validation
- [ ] 3.1 cargo fmt
- [ ] 3.2 cargo clippy
- [ ] 3.3 cargo test

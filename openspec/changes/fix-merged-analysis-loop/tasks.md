## 1. Implementation
- [ ] 1.1 merged判定ルール（archive commit存在・changesディレクトリ消失）を整理する
- [ ] 1.2 analysis前にmerged済みchangeを除外する処理を追加する
- [ ] 1.3 除外時にログ/イベントを発行する
- [ ] 1.4 すべて除外された場合に処理を終了する

## 2. Tests
- [ ] 2.1 merged済みchangeがanalysis対象から外れることを検証する
- [ ] 2.2 全件merged時に並列実行が終了することを検証する

## 3. Validation
- [ ] 3.1 cargo fmt
- [ ] 3.2 cargo clippy
- [ ] 3.3 cargo test

## Implementation Tasks

- [ ] 現在の config merge contract を characterization test で固定する（verification: unit - custom/project/XDG env/XDG default/platform/default の優先順、`Some` 上書き、`None` 非上書きを確認する）
- [ ] hooks deep merge と通常 Option 上書きの違いを characterization test で固定する（verification: unit - hooks の個別フィールド merge と server/proposal session 等の上書き挙動を確認する）
- [ ] deprecated path helper の後方互換を characterization test で固定する（verification: unit - `get_global_config_path` と `get_xdg_config_path` の既存 fallback/precedence を確認する）
- [ ] `OrchestratorConfig::merge` の通常 Option フィールド上書きを共通ヘルパーへ集約する（verification: unit - merge characterization test がリファクタ前と同じ期待値で成功する）
- [ ] deep merge や特殊互換処理を通常 Option 上書きと読み分けられる構造へ分離する（verification: unit - hooks deep merge、operation skill merge、server config validation の既存テストが成功する）
- [ ] config テストの配置または命名を整理し、path precedence と merge priority の意図を追跡しやすくする（verification: unit - `cargo test config` または該当 config テストが成功する）
- [ ] 対象検証を実行する（verification: manual - `cargo test config` または該当 config テスト、`cargo fmt --check` を実行し、設定 contract の差分がないことを確認する）

## Future Work

- deprecated helper の廃止判断は後方互換に関わるため、別提案で扱う。

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate refactor-config-merge-logic --archive-gate`

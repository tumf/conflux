## Implementation Tasks

- [ ] 1. `parse_and_validate_output` に `in_flight_ids: &[String]` パラメータを追加し、`parse_response` へ伝搬する (verification: `cargo build` が通る)
- [ ] 2. `parse_response` に `in_flight_ids: &[String]` パラメータを追加し、`validate_dependency_graph` へ伝搬する (verification: `cargo build` が通る)
- [ ] 3. `validate_dependency_graph` に `in_flight_ids: &[String]` パラメータを追加し、依存先が `order` または `in_flight_ids` に含まれていれば OK とする (verification: `cargo build` が通る)
- [ ] 4. `analyze_with_callback` から `parse_and_validate_output` へ `in_flight_ids` を渡す (verification: `cargo build` が通る)
- [ ] 5. テスト追加: in-flight change を依存先に含む result がバリデーションを通ることを確認する (verification: `cargo test test_validate_dependency_graph_with_inflight`)
- [ ] 6. テスト追加: in-flight にも order にも含まれない依存先が拒否されることを確認する (verification: `cargo test test_validate_dependency_graph_invalid_inflight_ref`)
- [ ] 7. `cargo fmt --check && cargo clippy -- -D warnings && cargo test` が全て通ることを確認 (verification: CI green)

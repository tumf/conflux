# REJECTED

- change_id: refactor-split-server-api
- reason: `mod.rs` に集中している統合テストを責務別サブモジュールへ移管する際、共有テストヘルパーの公開範囲と所有先を決めないと重複/循環依存を回避できない
- proposed_by: apply

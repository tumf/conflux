# REJECTED

- change_id: refactor-extract-git-sync-test-fixtures
- reason: remote_ahead シナリオが現行 fixture と resolve_command=true の組み合わせでは非 fast-forward を解消できず 422 を返すため、要求される 200/synced 契約 assertion を満たせない
- proposed_by: apply

## MODIFIED Requirements

### Requirement: resolve-merge-exclusive-execution

resolve_merge() が即時開始パスを取る際、is_resolving フラグを即座に true に設定し、後続の M キー操作がキュー追加パスに入ることを保証する。

#### Scenario: consecutive-m-key-press-during-resolve

**Given**: change-a が MergeWait 状態で、is_resolving が false
**When**: change-a に対して M キーを押す
**Then**: is_resolving が即座に true になり、TuiCommand::ResolveMerge(change-a) が返される

#### Scenario: second-m-key-queues-when-first-resolving

**Given**: change-a の resolve_merge() が即時開始され is_resolving が true
**When**: MergeWait 状態の change-b に対して M キーを押す
**Then**: change-b は ResolveWait に遷移し、resolve キューに追加される（即時開始されない）

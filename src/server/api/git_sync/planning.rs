#[derive(Debug, PartialEq, Eq)]
pub(super) struct SyncPlan {
    pub(super) should_skip_resolve_and_push: bool,
}

pub(super) fn plan_sync(local_sha_for_push: &str, remote_sha_for_push: &str) -> SyncPlan {
    let should_skip_resolve_and_push =
        !remote_sha_for_push.is_empty() && local_sha_for_push == remote_sha_for_push;

    SyncPlan {
        should_skip_resolve_and_push,
    }
}

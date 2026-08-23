//! Tests for parallel execution module.

#[cfg(test)]
mod acceptance_runtime_limit;
#[cfg(test)]
mod analysis_liveness_loop;
#[cfg(test)]
mod auto_resolve;
#[cfg(test)]
mod capacity_gated_reanalysis;
#[cfg(test)]
mod change_error_f5_retry;
#[cfg(test)]
mod change_local_merge_error_scope;
#[cfg(test)]
mod conflict;
#[cfg(test)]
mod effective_dependency_base;
#[cfg(test)]
mod executor;
#[cfg(test)]
mod failed_dependency;
#[cfg(test)]
mod idle_parallel_stop;
#[cfg(test)]
mod manual_resolve;
#[cfg(test)]
mod manual_resolve_continuation;
#[cfg(test)]
mod per_change_upstream;
#[cfg(test)]
mod reanalysis_trigger_lifetime;
#[cfg(test)]
mod reducer_snapshot_contention;

mod run_command_scope;
#[cfg(test)]
mod running_mark_reanalysis;
#[cfg(test)]
mod unchanged_analysis_input;
#[cfg(test)]
mod upstream_integration;
#[cfg(test)]
mod workspace_resume;

//! `cflx client refresh-proposal`: preview and apply a candidate proposal
//! refresh against an existing owner.
//!
//! The client runs no Git. The preview is the owner's read resource and the
//! apply is the owner's ordinary fenced `refresh_proposal` command; this module
//! only chooses which of the two to call and projects the answer onto the
//! envelope. Eligibility, expectations, the hook-enabled commit, and the
//! truthful effects all belong to the owner's shared transaction.

use crate::client::control::{
    describe_block, not_command_capable, restarted_envelope, revision_conflict,
    submit_and_settle_within, SubmitFailure, MAX_REVISION_ATTEMPTS,
};
use crate::client::envelope::{Operation, Outcome, ResultEnvelope};
use crate::client::session::{
    capabilities, describe_api_error, observe, refresh_preview_available, Connection,
};
use crate::web::remote_control_api::dto::{CommandSpec, CommandState, RefreshPreviewResponse};

/// How long one apply may stay `running` before the client stops waiting.
///
/// Longer than an ordinary command because the one commit runs repository
/// hooks; still finite, so a hung hook is a typed failure rather than a hang.
const REFRESH_SETTLEMENT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30 * 60);

/// Which form an invocation selected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Form {
    /// `--dry-run`: read the preview, submit nothing.
    Preview,
    /// Both expectations: submit the apply.
    Apply {
        /// Full commit ID the source ref must still resolve to.
        expected_source: String,
        /// Full commit ID the candidate `HEAD` must still be at.
        expected_head: String,
    },
}

/// Run one `refresh-proposal` invocation.
pub async fn run(
    connection: &Connection,
    change_id: &str,
    source_ref: &str,
    form: Form,
) -> ResultEnvelope {
    match form {
        Form::Preview => preview(connection, change_id, source_ref).await,
        Form::Apply {
            expected_source,
            expected_head,
        } => {
            apply(
                connection,
                change_id,
                source_ref,
                &expected_source,
                &expected_head,
            )
            .await
        }
    }
}

/// Read the owner's preview resource.
///
/// Served by a headless `cflx run` too: it is a read, and it needs no command
/// executor. Whether it is served at all is the owner's published capability,
/// read first: an owner that predates the surface is a compatible build that
/// does not offer this one operation, which is `unsupported_owner`, and it is
/// never asked for a route it cannot have.
async fn preview(connection: &Connection, change_id: &str, source_ref: &str) -> ResultEnvelope {
    let operation = Operation::RefreshProposalPreview;
    let capabilities = match capabilities(connection).await {
        Ok(capabilities) => capabilities,
        Err(error) => return error.into_envelope(operation),
    };
    if !refresh_preview_available(&capabilities) {
        return unsupported(operation, Some(capabilities.instance_id));
    }
    let path = format!(
        "/api/v2/proposals/{}/refresh-preview?source_ref={}",
        crate::client::transport::encode_query_value(change_id),
        crate::client::transport::encode_query_value(source_ref)
    );
    let response = match connection.client().get(&path).await {
        Ok(response) => response,
        Err(error) => {
            return crate::client::session::ObserveError::from(error).into_envelope(operation)
        }
    };
    match response.status {
        200 => {}
        401 | 403 => {
            return ResultEnvelope::new(operation, Outcome::AuthenticationFailed).with_message(
                describe_api_error(
                    &response.body,
                    "the owner refused the presented credentials",
                ),
            )
        }
        404 => {
            // A typed `not_found` names the change. Anything else is a route
            // this owner does not serve even though its capabilities said it
            // would, which is a contract mismatch rather than an unsupported
            // surface.
            let typed = serde_json::from_slice::<crate::web::remote_control_api::dto::ApiError>(
                &response.body,
            )
            .ok();
            return match typed {
                Some(error) => ResultEnvelope::new(operation, Outcome::ChangeNotFound)
                    .with_message(format!("{} ({})", error.message, error.error_code.as_str())),
                None => ResultEnvelope::new(operation, Outcome::IncompatibleOwner).with_message(
                    "the owner does not serve the refresh-preview resource, so it is not a \
                     compatible build",
                ),
            };
        }
        status => {
            return ResultEnvelope::new(operation, Outcome::TransportError).with_message(
                describe_api_error(
                    &response.body,
                    &format!("the refresh preview returned unexpected status {status}"),
                ),
            )
        }
    }
    let preview: RefreshPreviewResponse = match response.json() {
        Ok(preview) => preview,
        Err(error) => {
            return ResultEnvelope::new(operation, Outcome::IncompatibleOwner).with_message(
                format!("the refresh preview did not match this build: {error}"),
            )
        }
    };
    let message = match (&preview.refusal, preview.no_op) {
        (Some(reason), _) => format!(
            "preview of '{change_id}' from '{source_ref}': an apply would be refused as `{reason}`"
        ),
        (None, true) => format!(
            "preview of '{change_id}' from '{source_ref}': the candidate already matches; an \
             apply would settle unchanged"
        ),
        (None, false) => format!(
            "preview of '{change_id}' from '{source_ref}': +{} / -{} lines; nothing was submitted",
            preview.added_lines, preview.removed_lines
        ),
    };
    ResultEnvelope::new(operation, Outcome::Previewed)
        .with_instance(Some(preview.instance_id.clone()))
        .with_message(message)
        .with_detail(serde_json::json!({
            "commands_submitted": 0,
            "state_revision": preview.state_revision,
            "eligible": preview.eligible,
            "refusal": preview.refusal,
            "source_ref": preview.source_ref,
            "source_commit": preview.source_commit,
            "candidate_head": preview.candidate_head,
            "path": preview.path,
            "source_blob": preview.source_blob,
            "candidate_blob": preview.candidate_blob,
            "no_op": preview.no_op,
            "added_lines": preview.added_lines,
            "removed_lines": preview.removed_lines,
            "patch": preview.patch,
            "preview_detail": preview.detail,
        }))
}

/// Submit the apply through the ordinary fenced command.
async fn apply(
    connection: &Connection,
    change_id: &str,
    source_ref: &str,
    expected_source: &str,
    expected_head: &str,
) -> ResultEnvelope {
    let operation = Operation::ControlRefreshProposal;
    let mut first_instance: Option<String> = None;

    for attempt in 0..MAX_REVISION_ATTEMPTS {
        let observation = match observe(connection, None).await {
            Ok(observation) => observation,
            Err(error) if error.is_transient() && attempt + 1 < MAX_REVISION_ATTEMPTS => continue,
            Err(error) => return error.into_envelope(operation),
        };
        match &first_instance {
            None => first_instance = Some(observation.instance_id.clone()),
            Some(expected) if *expected != observation.instance_id => {
                return restarted_envelope(operation, expected, &observation.instance_id)
            }
            Some(_) => {}
        }
        let instance = Some(observation.instance_id.clone());
        // The surface comes before the executor: an owner that predates the
        // command would refuse it as unknown whether or not an executor is
        // bound, so nothing is submitted and no executor is blamed.
        if !observation.proposal_refresh_command_available() {
            return unsupported(operation, instance);
        }
        if !observation.command_capable() {
            return not_command_capable(operation, instance);
        }
        let Some(change) = observation.change(change_id) else {
            return ResultEnvelope::new(operation, Outcome::ChangeNotFound)
                .with_instance(instance)
                .with_message(format!(
                    "the owner does not track a proposal named '{change_id}'"
                ));
        };
        // The owner's own published table, read only to refuse before a record
        // exists. The shared transaction re-validates it anyway. The owner
        // advertised the command, so a row without the table is a shape this
        // build does not understand, never a permission.
        let Some(eligibility) = change.actions.refresh_proposal else {
            return ResultEnvelope::new(operation, Outcome::IncompatibleOwner)
                .with_instance(instance)
                .with_message(format!(
                    "the owner advertises `refresh_proposal` but published no refresh \
                     eligibility for '{change_id}', so its state does not match this build"
                ))
                .with_detail(serde_json::json!({
                    "commands_submitted": Vec::<serde_json::Value>::new(),
                    "observed_status": change.display_status,
                }));
        };
        if !eligibility.allowed {
            return ResultEnvelope::new(operation, Outcome::TargetIneligible)
                .with_instance(instance)
                .with_message(format!(
                    "this owner cannot refresh the proposal of '{change_id}' right now ({}); its \
                     display status is '{}'",
                    describe_block(eligibility.blocked_reason),
                    change.display_status
                ))
                .with_detail(serde_json::json!({
                    "commands_submitted": Vec::<serde_json::Value>::new(),
                    "observed_status": change.display_status,
                    "blocked_reason": eligibility.blocked_reason,
                }));
        }

        let mut audit = Vec::new();
        let submitted = submit_and_settle_within(
            connection,
            CommandSpec::RefreshProposal {
                change_id: change_id.to_string(),
                source_ref: source_ref.to_string(),
                expected_source: expected_source.to_string(),
                expected_head: expected_head.to_string(),
            },
            "refresh_proposal",
            Some(change_id),
            observation.state_revision,
            &observation.instance_id,
            &mut audit,
            REFRESH_SETTLEMENT_TIMEOUT,
        )
        .await;
        return match submitted {
            Ok(record) => {
                let outcome = match record.state {
                    CommandState::NoOp => Outcome::Unchanged,
                    _ => Outcome::Refreshed,
                };
                ResultEnvelope::new(operation, outcome)
                    .with_instance(instance)
                    .with_message(record.detail.clone().unwrap_or_default())
                    .with_detail(result_detail(&audit, &record))
            }
            Err(SubmitFailure::Stale { .. }) => continue,
            Err(SubmitFailure::Refused(record)) => {
                ResultEnvelope::new(operation, Outcome::RefreshRefused)
                    .with_instance(instance)
                    .with_message(record.detail.clone().unwrap_or_default())
                    .with_detail(result_detail(&audit, &record))
            }
            Err(failure) => failure
                .into_envelope(operation, instance)
                .with_detail(serde_json::json!({ "commands_submitted": audit })),
        };
    }

    revision_conflict(operation, first_instance)
}

/// The owner is reachable and speaks this API, but does not serve candidate
/// proposal refresh at all. Nothing was submitted.
fn unsupported(operation: Operation, instance: Option<String>) -> ResultEnvelope {
    ResultEnvelope::new(operation, Outcome::UnsupportedOwner)
        .with_instance(instance)
        .with_message(
            "this owner does not serve candidate proposal refresh, so nothing was previewed or \
             submitted. Every other client operation still works against it; refreshing needs \
             an owner built with the surface",
        )
        .with_detail(serde_json::json!({ "commands_submitted": 0 }))
}

/// The envelope detail for a settled record: the typed result, flattened.
fn result_detail(
    audit: &[serde_json::Value],
    record: &crate::web::remote_control_api::dto::CommandRecord,
) -> serde_json::Value {
    let mut detail = serde_json::json!({
        "commands_submitted": audit,
        "command_state": record.state,
        "result_revision": record.result_revision,
    });
    if let (
        Some(object),
        Some(crate::web::remote_control_api::dto::CommandResult::RefreshProposal(result)),
    ) = (detail.as_object_mut(), record.result.as_ref())
    {
        if let Ok(serde_json::Value::Object(fields)) = serde_json::to_value(result) {
            object.extend(fields);
        }
    }
    detail
}

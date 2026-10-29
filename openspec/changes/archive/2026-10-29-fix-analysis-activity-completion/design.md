# Design

## Cause and selected transition

The existing producer is sequential per executor and returns an `AnalysisOutcome`, not a Result. A returned fallback or unusable empty order still ends one invocation. Add exactly one `send_event(...AnalysisCompleted { groups_found }).await` directly after the analyzer await and before inspecting `result.order`. Use `result.groups.as_ref().map_or(0, Vec::len)`. The completed event is already presentation-only and already closes the facts-store activity; no new workflow authority is needed.

Keep the original return object intact. The existing empty-order retry arm must run unchanged after this closing observation, while later capacity-zero paths observe closure without requiring a task to launch. Interruption here means the producer future is dropped before it returns; it cannot run its post-await code and gets no forged completion. If cancellation is handled inside the analyzer and it returns an outcome, the invocation is returned and does emit completion. This change does not add a Drop task or asynchronous destructor. Process terminal cleanup remains the existing path.

## Discriminating tests

Use a real `ParallelExecutor` analysis pass with valid repository-local proposal fixtures and existing reducer/harness support. An analyzer double only controls the return; it must not emit events. A oneshot barrier pauses return after production Started is observed. Bridge actual producer output into the real `EventDispatcher`/`ExecutionFactsStore`; observe an event sink/barrier after Completed dispatch rather than racing a background receiver. Test healthy, metadata-only, fallback and empty results. Assert one Started/Completed pair in order and groups metadata (including None => 0). Check unrelated activity separately; a fabricated start for the unrelated control is allowed, fabricated analysis completion is not.

Exercise the actual pass/loop's empty-order or zero-capacity branch without running an AI/lifecycle child. Pin returned order/dependencies/provenance and corresponding existing retry behavior. Abort a genuinely pending analysis future after Started and assert no Completed from production. Keep source-controlled fixtures in the repository and operational evidence outside target worktrees. Use module name `analysis_activity_completion` for the bounded nonempty selector. No test-only runtime setting is necessary; use existing executor injection surfaces.

## Rejected alternatives

Clearing activity on idle or every Ready event hides still-active work. Waiting for workflow AllCompleted fails for a persistent scheduler. Changing the analyzer or blocking the non-authoritative judge is unrelated. A polling timeout is guessed completion. New activity variants/serialized IDs/epoch redesign are not required for this single sequential producer contract.

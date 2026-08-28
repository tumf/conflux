## ADDED Requirements

### Requirement: Initial client wait target validation

`cflx client wait <change-id>` MUST validate the requested change against its initial coherent owner observation before entering its observation loop. If the requested change is absent, wait MUST perform one bounded repository certification of the owner's declared terminal mode; when certification proves completion, wait MUST return `completed` under the existing completion rules. Otherwise wait MUST return immediately with outcome `change_not_found`, exit status `9`, the requested `change_id`, the observed owner instance, and zero submitted commands. A known change in `not queued` or another owner-progressing state MUST retain the observation behavior defined by `Observation-only completion wait`. Absence after the target was previously observed MUST retain the existing disappearance behavior and MUST NOT be reclassified by this initial-validation rule.

#### Scenario: Unknown initial wait target is refused immediately

**Given**: a coherent owner snapshot does not contain change `aaaa`
**When**: a caller runs `cflx client wait aaaa --json` without a positive timeout
**Then**: wait returns immediately with outcome `change_not_found` and exit status `9`
**And**: the envelope identifies `aaaa` and the observed owner instance
**And**: wait submits no mutation command

#### Scenario: Already archived absent change is certified, not refused

**Given**: a coherent owner snapshot does not contain change `alpha`
**And**: repository evidence proves `alpha` reached the owner's declared terminal mode
**When**: a caller runs `cflx client wait alpha --json`
**Then**: wait returns `completed` under the existing certification rules
**And**: it does not return `change_not_found`

#### Scenario: Known unqueued wait target continues observing

**Given**: a coherent owner snapshot contains change `alpha` with display status `not queued`
**When**: a caller runs `cflx client wait alpha --timeout 100ms --json`
**Then**: wait observes `alpha` until the explicit deadline or another typed outcome
**And**: it does not return `change_not_found`

#### Scenario: Later disappearance is not initial absence

**Given**: wait has already coherently observed change `alpha`
**When**: a later owner snapshot no longer contains `alpha`
**Then**: wait applies the existing disappearance and repository-evidence rules
**And**: it does not classify the later absence as an initially unknown target

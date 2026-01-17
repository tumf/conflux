# Design

## Overview

If reverse movement (archive → changes) persists after archive completion, execution stops at `MergeWait`. To prevent this, the archive/merge flow is protected by three layers of verification.

## Guard Design

1. **Pre-Archive Commit Check**
   - Immediately error if `openspec/changes/<change_id>` exists when executing `ensure_archive_commit`.
   - Detect reverse movement or manual restoration and block commit creation.

2. **Strengthened Archive Commit Completion Detection**
   - In addition to a clean working tree, `is_archive_commit_complete` now requires that `openspec/changes/<change_id>` does not exist.
   - Treat as incomplete even if archive commit exists when changes directory remains.

3. **Pre-Merge Verification**
   - Re-run `verify_archive_completion` immediately before `attempt_merge`.
   - Return `MergeDeferred` and keep in `MergeWait` if not archived.

## Expected Benefits

- Prevent false positives in archive completion detection and immediately detect change resurrection.
- Block unarchived changes before merge execution, clarifying the cause of `MergeWait`.

## Risks

- Error detection occurs earlier in abnormal flows, potentially increasing log output.
- If existing manual operations temporarily retained the changes directory, operational procedures may need review.

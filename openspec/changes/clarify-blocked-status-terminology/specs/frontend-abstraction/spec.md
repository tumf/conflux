## MODIFIED Requirements

### Requirement: Core / Frontend 状態所有の境界

Core が所有する display status の正規ソースは、dependency wait の `blocked`（canonical concept: `dependency-blocked`）、apply/rejecting resumable hold の `stalled`、acceptance gate observation の `gated`（canonical concept: `acceptance-gated`）を区別しなければならない（MUST）。

Frontend はこれらを独自の lifecycle copy や render-time simplification によって単一の `blocked` へ collapse してはならない（MUST NOT）。

#### Scenario: Frontend keeps blocked, stalled, and gated distinct
- **GIVEN** Core が 3 種類の blocker-adjacent display status を提供している
- **WHEN** TUI または Web UI が change row / API payload / status badge を描画する
- **THEN** dependency wait は `blocked` として表示される
- **AND** apply-side resumable hold は `stalled` として表示される
- **AND** acceptance gate observation は `gated` として表示または配信される
- **AND** Frontend はそれらを単一の `blocked` 値へ変換しない

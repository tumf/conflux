## REMOVED Requirements

### Requirement: サーバ設定セクション

The obsolete standalone server configuration is removed. Local web monitoring remains configured by its `web` section and `--web*` options.

#### Scenario: No server configuration section

**Given**: Current configuration parsing
**When**: supported top-level sections are inspected
**Then**: No standalone multi-project `server` section is exposed

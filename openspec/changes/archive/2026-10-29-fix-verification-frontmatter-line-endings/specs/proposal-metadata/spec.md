## ADDED Requirements

### Requirement: Frontmatter line-ending equivalence

Conflux MUST recognize a proposal's opening delimiter line `---` at the beginning of the file when terminated by LF or CRLF. A closing delimiter MUST be an exact `---` line terminated by LF, CRLF, or EOF. LF and CRLF MAY be mixed between lines. An empty block MUST be recognized. A recognized opening delimiter without a recognized closing delimiter MUST be invalid rather than absent. No BOM stripping, whitespace-delimiter relaxation or lone-CR delimiter support is introduced.

Execution verification reading, strict validation metadata, display metadata and strict dependency parsing MUST share this delimiter classification. Caller-specific tolerant display behavior MUST NOT become gate authority. The verification reader MUST preserve the existing typed source defects for malformed opened blocks and invalid decoded shapes; the native Acceptance and CLI failure behavior remains that of Required verification declarations fail closed.

Parsing MUST retain the original YAML/body slices and MUST NOT rewrite the proposal or alter commit/blob/digest inputs. Equivalent parsed declarations across LF and CRLF do not imply identical repository content IDs. Legitimately absent, null and empty verification sources MUST keep their existing semantics. Empty/Null YAML roots MUST produce empty strict/display metadata without decode diagnostics, and strict dependency reading MUST retain the body dependency fallback; this does not waive implementation-proposal required-verification policy. The YAML slice MUST end at the closing line start offset, retaining the complete preceding LF/CRLF terminator; the body slice MUST start after the closing line terminator or at EOF. No new configuration or authoring restriction may bypass these rules.

#### Scenario: CRLF required failing gate is not omitted

- **GIVEN** otherwise equivalent LF and CRLF proposals declaring one repository-local change-blocking verification whose command exits nonzero
- **WHEN** real CLI verification or native candidate Acceptance reads either proposal
- **THEN** exactly that gate is selected and executed, and its failure is retained
- **AND** native Acceptance does not start the reviewer
- **AND** a successful empty verification list is not returned

#### Scenario: Passing CRLF gate preserves positive execution

- **GIVEN** a CRLF or mixed LF/CRLF proposal declaring one valid passing required verification
- **WHEN** the actual CLI or production native boundary runs
- **THEN** the gate runs exactly once and its success is observed
- **AND** native Acceptance may start the reviewer only after the gate passes
- **AND** strict and archive-gate validation recognize the declaration and its task linkage

#### Scenario: Unclosed and wrong-typed CRLF blocks fail closed

- **GIVEN** a recognized CRLF opening block without a closing delimiter, or a closed block with a wrong-typed verification field
- **WHEN** strict/archive validation, verify plan/run, or native Acceptance consumes the proposal
- **THEN** the structural defect is reported through that consumer's existing failure contract
- **AND** verify/native Acceptance execute no declared command or reviewer and do not reuse or write evidence
- **AND** the input is not treated as an absent declaration block

#### Scenario: Candidate changes after admission

- **GIVEN** admission observed valid LF metadata and Apply later produces a CRLF candidate proposal
- **WHEN** the production manifest is derived from the candidate
- **THEN** candidate declarations and defects are derived from the actual candidate bytes
- **AND** earlier base admission cannot substitute for current gate selection

#### Scenario: Empty blocks and exact content identity

- **GIVEN** an empty block or a closing delimiter at EOF using supported line endings
- **WHEN** the shared scanner and verification reader run
- **THEN** the block is recognized without modifying original bytes
- **AND** legitimately empty verifications retain zero defects
- **AND** strict/display metadata decoding yields no decode diagnostics for an empty/Null root and strict dependencies retain body fallback
- **AND** independent implementation-proposal required-verification policy is not waived
- **AND** distinct raw LF/CRLF proposal blobs retain their actual distinct content identities

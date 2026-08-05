## ADDED Requirements

### Requirement: CLI OpenAPI schema export

The CLI MUST provide `cflx openapi` as a read-only command that emits the build's complete OpenAPI 3.1 YAML document to standard output. It MUST use the same generated contract source as the live `/api/v2/openapi.yaml` endpoint, MUST NOT require a Git repository, and MUST NOT start logging, listeners, lifecycle adapters, AI subprocesses, or orchestration. Standard output MUST contain only the schema so shell redirection produces a valid standalone document. Diagnostics MUST use standard error and failures MUST exit non-zero.

#### Scenario: Export schema without a repository

**Given**: `cflx` is built with OpenAPI support and the current directory is not a Git repository
**When**: the operator runs `cflx openapi`
**Then**: the command exits successfully
**And**: stdout parses as a complete OpenAPI 3.1 YAML document
**And**: no repository lock or orchestration service is started

#### Scenario: Redirect schema to a file

**Given**: `cflx` is built with OpenAPI support
**When**: the operator runs `cflx openapi > openapi.yaml`
**Then**: `openapi.yaml` contains only the generated schema
**And**: the document matches the contract served by `/api/v2/openapi.yaml` from the same build

#### Scenario: OpenAPI support is unavailable

**Given**: `cflx` is built without the feature that provides the OpenAPI document
**When**: the operator invokes `cflx openapi`
**Then**: the command exits non-zero
**And**: stderr explains that OpenAPI support is unavailable
**And**: stdout contains no partial schema

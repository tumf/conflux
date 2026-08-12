## Specification Tasks

- [ ] Promote complete combined client requirements to `openspec/specs/cli/spec.md`. Expected canonical result: all original client scenarios and all correction scenarios coexist.
- [ ] Promote complete combined compatibility requirement to `openspec/specs/remote-control-api/spec.md`. Expected canonical result: original capability/revision scenarios and bearer-token scenarios coexist.
- [ ] Verify scenario preservation against both archived changes. Expected canonical result: no scenario heading from either source delta is lost.

## Final Validation

Archive validation is authoritative. Expected gate: `cflx openspec validate fix-client-cli-spec-preservation --archive-gate`.

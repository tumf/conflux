# Tasks: Implement Release Workflow

## Phase 1: Configuration Files

- [ ] Create `cliff.toml` for changelog generation
  - Configure commit parsers for conventional and natural language
  - Set up header/body/footer templates
  - Define skip patterns for release/merge commits

- [ ] Create `dist-workspace.toml` for cargo-dist
  - Set cargo-dist version (0.30.3)
  - Configure target platforms (macOS, Linux, Windows)
  - Enable installers (shell, powershell, homebrew)
  - Set Homebrew tap (tumf/homebrew-tap)

## Phase 2: Release Script

- [ ] Create `scripts/release.sh`
  - Add environment validation (branch, clean tree, tools)
  - Implement version calculation (patch/minor/major)
  - Add pre-release checks (fmt, clippy, test)
  - Implement version bump in Cargo.toml
  - Add changelog generation with git-cliff
  - Implement git commit, tag, and push

- [ ] Make script executable and test locally
  - Verify patch version bump works
  - Verify changelog generation

## Phase 3: GitHub Actions

- [ ] Create `.github/workflows/release.yml`
  - Configure trigger on version tags
  - Set up plan job for build matrix
  - Configure build-local-artifacts job
  - Configure build-global-artifacts job
  - Set up host job for GitHub Release
  - Add publish-homebrew-formula job

## Phase 4: Documentation

- [ ] Create `RELEASE.md`
  - Document prerequisites (git-cliff, cargo-release)
  - Explain quick release method (script usage)
  - Document manual release process
  - Add troubleshooting guide
  - Include version numbering guide

## Phase 5: Homebrew Setup

- [ ] Verify tumf/homebrew-tap repository exists
  - Create if needed with proper structure

- [ ] Configure HOMEBREW_TAP_TOKEN secret
  - Generate personal access token with repo scope
  - Add to repository secrets

## Phase 6: Validation

- [ ] Test release workflow end-to-end
  - Create test release (v0.1.1 or pre-release)
  - Verify GitHub Release is created
  - Verify binaries are built for all platforms
  - Verify Homebrew formula is updated

- [ ] Verify installation methods
  - Test shell installer script
  - Test Homebrew installation

## Dependencies

```
Phase 1 ──┬──▶ Phase 2 ──▶ Phase 3 ──▶ Phase 6
          │
          └──▶ Phase 4

Phase 5 (parallel, required for Phase 6)
```

## Notes

- Phases 1, 4, 5 can be worked in parallel
- Phase 3 depends on Phase 1 (dist-workspace.toml)
- Phase 6 requires all other phases complete
- First release should be a patch (0.1.1) to test workflow

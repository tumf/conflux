# Tasks: Implement Release Workflow

## Phase 1: Configuration Files

- [x] Create `cliff.toml` for changelog generation
  - Configure commit parsers for conventional and natural language
  - Set up header/body/footer templates
  - Define skip patterns for release/merge commits

- [x] Create `dist-workspace.toml` for cargo-dist
  - Set cargo-dist version (0.30.3)
  - Configure target platforms (macOS, Linux, Windows)
  - Enable installers (shell, powershell, homebrew)
  - Set Homebrew tap (tumf/homebrew-tap)

## Phase 2: Release Script

- [x] Create `scripts/release.sh`
  - Add environment validation (branch, clean tree, tools)
  - Implement version calculation (patch/minor/major)
  - Add pre-release checks (fmt, clippy, test)
  - Implement version bump in Cargo.toml
  - Add changelog generation with git-cliff
  - Implement git commit, tag, and push

## Phase 3: GitHub Actions

- [x] Create `.github/workflows/release.yml`
  - Configure trigger on version tags
  - Set up plan job for build matrix
  - Configure build-local-artifacts job
  - Configure build-global-artifacts job
  - Set up host job for GitHub Release
  - Add publish-homebrew-formula job

## Phase 4: Documentation

- [x] Create `RELEASE.md`
  - Document prerequisites (git-cliff, cargo-release)
  - Explain quick release method (script usage)
  - Document manual release process
  - Add troubleshooting guide
  - Include version numbering guide

## Notes

- Phases 1-4 complete all in-scope implementation work
- Homebrew tap setup and release validation are out of scope (require user action)
- First release should be a patch (0.1.1) to test workflow

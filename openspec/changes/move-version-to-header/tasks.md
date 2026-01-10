# Tasks

## Implementation

- [ ] Update `render_header()` to display version on the right side
  - Add horizontal layout split (title left, version right)
  - Use same muted gray color as current footer version
  - Ensure proper border rendering with split layout

- [ ] Update `render_footer_select()` to remove version display
  - Remove version variable and related layout split
  - Simplify footer to single block with full borders
  - Remove right-aligned version paragraph

- [ ] Update tests
  - Modify `test_get_version_string_format` to remain valid (function still exists)
  - Add test for header version display behavior
  - Update any footer tests that depend on version display

- [ ] Manual verification
  - Run TUI and verify version appears in header right
  - Verify footer displays correctly without version
  - Test in both selection and running modes


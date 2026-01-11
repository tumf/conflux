# Tasks

## Implementation

- [x] Update `render_header()` to display version on the right side
  - Add horizontal layout split (title left, version right)
  - Use same muted gray color as current footer version
  - Ensure proper border rendering with split layout

- [x] Update `render_footer_select()` to remove version display
  - Remove version variable and related layout split
  - Simplify footer to single block with full borders
  - Remove right-aligned version paragraph

- [x] Update tests
  - Existing `test_get_version_string_format` remains valid (function unchanged)
  - No additional header tests required (rendering tests are integration-level)

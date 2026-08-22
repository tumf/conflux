# Design

## Timeout representation

Represent the operation deadline as optional rather than using a sentinel `Duration::ZERO` after parsing. The CLI accepts `0` as the user-facing sentinel, but wait internals must make bounded and unbounded control flow explicit.

## Deadline behavior

For a positive timeout, preserve the current single monotonic operation deadline across observation, event recovery, repository classification, and child Git work.

For zero or omission, do not create an overall operation deadline. Continue using existing per-request transport limits and bounded subprocess execution so an unresponsive socket or Git process cannot leak forever. After a recoverable inner timeout, the unbounded observer may retry; it must not translate an inner safety bound into the operation-level `timeout` outcome reserved for explicit positive deadlines.

## Compatibility

Positive timeout syntax and outcomes are unchanged. Zero changes from a parse error to the documented unbounded sentinel. Completion evidence and all mutation prohibitions remain unchanged.

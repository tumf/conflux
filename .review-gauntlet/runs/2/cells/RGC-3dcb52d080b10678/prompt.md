# review-gauntlet OCR Review Prompt

You are an external code review CLI. Produce verdict JSON matching the contract.
Do not include provider credentials or markdown fences in the verdict JSON.

## Output Contract
Write the final verdict JSON to this file:
/Users/tumf/work/conflux/.review-gauntlet/runs/2/cells/RGC-3dcb52d080b10678/verdict.json
Before finishing, validate the file with:
review-gauntlet validate-verdict /Users/tumf/work/conflux/.review-gauntlet/runs/2/cells/RGC-3dcb52d080b10678/verdict.json --expected-path .cflx.jsonc
If validation fails, fix the JSON file and run the validator again.
Stdout and stderr are audit/progress channels only; they are preserved but not parsed as verdict input.
Do not rely on stdout or stderr to deliver the verdict when file-json output is active.

## Repository Context
repository_root: /Users/tumf/work/conflux
ruleset_digest: 8f6075fa8b3d8ad3606e0f850277b73de14cf786552139cd817a8bd18d34477c

## Review Cell
cell_id: RGC-3dcb52d080b10678
file_path: .cflx.jsonc
slice_id: other
rule_id: cli-contract
content_digest: b308de374d5f98fe8c76c47f16aa279d7910523cadc5e997d9b51f4e7766f220
file_size_bytes: 618
line_count: 24

Source file contents are not embedded in this prompt. When source inspection is needed, read the target file from repository_root plus file_path.

## Review Scope Guardrails
Only report issues whose JSON path exactly equals the Review Cell file_path above.
You may inspect related files to understand context, but do not emit comments for related files or helper files.
If the only issue you find is in a different file, return an empty comments array.
A verdict comment whose path differs from file_path will be rejected by the adapter.

## Selected Rule
rule_document: default.md
# OCR Rule Attribution

Ported review guidance derived from Alibaba open-code-review at commit c323c6b40c72aa95d7cb801bedcb957b52ff9807 (Apache-2.0). Focus on correctness, security, maintainability, and actionable line-level comments.


## Verdict JSON Contract
The verdict object must contain exactly one top-level key: comments.
Each comment object may contain only these keys: path, content, suggestion_code, existing_code, start_line, end_line, thinking.
Do not include rule_id, cell_id, severity, confidence, title, category, metadata, or any other keys.
Every comment.path MUST equal: .cflx.jsonc
{
  "comments": [
    {
      "content": "Issue description for this exact review cell path only",
      "end_line": 1,
      "existing_code": "Existing code",
      "path": ".cflx.jsonc",
      "start_line": 1,
      "suggestion_code": "Suggested code",
      "thinking": "Optional reasoning"
    }
  ]
}

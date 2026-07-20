# Design: Current acceptance follow-up and latest-only prompt

## Context

Acceptanceはread-only reviewerであり、runtimeがfollow-up persistenceを所有する。現在はnumbered headingを再生成し、raw/history contextを複数経路からpromptへ渡すため、agent tokenとtask stateがattempt履歴へ引きずられる。

## Decision

### Follow-up ownership

Runtimeは`## Current Acceptance Follow-up`を一つだけupsertする。Legacy `## Acceptance #N Failure Follow-up` sectionsをruntime-owned patternとして認識し、次回writeで置換する。Original implementation tasksや未知sectionは変更しない。

Normalized repository findingsをstable identityでsort/dedupする。最新FAIL payloadに再出現したidentityは未解決のため必ずuncheckedへ戻す。Payloadから消えたobsolete identityは除去する。External blockersはcheckboxにせず、evidenceとnext actionをmetadataとしてrenderする。

### Prompt context

Prompt builderはcurrent diffとlatest normalized findingsを一つのauthoritative context blockへまとめる。Finalized FAIL finding payloadがあればraw outputを省く。CONTINUE、parser failure、command diagnosticsではbounded latest raw outputをfallbackとして含める。全attempt historyはworkflow observabilityには保持できるがprompt inputにはしない。

### Guidance

Canonical agent prompt requirementをread-onlyへ変更し、bundled `cflx-accept` variantsとcompatibility referencesを同期する。Runtime-owned sectionの編集をacceptance agentへ要求しない。

## Alternatives Rejected

- Numbered sections維持: historyがtask stateへ蓄積する。
- Raw outputとnormalized findings併記:同じfindingを重複注入する。
- History削除: observability用途まで失うため不要。
- Acceptance agentによるtasks編集: read-only review contractに反する。

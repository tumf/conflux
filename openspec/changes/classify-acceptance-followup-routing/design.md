# Design: acceptance follow-up の phase-aware routing

## Goal

`tasks.md` の総 checkbox 数ではなく、**次にどの phase が進めるべきか** を判断できる follow-up model を導入し、acceptance / archive blocker が apply stall を誘発しないようにする。

## Current Failure Shape

現行 runtime は acceptance failure を `## Acceptance #<n> Failure Follow-up` の unchecked checkbox に正規化し、その後の resume routing で `completed < total` を見て `Apply` を強制する。

この方式では次の 2 種類が区別できない。

1. **Apply-driving remediation**
   - 実装差分、UI修正、テスト追加、repo 内 wiring 変更が必要
2. **Blocker-only follow-up**
   - archive readiness blocker
   - commit-path blocker
   - external unblock / dependency resolve
   - その周回の apply だけでは差分を作れない hold reason

結果として blocker-only case でも apply が再実行され、空 WIP snapshot → stall detector へつながる。

## Proposed Model

### 1. Follow-up section を 2 種に分ける

`Acceptance #<n> Failure Follow-up` の中で、少なくとも次の 2 種を表現する。

- **Remediation task**: unchecked checkbox (`- [ ] ...`)
- **Blocker note**: non-checkbox bullet または明示マーカー付き note

runtime が apply routing に使うのは remediation task だけとする。

### 2. task_parser に apply-routing 用 view を追加する

既存の raw progress は維持しつつ、resume/apply routing 用には別 view を返す。

例:
- `overall_progress`: 現行互換の total/completed
- `apply_routing_progress`: implementation + remediation checkbox のみ
- `follow_up_summary`: remediation_count / blocker_count / latest_attempt

この分離により UI progress 表示と phase routing を同じ数値へ縛り付けなくてよくなる。

### 3. Routing rules

resumed workspace が `Applied` / `Archiving` 相当のとき:

- apply-driving remediation がある → `Apply`
- remediation はなく blocker-only follow-up だけある → `Blocked` or non-apply hold
- follow-up なし、acceptance pass durable state なし → `Acceptance`
- archive-complete → downstream archive/merge handling

acceptance fail の immediate reroute でも同じ分類を使う。

### 4. blocked / rejected judgment guideline

follow-up classification だけでなく、blocked と rejected の境界も runtime / spec / prompt で同じ語彙にそろえる。

- **Blocked**: change の妥当性は維持されており、環境修復・依存解消・追加情報・human approval・commit-path repair により再開可能
- **Rejected**: change 自体を閉じる判断が妥当であり、前提破綻・superseded・scope closure・継続価値消失などにより resume より closure を選ぶべき

特に `No space left on device`、ローカル CI/検証不能、archive readiness blocker、commit-path blocker のようなケースは temporary blocker として `Blocked` に入るべきであり、`Rejected` にしてはならない。

## Dependency on blocked lifecycle

この change は `separate-apply-block-from-reject` に依存する。理由は、blocker-only follow-up を `Apply` 以外へ送る先として resumable `Blocked` lifecycle が必要だからである。

もし dependency が未導入の段階で先行実装する場合は、temporary fallback として explicit retry required error を返す設計もありうるが、本 proposal では canonical path としない。

## Logging / Observability

少なくとも次の文言を分ける。

- `forcing apply because implementation remediation tasks are incomplete`
- `holding change because blocker-only follow-up remains`

これにより「implementation tasks incomplete」という誤解を避ける。

## Regression Shape

再現 fixture は次を満たす。

- `Implementation Tasks` は 100% 完了
- `Acceptance #1 Failure Follow-up` に archive/commit-path blocker のみ存在
- resumed workspace は `Apply` を選ばない
- empty WIP stall detector が走らない

別 fixture として:

- `Acceptance #1 Failure Follow-up` に UI bug 修正の unchecked checkbox がある
- resumed workspace は `Apply` を選ぶ

を置き、分類精度を担保する。

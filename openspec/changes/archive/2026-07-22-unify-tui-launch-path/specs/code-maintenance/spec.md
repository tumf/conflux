## ADDED Requirements

### Requirement: TUI entrypoints share one launch implementation

bare `cflx` と明示的な `cflx tui` は、parse後のTUI起動処理を単一の実装へ委譲しなければならない。共通化は引数validation、config load、change source、web monitoring、remote client、logging、exit behaviorを変更してはならない。

#### Scenario: Bare and explicit entrypoints use the same launch path

**Given**: bare entrypointとexplicit `tui` entrypointに同等のTUI optionsが渡される
**When**: CLIがTUI起動をdispatchする
**Then**: 両entrypointは同じTUI launch helperを呼ぶ
**And**: `run_tui_with_remote` へ渡される設定の意味はリファクタ前と同等である

#### Scenario: Invalid option combination is rejected consistently

**Given**: local-only post-archive pushとremote server modeが同時に指定される
**When**: bareまたはexplicit entrypointからTUIを起動する
**Then**: 両entrypointはTUI初期化前に失敗する
**And**: exit behaviorと利用者向けerror semanticsはリファクタ前と同等である

#### Scenario: Feature-gated web monitoring behavior is preserved

**Given**: web monitoring optionが指定されている
**When**: web-monitoring feature有効または無効のbinaryでTUIを起動する
**Then**: feature有効時は現在と同じserver起動とfailure fallbackを行う
**And**: feature無効時は現在と同じwarningを出してTUI起動を継続する

#### Scenario: Remote and local change sources remain distinct

**Given**: remote server endpointが指定される場合と指定されない場合がある
**When**: 共通launch helperがinitial changesを取得する
**Then**: remote modeはserverから取得し、local workspace changesを読み込まない
**And**: local modeは現在と同じnative OpenSpec listingを使用する

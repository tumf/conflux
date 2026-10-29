# Design: targeted hidden-state attention visibility

## Authority and approach
Baseline 148e5612b3a7c062624f65aefe0a75ef5905c2b4 shipped web/style.css, web/index.html and web/app.js govern all existing design. Add `.attention[hidden] { display: none; }` immediately after `.attention`. Do not change live flex appearance or JavaScript clearing behavior. A global [hidden] rule would affect unreviewed unrelated surfaces and is rejected. Clearing text alone leaves a styled empty warning box and is rejected.

## Required state inventory / owners
All rows use renderCurrentStatus -> attention-summary / attention-text, the same HTML and style rule. Each row must be exercised with shipped styles enabled, through production console state updates; no duplicated renderer. Missing coverage fails assertions.
- A: initial coherent snapshot with zero attention rows: hidden true and computed display none.
- B: one current attention/error row: hidden false, computed display flex, current name present.
- C: B -> zero rows after recovery/archive, same console: hidden true and computed display none even if old text is retained internally.
- D: B -> non-attention row: hidden true and computed display none; E: C/D -> a new attention row: visible flex and only current names in the visible summary.

## Geometry / accessibility / responsive contract
For hidden A/C/D there is no rendered attention box; display none removes layout/accessible display rather than making text transparent or offscreen. For B/E all anatomy and values remain exactly baseline: flex, align-items:flex-start, gap space-2, padding space-3, margin-bottom space-4, semantic attention color/border, 4px left border, radius and weight600. No action priority, focus target, keyboard, aria, theme, locale or reduced-motion change. The selector is viewport-independent and introduces no breakpoint; existing responsive/contrast tests remain selected. Do not claim pixel evidence from jsdom. Parent browser follow-up owns actual desktop/narrow visibility and appearance, not a new Apply gate.

## Verification
Use tests/web/helpers/console.js loadDocument({withStyles:true}) and real console snapshot/recovery path. At least four tests cover A-E, including one combined C/D/E transition test if desired. Assert getComputedStyle(...).display for each required state and fixture command count zero; boolean hidden alone is insufficient. No new screenshot runner or dependency. The bounded declaration in proposal.md runs exactly one selected suite command. Preserve old failing assertion output and corrected passing output when authoring the tests.

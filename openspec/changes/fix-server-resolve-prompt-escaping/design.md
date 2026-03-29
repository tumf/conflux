## Context
- server mode の `git/sync` は `resolve_command` 実行を必須化している
- orchestrator 側の apply/analyze/archive/acceptance は `src/config/expand.rs` の共通 placeholder 展開を使うが、server sync の resolve 実行だけ独自実装になっている
- 既存ユーザ設定では `"... '{prompt}' ..."` 形式が広く使われており、互換維持が重要である

## Goals / Non-Goals
- Goals:
  - server mode の `resolve_command` 展開規則を他コマンドと一致させる
  - 既存の `'{prompt}'` テンプレート互換を維持する
  - multi-line prompt の回帰をテストで固定する
- Non-Goals:
  - shell 実行モデル全体の置換
  - server 起動環境の PATH 改善

## Decisions
- Decision: `src/server/api.rs` の `run_resolve_command()` は独自の quoting 実装をやめ、既存の共通 placeholder 展開を利用する
  - Alternatives considered: server 側だけテンプレートからクォートを禁止する
  - Rationale: 既存設定との互換を壊さず、spec `shell-escaping` と挙動を統一できるため
- Decision: 修正は最小スコープに留め、shell 非経由実行への移行は別件とする
  - Alternatives considered: ここで argv 実行へ全面移行する
  - Rationale: 互換性と修正範囲を抑え、回帰原因を直接潰せるため

## Risks / Trade-offs
- 共通展開の利用方法を誤ると、server mode と orchestrator mode で再び挙動差が生じる
  - Mitigation: `'{prompt}'` / `{prompt}` / multi-line prompt の回帰テストを追加する
- shell 実行を維持するため、PATH や shell 初期化の問題は別途残り得る
  - Mitigation: 今回の proposal では scope 外と明記する

## Migration Plan
1. server resolve 実行経路を共通展開へ寄せる
2. 既存テンプレート互換テストを追加する
3. strict validate で spec 整合性を確認する

## Open Questions
- なし（現時点では最小修正で十分）

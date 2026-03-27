## Context

並列実行では archive 完了後の merge が失敗すると `MergeDeferred` が返り、現在は主に `MergeWait` として扱われる。だが、実際には以下の 2 種類が混在している。

- すぐには base へ入れられないが、先行 merge / resolve が終われば自動で再開できる待機
- ユーザーが明示的に競合解消しない限り進められない待機

この 2 つを同じ `MergeWait` に押し込めると、scheduler は再試行根拠を失い、TUI も「手動 `M` 待ち」に見えるまま残る。

## Goals / Non-Goals

- Goals:
  - `MergeDeferred` 後の待機理由を状態として区別する
  - 先行 merge / resolve 完了をトリガーに deferred change を再評価できるようにする
  - TUI / reducer / scheduler で待機状態の意味を一致させる
- Non-Goals:
  - resolve コマンド自体の全面的な再設計
  - merge conflict 解消アルゴリズムの高機能化

## Decisions

- Decision: `MergeWait` は手動介入専用とし、自動再開可能な deferred change は reducer と scheduler が再評価対象として保持する
  - Alternatives considered:
    - 既存の `MergeWait` を流用する: 表示上も手動待機に見え、再評価契機を表現しにくい
    - 常に `ResolveWait` へ昇格する: 真に手動介入が必要なケースまで自動処理対象に見えてしまう

- Decision: 先行 merge / resolve 完了イベントを deferred change 再評価の明示トリガーにする
  - Alternatives considered:
    - refresh 観測だけで復元する: durable state から理由分類を復元できず、race が残る
    - ユーザーの `M` のみで再開する: 今回の stuck UX を解消できない

- Decision: 仕様変更は `parallel-execution` `orchestration-state` `tui-architecture` の 3 capability にまたがって記述する
  - Alternatives considered:
    - `parallel-execution` のみに閉じる: reducer/TUI の責務境界が曖昧になる

## Risks / Trade-offs

- 待機状態の細分化で reducer 遷移が複雑になる
  - Mitigation: terminal/active/wait の優先順位を spec とテストで固定する
- 自動再評価が過剰に走ると不要な resolve 試行が増える
  - Mitigation: 再評価対象を `MergeDeferred` 理由分類済みの change に限定する

## Migration Plan

1. spec で待機理由と再評価契機を定義する
2. reducer に新しい待機意味論を導入する
3. parallel scheduler で merge/resolve 完了後の再評価を接続する
4. TUI 表示と回帰テストを更新する

## Open Questions

- 自動再評価対象を新しい wait state 名で表現するか、既存 `ResolveWait` に寄せるかは実装時に最小差分で決める

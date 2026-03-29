## ADDED Requirements

### Requirement: dashboard は active command に基づいて project/worktree 操作を表示する
server-mode dashboard は server から受信した active command 状態を真実源として、project と worktree の操作ボタンの有効/無効および進行表示を決定しなければならない（MUST）。

#### Scenario: sync 中の project card が disable される
**Given** dashboard がある project の base root に対する `operation=sync` の active command を受信している
**When** ProjectsPanel がその project を描画する
**Then** Sync ボタンは disabled で表示される
**And** ボタンラベルまたは表示は Sync 実行中であることを示す

#### Scenario: worktree busy 状態が worktree row に反映される
**Given** dashboard がある worktree root に対する active command を受信している
**When** WorktreesPanel がその worktree を描画する
**Then** 対応する merge や delete などの競合操作ボタンは disabled で表示される

#### Scenario: browser reload restores active command state
**Given** dashboard を開いている間に base または worktree root で command が実行中である
**When** ユーザーがブラウザをリロードする
**Then** 初回 state 読み込みまたは WebSocket `full_state` に active command 状態が含まれる
**And** UI はリロード前と同じ busy 表示を再現する

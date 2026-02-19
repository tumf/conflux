# Change: serverモードTUIのChangesをプロジェクト別にグルーピング表示する

## Why

serverモードのChanges一覧は現在、各行にプロジェクト識別子が繰り返し表示され、一覧の視認性と比較のしやすさが低下しています。プロジェクト単位で見出しを分けることで、複数プロジェクトの進行状況を素早く把握できるようにします。

## What Changes

- serverモードのChanges一覧をプロジェクト見出しでグルーピング表示する
- change行はプロジェクト名を重複表示せず、change_idのみを表示する
- 見出し行は選択・操作対象にせず、カーソル移動と操作はchange行のみを対象にする
- 既存のログプレビュー表示はグルーピング後の表示幅に合わせて維持する

## Impact

- Affected specs: tui-architecture
- Affected code: src/tui/render.rs, src/tui/state.rs

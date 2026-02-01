## ADDED Requirements
### Requirement: Running Changes一覧の経過時間配置
TUIのRunningモードにおけるChanges一覧は、in-flight状態（Applying/Accepting/Archiving/Resolving）の行で、動作中スピナーの直後に経過時間を表示しなければならない（SHALL）。経過時間はステータス表示の前に配置しなければならない（SHALL）。

#### Scenario: in-flight行でスピナー直後に経過時間を表示する
- **GIVEN** TUIがRunningモードである
- **AND** changeのqueue_statusがApplyingである
- **AND** changeの開始時刻が取得できる
- **WHEN** TUIがChanges一覧を描画する
- **THEN** change行の表示はスピナーの直後に経過時間を含む
- **AND** 経過時間はステータス表示の前に配置される

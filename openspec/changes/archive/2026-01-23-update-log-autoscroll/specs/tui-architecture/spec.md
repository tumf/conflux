## MODIFIED Requirements
### Requirement: Log Entry Structure and Display

TUIのログエントリーは、タイムスタンプ、メッセージ、色に加えて、オプションのコンテキスト情報（change ID、オペレーション、イテレーション番号）を含まなければならない（SHALL）。
ログヘッダーは、利用可能なコンテキスト情報に基づいて段階的に表示される。

- archive のログ出力は常にイテレーション番号を含み、ログヘッダーは `[{change_id}:archive:{iteration}]` 形式で表示されなければならない（MUST）。
- change_id のない analysis ログ出力は常にイテレーション番号を含み、ログヘッダーは `[analysis:{iteration}]` 形式で表示されなければならない（MUST）。
- ログの自動スクロールが無効な場合、TUIはユーザーが見ているログ範囲を保持し、新しいログ追加やログバッファのトリムで表示中の行が移動してはならない（MUST NOT）。表示中の行がトリムされた場合は残存ログの最古行へクランプし、オートスクロールを再有効化してはならない（MUST NOT）。

#### Scenario: archiveログは常にイテレーション付きで表示される
- **GIVEN** ログエントリーが `change_id="test-change"`, `operation="archive"`, `iteration=2` で作成される
- **WHEN** TUIがログをレンダリングする
- **THEN** ログヘッダーは `[test-change:archive:2]` と表示される
- **AND** リトライの順序が判別できる

#### Scenario: analysisログはイテレーション付きで表示される
- **GIVEN** ログエントリーが `change_id=None`, `operation="analysis"`, `iteration=3` で作成される
- **WHEN** TUIがログをレンダリングする
- **THEN** ログヘッダーは `[analysis:3]` と表示される
- **AND** 解析の再実行が区別できる

#### Scenario: オートスクロール無効時の表示固定
- **GIVEN** ユーザーがログをスクロールしてオートスクロールが無効になっている
- **WHEN** 新しいログが追加される（必要に応じて古いログがトリムされる）
- **THEN** 既存の表示範囲は同じログ行を指し続ける
- **AND** 表示範囲がトリムされた場合は最古の残存ログ行へクランプされる
- **AND** オートスクロールは自動で再有効化されない

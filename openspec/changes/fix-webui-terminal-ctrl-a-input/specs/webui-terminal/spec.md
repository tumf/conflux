## MODIFIED Requirements

### Requirement: terminal-keyboard-input

WebUI の仮想ターミナル (`TerminalTab`) は、ターミナルにフォーカスがある状態でシェル操作キーバインドのブラウザデフォルト動作を抑制し、キー入力を PTY に正しく転送する。

#### Scenario: ctrl-a-moves-to-beginning-of-line

**Given**: WebUI ターミナルにフォーカスがあり、コマンドラインにテキストが入力されている
**When**: ユーザーが Ctrl+A を押す
**Then**: ブラウザの全選択は発火せず、シェルのカーソルが行頭に移動し、以降のキー入力が正常に動作する

#### Scenario: ctrl-c-copies-when-selection-exists

**Given**: WebUI ターミナルにフォーカスがあり、テキストが選択されている
**When**: ユーザーが Ctrl+C を押す
**Then**: 選択テキストがクリップボードにコピーされ、SIGINT は送信されない

#### Scenario: ctrl-c-sends-sigint-without-selection

**Given**: WebUI ターミナルにフォーカスがあり、テキストが選択されていない
**When**: ユーザーが Ctrl+C を押す
**Then**: PTY に SIGINT (0x03) が送信される

#### Scenario: shell-keybindings-reach-pty

**Given**: WebUI ターミナルにフォーカスがある
**When**: ユーザーが Ctrl+E, Ctrl+K, Ctrl+U, Ctrl+L, Ctrl+R, Ctrl+D, Ctrl+W のいずれかを押す
**Then**: 対応する制御コードが PTY に送信され、ブラウザのデフォルト動作は発火しない

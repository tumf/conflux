## MODIFIED Requirements

### Requirement: terminal-keyboard-input

WebUI の仮想ターミナル (`TerminalTab`) は、ターミナルにフォーカスがある状態でシェル操作キーバインドのブラウザデフォルト動作を抑制し、キー入力を PTY に正しく転送する。さらに、制御入力の後に xterm.js の hidden helper textarea に stale text が残留しても、後続の printable input で以前の入力内容を再送しないようにしなければならない。

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

#### Scenario: stale-helper-textarea-does-not-replay-previous-input

**Given**: WebUI ターミナルで printable text を入力した直後に Ctrl+A などの制御入力を送信し、xterm.js の hidden helper textarea に直前の文字列が一時的に残留している
**When**: ユーザーが次の printable key を入力する
**Then**: 直前の文字列全体は再送・再表示されず、新しいキー入力だけが PTY に反映される

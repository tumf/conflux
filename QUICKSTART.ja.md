# QUICKSTART

[![日本語](https://img.shields.io/badge/%E8%A8%80%E8%AA%9E-日本語-0f766e?style=flat-square)](./QUICKSTART.ja.md)
[![English](https://img.shields.io/badge/Language-English-2563eb?style=flat-square)](./QUICKSTART.md)
[![简体中文](https://img.shields.io/badge/%E8%AF%AD%E8%A8%80-%E7%AE%80%E4%BD%93%E4%B8%AD%E6%96%87-dc2626?style=flat-square)](./QUICKSTART.zh-CN.md)
[![Español](https://img.shields.io/badge/Idioma-Espa%C3%B1ol-f59e0b?style=flat-square)](./QUICKSTART.es.md)
[![Português (BR)](https://img.shields.io/badge/Idioma-Portugu%C3%AAs%20(BR)-16a34a?style=flat-square)](./QUICKSTART.pt-BR.md)
[![한국어](https://img.shields.io/badge/%EC%96%B8%EC%96%B4-%ED%95%9C%EA%B5%AD%EC%96%B4-7c3aed?style=flat-square)](./QUICKSTART.ko.md)
[![Français](https://img.shields.io/badge/Langue-Fran%C3%A7ais-0891b2?style=flat-square)](./QUICKSTART.fr.md)
[![Deutsch](https://img.shields.io/badge/Sprache-Deutsch-4b5563?style=flat-square)](./QUICKSTART.de.md)
[![Русский](https://img.shields.io/badge/%D0%AF%D0%B7%D1%8B%D0%BA-%D0%A0%D1%83%D1%81%D1%81%D0%BA%D0%B8%D0%B9-b91c1c?style=flat-square)](./QUICKSTART.ru.md)
[![Tiếng Việt](https://img.shields.io/badge/Ng%C3%B4n%20ng%E1%BB%AF-Ti%E1%BA%BFng%20Vi%E1%BB%87t-ea580c?style=flat-square)](./QUICKSTART.vi.md)

はじめて `cflx` をインストールし、プロジェクトを設定し、OpenSpec の change を作って TUI で実装を完走するまでの最短ガイドです。

Conflux はコマンド名 `cflx` で実装されています。

## 0. 前提

- Rust / Cargo が使える: [Rustインストール](https://rust-lang.org/ja/tools/install/)
- [Claude Code](https://claude.com/product/claude-code) が使える
- `~/myproject` のような git 管理されたプロジェクトがある

> Conflux は AI エージェントを起動・制御するオーケストレーターです。自身がコーディングエージェントではありません。
> [Claude Code](https://claude.com/product/claude-code)、[OpenCode](https://opencode.ai/)、[Codex](https://developers.openai.com/codex/cli) などの CLI を利用できます。
> この QUICKSTART では Claude Code を例に説明します。

前提条件の確認:

```bash
cargo --version
claude --version
claude -p 'hi'
```

## 1. `cflx` をインストール

crates.io からインストールします。

```bash
cargo install cflx
```

インストール後の確認:

```bash
cflx --version
```

## 2. プロジェクトを用意する

ここから先はプロジェクトディレクトリで作業します。例として `~/myproject` を使います。

Conflux は `git worktree` を利用するので、プロジェクトは git 管理されている必要があります。

```bash
cd ~/myproject
```

新規プロジェクトなら:

```bash
mkdir -p ~/myproject
cd ~/myproject
git init
```

## 3. bundled skills をインストール

Conflux の bundled skill をプロジェクトに入れます。

```bash
cflx install-skills
```

これで `./.agents/skills` 以下に `cflx-*` スキルが入ります。

Git に入れるかどうかは `.cflx.jsonc` とあわせてあとでまとめて決めます。

## 4. 設定ファイルを作る

設定ファイル名は `.cflx.conf` ではなく `.cflx.jsonc` です。

最短はテンプレート生成です。

```bash
cflx init
```

これでカレントディレクトリに `.cflx.jsonc` が作られます。

## 5. `.cflx.jsonc` を確認する

最低限、使いたいエージェント向けのコマンドが入っていれば動きます。

Claude Code テンプレート例:

```jsonc
{
  "analyze_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p {prompt}",
  "apply_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '{prompt}'",
  "archive_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '{prompt}'",
  "acceptance_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '{prompt}'",
  "resolve_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p {prompt}"
}
```

最初の 1 回は `cflx init` が生成した内容をそのまま使えば十分です。

## 6. Git に入れるものを決める

初回セットアップでは、次の 2 つを Git に入れるかどうかを決めます。

- `./.agents/skills/cflx-*`
- `./.cflx.jsonc`

おすすめ:

- チームや複数マシンで同じ挙動を再現したいなら、両方コミットする
- ローカル専用で使い捨てに近いなら、両方 `.gitignore` に入れる

判断に迷ったら、まずは両方コミットで問題ありません。`.cflx.jsonc` に秘密情報を直接書かない運用にしておくと扱いやすいです。

両方 `.gitignore` に追加する場合:

```bash
printf ".agents/skills/cflx-*\n.cflx.jsonc\n" >> .gitignore
git add .gitignore
git commit -m 'Ignore Conflux local setup files'
```

両方リポジトリに追加する場合:

```bash
git add .agents/skills/cflx-* .cflx.jsonc
git commit -m 'Add Conflux setup files'
```

## 7. 最初の change proposal を作る

Conflux は OpenSpec の change を処理します。

OpenSpec にまだ慣れていなくても大丈夫です。bundled skill はすでに入っているので、Claude Code に proposal を作らせられます。

たとえば:

```text
/cflx-proposal python で hello world と表示する
```

すると `openspec/changes/add-hello-world/` のような change ディレクトリが生成され、少なくとも次の 2 つが入ります。

- `proposal.md`: 何を変えるか
- `tasks.md`: 何を実装するか

最短では、この 2 つをざっと確認して問題なければそのままコミットで十分です。

確認ポイント:

- `proposal.md` の内容がやりたい変更になっている
- `tasks.md` の実装タスクが過不足なく並んでいる
- 余計な変更が混ざっていない

必要なら proposal や tasks を修正し、内容に問題がなければコミットします。

```bash
git add openspec/changes/add-hello-world
git commit -m 'proposal: add-hello-world'
```

詳細な構造は次のようになります。

```text
openspec
└── changes
    └── add-hello-world
        ├── proposal.md
        ├── specs
        │   └── hello-world
        │       └── spec.md
        └── tasks.md
```

## 8. ワークスペースがクリーンか確認する

TUI を立ち上げる前に、作業ツリーがクリーンか確認します。

```bash
git status
```

クリーンなら次のようになります。

```text
On branch main
nothing to commit, working tree clean
```

## 9. TUI を起動する

TUI モードで Conflux を起動します。

```bash
cflx
```

以下のような画面が表示されます。

![Conflux TUI ready screen](docs/images/quickstart/tui-ready-screen.png)

## 10. TUI で実行する

基本操作:

- `↑/↓` or `j/k`: change を選ぶ
- `Space`: 実行対象にマーク
- `F5`: 実行開始
- `Ctrl+C`: 終了

最短フロー:

1. `cflx` を起動
2. 処理したい change に移動
3. `Space` で選択
4. `F5` で実行

今回の例では change はひとつなので、`Space` → `F5` で実行します。

![Conflux TUI running screen](docs/images/quickstart/tui-running-screen.png)

Conflux は次のループを自動で回します。

- apply
- accept
- archive
- resolve / merge

`merged` になれば完了です。

![Conflux TUI merged screen](docs/images/quickstart/tui-merged-screen.png)

## 11. 結果を確認する

実装が入っていることを確認します。

```bash
tree
cat hello.py
```

例:

```text
.
├── hello.py
└── openspec
    ├── changes
    └── specs
```

```python
print("hello world")
```

OpenSpec 側も更新されています。

```bash
tree openspec -L 10
```

例:

```text
openspec
├── changes
│   └── archive
│       └── add-hello-world
│           ├── proposal.md
│           ├── specs
│           │   └── hello-world
│           │       └── spec.md
│           └── tasks.md
└── specs
    └── hello-world
        └── spec.md
```

change proposal が archive され、最終的な仕様が `openspec/specs` に昇格しているのがわかります。

たとえば:

```bash
cat openspec/specs/hello-world/spec.md
```

```markdown
## Requirements

### Requirement: hello-world-output

The program must print "hello world" to standard output when executed.

#### Scenario: default-execution

**Given**: The user has Python installed
**When**: The user runs `python hello.py`
**Then**: The program prints `hello world` to stdout and exits with code 0
```

この spec があることで、Conflux はソフトウェアの振る舞いをすばやく理解し、次の変更にも安定して進めます。

---

以上で、最も簡単な実装サイクルは完了です。

この QUICKSTART は、最初の 1 回を最短で通すところまでで区切っています。
実運用では、proposal の磨き込み、設定調整、並列実行、トラブルシュートなど、さらに細かいテクニックが必要になることがあります。
続きは README や `cflx --help` を参照してください。

意見や質問は [GitHub Issue](https://github.com/tumf/conflux/issues) か X で `@tumf` にメンションしてください。

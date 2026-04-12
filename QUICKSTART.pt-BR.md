# QUICKSTART

[![日本語](https://img.shields.io/badge/日本語-QUICKSTART-blue?style=flat-square)](./QUICKSTART.ja.md) [![English](https://img.shields.io/badge/English-QUICKSTART-blue?style=flat-square)](./QUICKSTART.md) [![简体中文](https://img.shields.io/badge/简体中文-QUICKSTART-blue?style=flat-square)](./QUICKSTART.zh-CN.md) [![Español](https://img.shields.io/badge/Español-QUICKSTART-blue?style=flat-square)](./QUICKSTART.es.md) [![Português%20(BR)](https://img.shields.io/badge/Português%20(BR)-QUICKSTART-blue?style=flat-square)](./QUICKSTART.pt-BR.md) [![한국어](https://img.shields.io/badge/한국어-QUICKSTART-blue?style=flat-square)](./QUICKSTART.ko.md) [![Français](https://img.shields.io/badge/Français-QUICKSTART-blue?style=flat-square)](./QUICKSTART.fr.md) [![Deutsch](https://img.shields.io/badge/Deutsch-QUICKSTART-blue?style=flat-square)](./QUICKSTART.de.md) [![Русский](https://img.shields.io/badge/Русский-QUICKSTART-blue?style=flat-square)](./QUICKSTART.ru.md) [![Tiếng%20Việt](https://img.shields.io/badge/Tiếng%20Việt-QUICKSTART-blue?style=flat-square)](./QUICKSTART.vi.md)

Este é o guia mais curto para instalar o `cflx` pela primeira vez, configurar um projeto, criar um change do OpenSpec e concluir a implementação pela TUI.

O Conflux é implementado como o comando `cflx`.

## 0. Pré-requisitos

- Rust / Cargo disponível: [Instalar Rust](https://rust-lang.org/tools/install/)
- [Claude Code](https://claude.com/product/claude-code) disponível
- Um projeto gerenciado com git, como `~/myproject`

> O Conflux é um orquestrador que inicia e controla agentes de IA. Ele próprio não é um agente de programação.
> Ele pode usar CLIs como [Claude Code](https://claude.com/product/claude-code), [OpenCode](https://opencode.ai/) e [Codex](https://developers.openai.com/codex/cli).
> Este QUICKSTART usa o Claude Code como exemplo.

Verifique os pré-requisitos:

```bash
cargo --version
claude --version
claude -p 'hi'
```

## 1. Instalar o `cflx`

Instale a partir do crates.io.

```bash
cargo install cflx
```

Verifique a instalação:

```bash
cflx --version
```

## 2. Preparar um projeto

A partir daqui, trabalhe dentro do diretório do projeto. Este guia usa `~/myproject` como exemplo.

O Conflux usa `git worktree`, então o projeto precisa ser gerenciado com git.

```bash
cd ~/myproject
```

Se for um projeto novo:

```bash
mkdir -p ~/myproject
cd ~/myproject
git init
```

## 3. Instalar as bundled skills

Adicione ao projeto as bundled skills do Conflux para Claude Code.

```bash
cflx install-skills --claude
```

Isso instalará as skills `cflx-*` em `./.claude/skills`.

Você pode decidir depois, junto com `.cflx.jsonc`, se vai versioná-las no Git.

## 4. Criar o arquivo de configuração

O nome do arquivo de configuração é `.cflx.jsonc`, e não `.cflx.conf`.

A forma mais rápida é gerar o template.

```bash
cflx init
```

Isso cria `.cflx.jsonc` no diretório atual.

## 5. Verificar `.cflx.jsonc`

No mínimo, ele funciona se contiver os comandos para o agente que você quer usar.

Exemplo de template do Claude Code:

```jsonc
{
  "analyze_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p {prompt}",
  "apply_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '{prompt}'",
  "archive_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '{prompt}'",
  "acceptance_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '{prompt}'",
  "resolve_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p {prompt}"
}
```

Na primeira vez, basta usar como está o conteúdo gerado por `cflx init`.

## 6. Decidir o que colocar no Git

Na configuração inicial, decida se vai colocar estes dois itens no Git.

- `./.claude/skills/cflx-*`
- `./.cflx.jsonc`

Recomendação:

- Se você quiser reproduzir o mesmo comportamento em equipe ou em várias máquinas, faça commit dos dois
- Se for uso apenas local e quase descartável, adicione os dois ao `.gitignore`

Se estiver em dúvida, fazer commit dos dois é um bom padrão inicial. Fica mais fácil de manter se você não escrever segredos diretamente em `.cflx.jsonc`.

Se quiser adicionar ambos ao `.gitignore`:

```bash
printf ".claude/skills/cflx-*\n.cflx.jsonc\n" >> .gitignore
git add .gitignore
git commit -m 'Ignore Conflux local setup files'
```

Se quiser adicionar ambos ao repositório:

```bash
git add .claude/skills/cflx-* .cflx.jsonc
git commit -m 'Add Conflux setup files'
```

## 7. Criar o primeiro change proposal

O Conflux processa changes do OpenSpec.

Mesmo que você ainda não esteja familiarizado com o OpenSpec, tudo bem. As bundled skills já estão instaladas, então você pode pedir ao Claude Code para criar o proposal.

Por exemplo:

```text
/cflx-proposal exibir hello world em python
```

Isso gera um diretório de change como `openspec/changes/add-hello-world/`, com pelo menos estes dois arquivos.

- `proposal.md`: o que mudar
- `tasks.md`: o que implementar

Pelo caminho mais curto, basta revisar rapidamente esses dois arquivos e fazer commit se estiverem corretos.

Pontos de verificação:

- O conteúdo de `proposal.md` corresponde à mudança que você quer fazer
- As tarefas de implementação em `tasks.md` estão completas, sem excessos
- Não há mudanças desnecessárias misturadas

Se necessário, ajuste o proposal ou as tasks e, quando estiver tudo certo, faça commit.

```bash
git add openspec/changes/add-hello-world
git commit -m 'proposal: add-hello-world'
```

A estrutura detalhada fica assim:

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

## 8. Confirmar se o workspace está limpo

Antes de iniciar a TUI, confirme se a working tree está limpa.

```bash
git status
```

Se estiver limpa, aparecerá algo assim.

```text
On branch main
nothing to commit, working tree clean
```

## 9. Iniciar a TUI

Inicie o Conflux em modo TUI.

```bash
cflx
```

Você verá uma tela como esta.

![Conflux TUI ready screen](docs/images/quickstart/tui-ready-screen.png)

## 10. Executar pela TUI

Controles básicos:

- `↑/↓` ou `j/k`: selecionar um change
- `Space`: marcar para execução
- `F5`: iniciar a execução
- `Ctrl+C`: sair

Fluxo mais curto:

1. Inicie `cflx`
2. Vá até o change que deseja processar
3. Pressione `Space` para selecioná-lo
4. Pressione `F5` para executá-lo

Neste exemplo há apenas um change, então execute com `Space` → `F5`.

![Conflux TUI running screen](docs/images/quickstart/tui-running-screen.png)

O Conflux executa automaticamente o seguinte loop.

- apply
- accept
- archive
- resolve / merge

Quando chegar a `merged`, está concluído.

![Conflux TUI merged screen](docs/images/quickstart/tui-merged-screen.png)

## 11. Verificar o resultado

Confirme que a implementação foi criada.

```bash
tree
cat hello.py
```

Exemplo:

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

O lado do OpenSpec também é atualizado.

```bash
tree openspec -L 10
```

Exemplo:

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

Você pode ver que o change proposal foi arquivado e que a especificação final foi promovida para `openspec/specs`.

Por exemplo:

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

Com essa spec, o Conflux consegue entender rapidamente o comportamento do software e avançar com mais estabilidade para a próxima mudança.

---

Com isso, o ciclo de implementação mais simples está concluído.

Este QUICKSTART termina no ponto em que a primeira execução funciona pelo caminho mais curto possível.
No uso real, você pode precisar de técnicas mais detalhadas para refinar proposals, ajustar configurações, executar em paralelo e solucionar problemas.
Para os próximos passos, consulte o README ou `cflx --help`.

Se tiver comentários ou dúvidas, abra uma [GitHub Issue](https://github.com/tumf/conflux/issues) ou mencione `@tumf` no X.

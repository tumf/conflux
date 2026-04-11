# Conflux

[![日本語](https://img.shields.io/badge/%E6%97%A5%E6%9C%AC%E8%AA%9E-blue?style=flat-square)](./README.ja.md) [![English](https://img.shields.io/badge/English-blue?style=flat-square)](./README.md) [![简体中文](https://img.shields.io/badge/%E7%AE%80%E4%BD%93%E4%B8%AD%E6%96%87-blue?style=flat-square)](./README.zh-CN.md) [![Español](https://img.shields.io/badge/Espa%C3%B1ol-blue?style=flat-square)](./README.es.md) [![Português%20(BR)](https://img.shields.io/badge/Portugu%C3%AAs%20(BR)-blue?style=flat-square)](./README.pt-BR.md) [![한국어](https://img.shields.io/badge/%ED%95%9C%EA%B5%AD%EC%96%B4-blue?style=flat-square)](./README.ko.md) [![Français](https://img.shields.io/badge/Fran%C3%A7ais-blue?style=flat-square)](./README.fr.md) [![Deutsch](https://img.shields.io/badge/Deutsch-blue?style=flat-square)](./README.de.md) [![Русский](https://img.shields.io/badge/%D0%A0%D1%83%D1%81%D1%81%D0%BA%D0%B8%D0%B9-blue?style=flat-square)](./README.ru.md) [![Tiếng%20Việt](https://img.shields.io/badge/Ti%E1%BA%BFng%20Vi%E1%BB%87t-blue?style=flat-square)](./README.vi.md)

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

![Conflux TUI](docs/images/conflux-tui.jpg)

O Conflux é uma ferramenta que orquestra o desenvolvimento autônomo de agentes de codificação com IA com base em desenvolvimento orientado por especificações. Mesmo sem acompanhamento humano constante, ele mantém as mudanças avançando por um fluxo completo: aplicação, decisão de aceitação, arquivamento e merge final.

O objetivo não é gerar código de forma pontual. A proposta é definir primeiro a especificação e, a partir dela, acumular mudanças continuamente para fazer evoluir um produto final de certo porte, pensado para uso real.

Além disso, o Conflux não depende de um fornecedor específico de IA. Ele foi projetado para permitir a troca de ferramentas como [Claude Code](https://docs.anthropic.com/en/docs/claude-code), [Codex](https://openai.com/index/openai-codex/) e [OpenCode](https://opencode.ai/).

## Conceitos básicos do Conflux

- **Desenvolvimento autônomo que avança enquanto você dorme**: Mesmo sem supervisão humana constante, os agentes de IA processam as mudanças uma a uma e mantêm o desenvolvimento em movimento.
- **Desenvolvimento orientado por especificações**: Com [OpenSpec](https://github.com/openspec/openspec), você define primeiro a especificação e depois segue com implementação, aceitação e melhoria com base nela.
- **Evolução contínua de um produto final de certo porte**: Em vez de parar em uma geração única, o Conflux acumula mudanças ao longo do tempo e aproxima o projeto continuamente de um produto completo.

## Mecanismos que tornam isso possível

- **Múltiplos loops Ralph**: O Conflux melhora por meio de iterações repetidas, mantendo o contexto transferido em cada iteração no mínimo possível para usar os LLMs com mais eficiência.
- **Desenvolvimento paralelo com git worktree**: Ao atribuir um worktree independente a cada change, o Conflux permite que várias mudanças avancem em paralelo com segurança.
- **Escolha de agentes independente de fornecedor**: O Conflux não fica preso a um fornecedor específico como [Claude Code](https://docs.anthropic.com/en/docs/claude-code), [Codex](https://openai.com/index/openai-codex/) ou [OpenCode](https://opencode.ai/). Você pode trocar agentes de implementação e avaliação conforme o objetivo.
- **Separação entre implementação e aceitação**: Ao separar o papel que impulsiona a implementação do papel que valida o resultado, você pode combinar um codificador rápido com um revisor mais inteligente. Isso aumenta a eficiência no uso de LLMs e acelera o desenvolvimento como um todo.

Em resumo, o Conflux é um **orquestrador para operar o desenvolvimento autônomo orientado por especificações como um fluxo de desenvolvimento prático, com execução paralela e separação de papéis, impulsionando continuamente um produto final de certo porte**.

## Uso principal

| Uso | Comando |
|------|---------|
| TUI | `cflx` |
| Execução headless | `cflx run` |

Para modo servidor, TUI remota, API REST e `cflx service`, consulte o [guia do modo servidor (em inglês)](docs/guides/SERVER.md).

## Início rápido

Para a configuração inicial, consulte [QUICKSTART.pt-BR.md](QUICKSTART.pt-BR.md).

## Comandos básicos

```bash
# TUI
cflx

# Execução headless
cflx run

# Executar apenas uma mudança específica
cflx run --change add-feature-x

# Inicializar o arquivo de configuração
cflx init

# Instalar bundled skills
cflx install-skills
```

## Configuração

O arquivo de configuração usa o formato JSONC.

- `.cflx.jsonc`
- `~/.config/cflx/config.jsonc`
- `--config <PATH>`

Geração de templates:

```bash
cflx init
cflx init --template opencode
cflx init --template codex
cflx init --force
```

Para exemplos detalhados de configuração, hooks, execução em workspace e explicações sobre a fila de comandos, consulte o README em inglês.

## Instalação

```bash
cargo install cflx
```

## Documentação

| Documento | Descrição |
|----------|-------------|
| [QUICKSTART.pt-BR.md](QUICKSTART.pt-BR.md) | Configuração inicial |
| [Guia do modo servidor (em inglês)](docs/guides/SERVER.md) | Modo servidor, TUI remota, Web UI, API REST, serviço em segundo plano |
| [README.md](README.md) | Documentação completa (inglês) |
| [docs/guides/USAGE.md](docs/guides/USAGE.md) | Exemplos de uso |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Guia de contribuição |
| [docs/guides/DEVELOPMENT.md](docs/guides/DEVELOPMENT.md) | Guia de desenvolvimento |
| [docs/guides/RELEASE.md](docs/guides/RELEASE.md) | Guia de release |
| [docs/openapi.yaml](docs/openapi.yaml) | Especificação da API |

## Licença

MIT

# Conflux

[![日本語](https://img.shields.io/badge/%E6%97%A5%E6%9C%AC%E8%AA%9E-informational?style=flat-square)](./README.ja.md) [![English](https://img.shields.io/badge/English-informational?style=flat-square)](./README.md) [![简体中文](https://img.shields.io/badge/%E7%AE%80%E4%BD%93%E4%B8%AD%E6%96%87-informational?style=flat-square)](./README.zh-CN.md) [![Español](https://img.shields.io/badge/Espa%C3%B1ol-informational?style=flat-square)](./README.es.md) [![Português (BR)](https://img.shields.io/badge/Portugu%C3%AAs%20(BR)-informational?style=flat-square)](./README.pt-BR.md) [![한국어](https://img.shields.io/badge/%ED%95%9C%EA%B5%AD%EC%96%B4-informational?style=flat-square)](./README.ko.md) [![Français](https://img.shields.io/badge/Fran%C3%A7ais-informational?style=flat-square)](./README.fr.md) [![Deutsch](https://img.shields.io/badge/Deutsch-informational?style=flat-square)](./README.de.md) [![Русский](https://img.shields.io/badge/%D0%A0%D1%83%D1%81%D1%81%D0%BA%D0%B8%D0%B9-informational?style=flat-square)](./README.ru.md) [![Tiếng Việt](https://img.shields.io/badge/Ti%E1%BA%BFng%20Vi%E1%BB%87t-informational?style=flat-square)](./README.vi.md)

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

![Conflux TUI](docs/images/conflux-tui.jpg)

Conflux est un outil qui orchestre le développement autonome d’agents de codage IA sur la base d’un développement piloté par les spécifications. Même sans supervision humaine continue, il fait avancer les changements de façon ininterrompue, en enchaînant application, validation d’acceptation, archivage et fusion finale dans un flux unique.

L’objectif n’est pas la génération de code ponctuelle. Il s’agit de définir d’abord les spécifications, puis de faire croître en continu un produit d’une certaine ampleur, prêt pour un usage réel, en accumulant les changements conformément à ces spécifications.

Conflux ne dépend pas non plus d’un fournisseur d’IA particulier. Il est conçu pour permettre l’utilisation interchangeable de solutions comme [Claude Code](https://docs.anthropic.com/en/docs/claude-code), [Codex](https://openai.com/index/openai-codex/) et [OpenCode](https://opencode.ai/).

## Concepts fondamentaux de Conflux

- **Développement autonome qui progresse pendant que vous dormez** : même sans intervention humaine permanente, les agents IA traitent les changements les uns après les autres et font avancer le développement.
- **Développement piloté par les spécifications** : avec [OpenSpec](https://github.com/openspec/openspec), on définit d’abord les spécifications, puis on fait progresser l’implémentation, la validation et l’amélioration en s’y conformant.
- **Faire évoluer en continu un produit d’une certaine envergure** : au lieu de s’arrêter à une génération ponctuelle, on empile les changements pour se rapprocher progressivement d’un produit fini.

## Mécanismes qui rendent cela possible

- **Boucles Ralph imbriquées** : l’amélioration se fait par itérations successives, en minimisant à chaque fois le contexte transmis afin d’utiliser les LLM plus efficacement.
- **Développement parallèle avec `git worktree`** : Conflux assigne un worktree indépendant à chaque change, ce qui permet de faire avancer plusieurs changements en parallèle en toute sécurité.
- **Choix d’agents indépendant des fournisseurs** : sans être lié à un fournisseur précis, il permet de remplacer selon les besoins les agents d’implémentation ou d’évaluation par [Claude Code](https://docs.anthropic.com/en/docs/claude-code), [Codex](https://openai.com/index/openai-codex/), [OpenCode](https://opencode.ai/) et d’autres.
- **Séparation des rôles entre implémentation et validation** : en dissociant le rôle qui fait avancer l’implémentation de celui qui valide les résultats, on peut combiner des codeurs rapides avec des reviewers plus pertinents. Cela permet d’utiliser les LLM plus efficacement tout en accélérant l’ensemble du processus de développement.

En bref, Conflux est un **orchestrateur destiné à faire avancer en continu un produit d’une certaine ampleur, en opérant un développement autonome piloté par les spécifications comme un flux de développement réaliste, avec exécution parallèle et séparation des rôles**.

## Principales utilisations

| Usage | Commande |
|------|---------|
| TUI | `cflx` |
| Exécution headless | `cflx run` |

Pour le mode serveur, le TUI distant, l’API REST et `cflx service`, consultez le [guide du mode serveur (anglais)](docs/guides/SERVER.md).

## Démarrage rapide

Pour la configuration initiale, consultez [QUICKSTART.fr.md](QUICKSTART.fr.md).

## Commandes de base

```bash
# TUI
cflx

# Exécution headless
cflx run

# Exécuter uniquement un changement spécifique
cflx run --change add-feature-x

# Initialiser le fichier de configuration
cflx init

# Installer les bundled skills
cflx install-skills
```

## Configuration

Le fichier de configuration est en JSONC.

- `.cflx.jsonc`
- `~/.config/cflx/config.jsonc`
- `--config <PATH>`

Génération de modèles :

```bash
cflx init
cflx init --template opencode
cflx init --template codex
cflx init --force
```

Pour des exemples de configuration plus détaillés, les hooks, l’exécution en workspace et l’explication de la file de commandes, consultez le README en anglais.

## Installation

```bash
cargo install cflx
```

## Documentation

| Document | Description |
|----------|-------------|
| [QUICKSTART.fr.md](QUICKSTART.fr.md) | Configuration initiale |
| [Guide du mode serveur (anglais)](docs/guides/SERVER.md) | Mode serveur, TUI distant, Web UI, API REST, service en arrière-plan |
| [README.md](README.md) | Documentation complète (anglais) |
| [docs/guides/USAGE.md](docs/guides/USAGE.md) | Exemples d’utilisation |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Guide de contribution |
| [docs/guides/DEVELOPMENT.md](docs/guides/DEVELOPMENT.md) | Guide de développement |
| [docs/guides/RELEASE.md](docs/guides/RELEASE.md) | Guide de publication |
| [docs/openapi.yaml](docs/openapi.yaml) | Spécification de l’API |

## Licence

MIT

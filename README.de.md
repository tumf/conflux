# Conflux

[![日本語](https://img.shields.io/badge/%E8%A8%80%E8%AA%9E-日本語-0f766e?style=flat-square)](./README.ja.md)
[![English](https://img.shields.io/badge/Language-English-2563eb?style=flat-square)](./README.md)
[![简体中文](https://img.shields.io/badge/%E8%AF%AD%E8%A8%80-%E7%AE%80%E4%BD%93%E4%B8%AD%E6%96%87-dc2626?style=flat-square)](./README.zh-CN.md)
[![Español](https://img.shields.io/badge/Idioma-Espa%C3%B1ol-f59e0b?style=flat-square)](./README.es.md)
[![Português (BR)](https://img.shields.io/badge/Idioma-Portugu%C3%AAs%20(BR)-16a34a?style=flat-square)](./README.pt-BR.md)
[![한국어](https://img.shields.io/badge/%EC%96%B8%EC%96%B4-%ED%95%9C%EA%B5%AD%EC%96%B4-7c3aed?style=flat-square)](./README.ko.md)
[![Français](https://img.shields.io/badge/Langue-Fran%C3%A7ais-0891b2?style=flat-square)](./README.fr.md)
[![Deutsch](https://img.shields.io/badge/Sprache-Deutsch-4b5563?style=flat-square)](./README.de.md)
[![Русский](https://img.shields.io/badge/%D0%AF%D0%B7%D1%8B%D0%BA-%D0%A0%D1%83%D1%81%D1%81%D0%BA%D0%B8%D0%B9-b91c1c?style=flat-square)](./README.ru.md)
[![Tiếng Việt](https://img.shields.io/badge/Ng%C3%B4n%20ng%E1%BB%AF-Ti%E1%BA%BFng%20Vi%E1%BB%87t-ea580c?style=flat-square)](./README.vi.md)

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

![Conflux TUI](docs/images/conflux-tui.jpg)

Conflux ist ein Werkzeug zur Orchestrierung autonomer Entwicklung durch KI-Coding-Agenten auf Basis spezifikationsgetriebener Entwicklung. Auch ohne ständige menschliche Aufsicht treibt es Änderungen fortlaufend voran und führt Anwendung, Abnahmeentscheidung, Archivierung und schließlich das Merge in einem durchgängigen Ablauf zusammen.

Das Ziel ist nicht einmalige Codegenerierung. Zuerst werden Spezifikationen festgelegt, und anschließend wird ein Produkt von gewisser Größenordnung, das auf den realen Einsatz ausgerichtet ist, kontinuierlich weiterentwickelt, indem Änderungen entlang dieser Spezifikationen aufgebaut werden.

Conflux ist außerdem nicht von einem bestimmten KI-Anbieter abhängig. Es ist so konzipiert, dass Lösungen wie [Claude Code](https://docs.anthropic.com/en/docs/claude-code), [Codex](https://openai.com/index/openai-codex/) und [OpenCode](https://opencode.ai/) austauschbar genutzt werden können.

## Grundkonzepte von Conflux

- **Autonome Entwicklung, die weiterläuft, während Sie schlafen**: Auch ohne ständige menschliche Begleitung verarbeiten KI-Agenten Änderungen nacheinander und treiben die Entwicklung voran.
- **Spezifikationsgetriebene Entwicklung**: Mit [OpenSpec](https://github.com/openspec/openspec) werden zuerst Spezifikationen definiert; darauf aufbauend werden Implementierung, Abnahme und Verbesserungen vorangetrieben.
- **Ein Produkt von gewisser Größenordnung kontinuierlich ausbauen**: Statt bei einmaliger Generierung stehenzubleiben, werden Änderungen schrittweise aufgebaut, um sich einem fertigen Produkt anzunähern.

## Mechanismen, die das ermöglichen

- **Mehrstufige Ralph-Loops**: Verbesserungen erfolgen iterativ, wobei der pro Iteration übergebene Kontext auf ein Minimum reduziert wird, um LLMs effizient zu nutzen.
- **Parallele Entwicklung mit `git worktree`**: Conflux weist jedem Change ein eigenes Worktree zu, sodass mehrere Änderungen sicher parallel bearbeitet werden können.
- **Anbieterunabhängige Wahl der Agenten**: Ohne an einen bestimmten Anbieter gebunden zu sein, können Implementierungs- und Bewertungsagenten je nach Zweck durch [Claude Code](https://docs.anthropic.com/en/docs/claude-code), [Codex](https://openai.com/index/openai-codex/), [OpenCode](https://opencode.ai/) und andere ersetzt werden.
- **Trennung von Implementierung und Abnahme**: Indem die Rolle, die die Implementierung vorantreibt, von der Rolle getrennt wird, die die Ergebnisse abnimmt, lassen sich schnelle Coder mit klugen Reviewern kombinieren. Dadurch können LLMs effizienter eingesetzt und gleichzeitig die Gesamtgeschwindigkeit der Entwicklung erhöht werden.

Kurz gesagt ist Conflux ein **Orchestrator, der autonome, spezifikationsgetriebene Entwicklung als realistischen Entwicklungsfluss mit paralleler Ausführung und Rollentrennung betreibt, um ein Produkt von gewisser Größenordnung kontinuierlich voranzubringen**.

## Hauptanwendungsfälle

| Verwendung | Befehl |
|------|---------|
| TUI | `cflx` |
| Headless-Ausführung | `cflx run` |

Für Servermodus, Remote-TUI, REST API und `cflx service` siehe den [Leitfaden zum Servermodus (Englisch)](docs/guides/SERVER.md).

## Schnellstart

Für die Ersteinrichtung siehe [QUICKSTART.de.md](QUICKSTART.de.md).

## Grundlegende Befehle

```bash
# TUI
cflx

# Headless-Ausführung
cflx run

# Nur eine bestimmte Änderung ausführen
cflx run --change add-feature-x

# Konfigurationsdatei initialisieren
cflx init

# bundled skills installieren
cflx install-skills
```

## Konfiguration

Die Konfigurationsdatei ist im JSONC-Format.

- `.cflx.jsonc`
- `~/.config/cflx/config.jsonc`
- `--config <PATH>`

Vorlagen generieren:

```bash
cflx init
cflx init --template opencode
cflx init --template codex
cflx init --force
```

Ausführlichere Konfigurationsbeispiele sowie Erläuterungen zu Hooks, Workspace-Ausführung und der Befehlswarteschlange finden Sie im englischen README.

## Installation

```bash
cargo install cflx
```

## Dokumentation

| Dokument | Beschreibung |
|----------|-------------|
| [QUICKSTART.de.md](QUICKSTART.de.md) | Ersteinrichtung |
| [Leitfaden zum Servermodus (Englisch)](docs/guides/SERVER.md) | Servermodus, Remote-TUI, Web UI, REST API, Hintergrunddienst |
| [README.md](README.md) | Vollständige Dokumentation (Englisch) |
| [docs/guides/USAGE.md](docs/guides/USAGE.md) | Anwendungsbeispiele |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Leitfaden für Beiträge |
| [docs/guides/DEVELOPMENT.md](docs/guides/DEVELOPMENT.md) | Entwicklungsleitfaden |
| [docs/guides/RELEASE.md](docs/guides/RELEASE.md) | Release-Leitfaden |
| [docs/openapi.yaml](docs/openapi.yaml) | API-Spezifikation |

## Lizenz

MIT

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

Conflux — это инструмент, оркестрирующий автономную разработку с участием AI-агентов для программирования на основе спецификационно-ориентированного подхода. Даже без постоянного участия человека он непрерывно продвигает изменения, объединяя применение, приёмочное решение, архивирование и финальное слияние в единый рабочий процесс.

Его цель — не разовая генерация кода. Сначала задаются спецификации, а затем на их основе последовательно наращиваются изменения, чтобы непрерывно развивать продукт определённого масштаба, рассчитанный на реальную эксплуатацию.

Кроме того, Conflux не зависит от конкретного поставщика ИИ. Он спроектирован так, чтобы можно было взаимозаменяемо использовать такие решения, как [Claude Code](https://docs.anthropic.com/en/docs/claude-code), [Codex](https://openai.com/index/openai-codex/) и [OpenCode](https://opencode.ai/).

## Базовые концепции Conflux

- **Автономная разработка, которая движется вперёд, пока вы спите**: даже без постоянного человеческого участия AI-агенты последовательно обрабатывают изменения и продвигают разработку.
- **Спецификационно-ориентированная разработка**: с помощью [OpenSpec](https://github.com/openspec/openspec) сначала определяются спецификации, а затем в соответствии с ними ведутся реализация, приёмка и улучшения.
- **Непрерывное развитие продукта определённого масштаба**: вместо разовой генерации изменения накапливаются шаг за шагом, приближая систему к завершённому продукту.

## Механизмы, которые это обеспечивают

- **Многоуровневые циклы Ralph**: улучшения выполняются итеративно, при этом объём контекста, передаваемого между итерациями, сводится к минимуму для более эффективного использования LLM.
- **Параллельная разработка с `git worktree`**: Conflux выделяет отдельный worktree для каждого change, что позволяет безопасно вести несколько изменений параллельно.
- **Свободный выбор агентов без привязки к вендору**: без фиксации на одном поставщике можно заменять агентов реализации и оценки в зависимости от задачи, используя [Claude Code](https://docs.anthropic.com/en/docs/claude-code), [Codex](https://openai.com/index/openai-codex/), [OpenCode](https://opencode.ai/) и другие инструменты.
- **Разделение ролей реализации и приёмки**: разделяя роль, продвигающую реализацию, и роль, проверяющую результат, можно сочетать быстрых кодеров с более сильными ревьюерами. Это позволяет эффективнее использовать LLM и одновременно ускорять весь процесс разработки.

Иными словами, Conflux — это **оркестратор, предназначенный для непрерывного продвижения продукта определённого масштаба за счёт спецификационно-ориентированной автономной разработки, организованной как реалистичный процесс с параллельным выполнением и разделением ролей**.

## Основные способы использования

| Использование | Команда |
|------|---------|
| TUI | `cflx` |
| Безголовый запуск | `cflx run` |

Полезные клавиши TUI:

| Клавиша | Действие |
|---------|----------|
| `Space` | Отметить или снять отметку с changes |
| `F5` | Запустить, возобновить, повторить или продолжить обработку |
| `x` | Добавить подходящие changes со статусом `not queued` в очередь во время обработки |

О веб-интерфейсе мониторинга, REST API и `/api/v2` см. [руководство по Web UI (английская версия)](docs/guides/WEBUI.md).

## Быстрый старт

Первоначальную настройку см. в [QUICKSTART.ru.md](QUICKSTART.ru.md).

## Основные команды

```bash
# TUI
cflx

# Безголовый запуск
cflx run

# Запустить только конкретное изменение
cflx run --change add-feature-x

# Инициализировать файл конфигурации
cflx init

# Установить bundled skills
cflx install-skills

# Установить bundled skills для Claude Code
cflx install-skills --claude

# Установить bundled skills глобально для Claude Code
cflx install-skills --claude --global
```

## Конфигурация

Файл конфигурации использует формат JSONC.

- `.cflx.jsonc`
- `~/.config/cflx/config.jsonc`
- `--config <PATH>`

Пользовательские настройки TUI намеренно отделены от конфигурации оркестрации. Клавиша по умолчанию для запуска, возобновления, повтора или продолжения — `F5`; чтобы переопределить только локальную привязку запуска TUI, используйте `~/.config/cflx/tui.jsonc`:

```jsonc
{
  "keybindings": {
    "start": ["F5", "!"]
  }
}
```

Генерация шаблонов конфигурации:

```bash
cflx init
cflx init --template opencode
cflx init --template codex
cflx init --force
```

Более подробные примеры конфигурации, а также описание hooks, выполнения в workspace и очереди команд см. в [docs/guides/USAGE.md](docs/guides/USAGE.md).

## Установка

```bash
cargo install cflx
```

## Документация

| Документ | Описание |
|----------|-------------|
| [QUICKSTART.ru.md](QUICKSTART.ru.md) | Первоначальная настройка |
| [Руководство по Web UI (английская версия)](docs/guides/WEBUI.md) | Web UI, REST API, `/api/v2`, миграция с серверного режима |
| [README.md](README.md) | Полная документация (английский) |
| [docs/guides/USAGE.md](docs/guides/USAGE.md) | Примеры использования |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Руководство по внесению вклада |
| [docs/guides/DEVELOPMENT.md](docs/guides/DEVELOPMENT.md) | Руководство по разработке |
| [docs/guides/RELEASE.md](docs/guides/RELEASE.md) | Руководство по выпуску релизов |
| [docs/openapi.yaml](docs/openapi.yaml) | Спецификация API |

## Лицензия

MIT

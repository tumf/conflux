# QUICKSTART

[![日本語](https://img.shields.io/badge/日本語-QUICKSTART-blue?style=flat-square)](./QUICKSTART.ja.md)
[![English](https://img.shields.io/badge/English-QUICKSTART-blue?style=flat-square)](./QUICKSTART.md)
[![简体中文](https://img.shields.io/badge/简体中文-QUICKSTART-blue?style=flat-square)](./QUICKSTART.zh-CN.md)
[![Español](https://img.shields.io/badge/Español-QUICKSTART-blue?style=flat-square)](./QUICKSTART.es.md)
[![Português (BR)](https://img.shields.io/badge/Português%20(BR)-QUICKSTART-blue?style=flat-square)](./QUICKSTART.pt-BR.md)
[![한국어](https://img.shields.io/badge/한국어-QUICKSTART-blue?style=flat-square)](./QUICKSTART.ko.md)
[![Français](https://img.shields.io/badge/Français-QUICKSTART-blue?style=flat-square)](./QUICKSTART.fr.md)
[![Deutsch](https://img.shields.io/badge/Deutsch-QUICKSTART-blue?style=flat-square)](./QUICKSTART.de.md)
[![Русский](https://img.shields.io/badge/Русский-QUICKSTART-blue?style=flat-square)](./QUICKSTART.ru.md)
[![Tiếng Việt](https://img.shields.io/badge/Tiếng%20Việt-QUICKSTART-blue?style=flat-square)](./QUICKSTART.vi.md)

Это самый короткий путь: впервые установить `cflx`, настроить проект, создать change OpenSpec и полностью пройти реализацию в TUI.

Conflux реализован как команда `cflx`.

## 0. Предварительные условия

- Доступны Rust / Cargo: [Установить Rust](https://rust-lang.org/tools/install/)
- Доступен [Claude Code](https://claude.com/product/claude-code)
- Есть проект под управлением git, например `~/myproject`

> Conflux — это оркестратор для запуска и управления AI-агентами. Он сам не является агентом для программирования.
> Он может использовать CLI-инструменты, такие как [Claude Code](https://claude.com/product/claude-code), [OpenCode](https://opencode.ai/) и [Codex](https://developers.openai.com/codex/cli).
> В этом QUICKSTART в качестве примера используется Claude Code.

Проверьте предварительные условия:

```bash
cargo --version
claude --version
claude -p 'hi'
```

## 1. Установить `cflx`

Установите пакет из crates.io.

```bash
cargo install cflx
```

После установки проверьте результат:

```bash
cflx --version
```

## 2. Подготовить проект

Далее работа ведётся в каталоге проекта. В качестве примера используется `~/myproject`.

Поскольку Conflux использует `git worktree`, проект должен находиться под управлением git.

```bash
cd ~/myproject
```

Если проект новый:

```bash
mkdir -p ~/myproject
cd ~/myproject
git init
```

## 3. Установить bundled skills

Добавьте bundled skills Conflux для Claude Code в проект.

```bash
cflx install-skills --claude
```

После этого skills `cflx-*` появятся в `./.claude/skills`.

Позже вместе с `.cflx.jsonc` вы решите, нужно ли включать их в Git.

## 4. Создать файл конфигурации

Имя файла конфигурации — `.cflx.jsonc`, а не `.cflx.conf`.

Самый быстрый способ — сгенерировать шаблон.

```bash
cflx init
```

В текущем каталоге будет создан файл `.cflx.jsonc`.

## 5. Проверить `.cflx.jsonc`

Как минимум достаточно, чтобы в нём были команды для нужного вам агента.

Пример шаблона для Claude Code:

```jsonc
{
  "analyze_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p {prompt}",
  "apply_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '{prompt}'",
  "archive_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '{prompt}'",
  "acceptance_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '{prompt}'",
  "resolve_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p {prompt}"
}
```

Для первого запуска обычно достаточно использовать содержимое, созданное `cflx init`, без изменений.

## 6. Решить, что добавлять в Git

Во время первоначальной настройки нужно решить, добавлять ли в Git следующие два элемента:

- `./.claude/skills/cflx-*`
- `./.cflx.jsonc`

Рекомендуемый подход:

- Если вы хотите воспроизводить одинаковое поведение в команде или на нескольких машинах, закоммитьте оба.
- Если это только локальная и почти одноразовая настройка, добавьте оба в `.gitignore`.

Если сомневаетесь, можно начать с коммита обоих. Работать проще, если не записывать секреты напрямую в `.cflx.jsonc`.

Если оба файла нужно добавить в `.gitignore`:

```bash
printf ".claude/skills/cflx-*\n.cflx.jsonc\n" >> .gitignore
git add .gitignore
git commit -m 'Ignore Conflux local setup files'
```

Если оба файла нужно добавить в репозиторий:

```bash
git add .claude/skills/cflx-* .cflx.jsonc
git commit -m 'Add Conflux setup files'
```

## 7. Создать первую change proposal

Conflux обрабатывает changes OpenSpec.

Даже если вы ещё не знакомы с OpenSpec, это не проблема. Bundled skills уже установлены, поэтому можно поручить Claude Code создать proposal.

Например:

```text
/cflx-proposal python で hello world と表示する
```

В результате будет создан каталог change, например `openspec/changes/add-hello-world/`, содержащий как минимум следующие два файла:

- `proposal.md`: что будет изменено
- `tasks.md`: что нужно реализовать

Для самого короткого пути обычно достаточно быстро просмотреть эти два файла и, если всё в порядке, сразу закоммитить их.

Что проверить:

- Содержимое `proposal.md` соответствует желаемому изменению
- В `tasks.md` перечислены все нужные задачи реализации, без пропусков и лишнего
- В proposal не попали посторонние изменения

При необходимости исправьте proposal или tasks, затем закоммитьте, если содержимое вас устраивает.

```bash
git add openspec/changes/add-hello-world
git commit -m 'proposal: add-hello-world'
```

Подробная структура выглядит так:

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

## 8. Убедиться, что рабочее дерево чистое

Перед запуском TUI проверьте, что рабочее дерево чистое.

```bash
git status
```

Если всё чисто, вывод будет примерно таким:

```text
On branch main
nothing to commit, working tree clean
```

## 9. Запустить TUI

Запустите Conflux в режиме TUI.

```bash
cflx
```

Появится экран примерно такого вида.

![Conflux TUI ready screen](docs/images/quickstart/tui-ready-screen.png)

## 10. Выполнение в TUI

Основные действия:

- `↑/↓` or `j/k`: выбрать change
- `Space`: отметить для выполнения
- `F5`: начать выполнение
- `Ctrl+C`: выйти

Самый короткий сценарий:

1. Запустите `cflx`
2. Перейдите к нужному change
3. Выберите его клавишей `Space`
4. Запустите выполнение клавишей `F5`

В этом примере change только один, поэтому достаточно `Space` → `F5`.

![Conflux TUI running screen](docs/images/quickstart/tui-running-screen.png)

Conflux автоматически выполняет следующий цикл:

- apply
- accept
- archive
- resolve / merge

Когда состояние станет `merged`, работа завершена.

![Conflux TUI merged screen](docs/images/quickstart/tui-merged-screen.png)

## 11. Проверить результат

Убедитесь, что реализация была добавлена.

```bash
tree
cat hello.py
```

Пример:

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

Со стороны OpenSpec изменения тоже отражены.

```bash
tree openspec -L 10
```

Пример:

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

Здесь видно, что change proposal была архивирована, а итоговая спецификация перенесена в `openspec/specs`.

Например:

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

Благодаря этой spec Conflux может быстро понять поведение программы и стабильно продолжать работу над следующими изменениями.

---

На этом самый простой цикл реализации завершён.

Этот QUICKSTART ограничивается самым коротким сценарием первого запуска.
В реальной работе могут понадобиться более тонкие приёмы: доработка proposal, настройка конфигурации, параллельный запуск и устранение неполадок.
См. README или `cflx --help`.

Отзывы и вопросы можно отправлять через [GitHub Issue](https://github.com/tumf/conflux/issues) или упомянуть `@tumf` в X.

# QUICKSTART

[![日本語](https://img.shields.io/badge/日本語-QUICKSTART-blue?style=flat-square)](./QUICKSTART.ja.md) [![English](https://img.shields.io/badge/English-QUICKSTART-blue?style=flat-square)](./QUICKSTART.md) [![简体中文](https://img.shields.io/badge/简体中文-QUICKSTART-blue?style=flat-square)](./QUICKSTART.zh-CN.md) [![Español](https://img.shields.io/badge/Español-QUICKSTART-blue?style=flat-square)](./QUICKSTART.es.md) [![Português%20(BR)](https://img.shields.io/badge/Português%20(BR)-QUICKSTART-blue?style=flat-square)](./QUICKSTART.pt-BR.md) [![한국어](https://img.shields.io/badge/한국어-QUICKSTART-blue?style=flat-square)](./QUICKSTART.ko.md) [![Français](https://img.shields.io/badge/Français-QUICKSTART-blue?style=flat-square)](./QUICKSTART.fr.md) [![Deutsch](https://img.shields.io/badge/Deutsch-QUICKSTART-blue?style=flat-square)](./QUICKSTART.de.md) [![Русский](https://img.shields.io/badge/Русский-QUICKSTART-blue?style=flat-square)](./QUICKSTART.ru.md) [![Tiếng%20Việt](https://img.shields.io/badge/Tiếng%20Việt-QUICKSTART-blue?style=flat-square)](./QUICKSTART.vi.md)

Esta es la guía más corta para instalar `cflx` por primera vez, configurar un proyecto, crear un change de OpenSpec y completar la implementación desde la TUI.

Conflux se implementa con el comando `cflx`.

## 0. Requisitos previos

- Rust / Cargo disponible: [Instalar Rust](https://rust-lang.org/tools/install/)
- [Claude Code](https://claude.com/product/claude-code) disponible
- Un proyecto gestionado con git, como `~/myproject`

> Conflux es un orquestador que inicia y controla agentes de IA. No es en sí mismo un agente de programación.
> Puede usar CLIs como [Claude Code](https://claude.com/product/claude-code), [OpenCode](https://opencode.ai/) y [Codex](https://developers.openai.com/codex/cli).
> En este QUICKSTART se usa Claude Code como ejemplo.

Verifica los requisitos previos:

```bash
cargo --version
claude --version
claude -p 'hi'
```

## 1. Instalar `cflx`

Instálalo desde crates.io.

```bash
cargo install cflx
```

Verifica la instalación:

```bash
cflx --version
```

## 2. Preparar un proyecto

A partir de aquí, trabaja dentro del directorio del proyecto. Este ejemplo usa `~/myproject`.

Conflux usa `git worktree`, así que el proyecto debe estar gestionado con git.

```bash
cd ~/myproject
```

Si es un proyecto nuevo:

```bash
mkdir -p ~/myproject
cd ~/myproject
git init
```

## 3. Instalar las bundled skills

Añade al proyecto las bundled skills de Conflux para Claude Code.

```bash
cflx install-skills --claude
```

Esto instalará las skills `cflx-*` en `./.claude/skills`.

Más adelante puedes decidir, junto con `.cflx.jsonc`, si las vas a incluir en Git.

## 4. Crear el archivo de configuración

El archivo de configuración se llama `.cflx.jsonc`, no `.cflx.conf`.

La forma más rápida es generar la plantilla.

```bash
cflx init
```

Esto crea `.cflx.jsonc` en el directorio actual.

## 5. Revisar `.cflx.jsonc`

Como mínimo, funcionará si contiene los comandos del agente que quieras usar.

Ejemplo de plantilla para Claude Code:

```jsonc
{
  "analyze_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p {prompt}",
  "apply_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '{prompt}'",
  "archive_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '{prompt}'",
  "acceptance_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '{prompt}'",
  "resolve_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p {prompt}"
}
```

Para la primera vez, basta con usar tal cual el contenido generado por `cflx init`.

## 6. Decidir qué incluir en Git

En la configuración inicial, decide si vas a incluir estos dos elementos en Git.

- `./.claude/skills/cflx-*`
- `./.cflx.jsonc`

Recomendación:

- Si quieres reproducir el mismo comportamiento en un equipo o en varias máquinas, haz commit de ambos
- Si es solo para uso local y casi desechable, añade ambos a `.gitignore`

Si no estás seguro, hacer commit de ambos es una buena opción inicial. Resulta más fácil de manejar si no escribes secretos directamente en `.cflx.jsonc`.

Si añades ambos a `.gitignore`:

```bash
printf ".claude/skills/cflx-*\n.cflx.jsonc\n" >> .gitignore
git add .gitignore
git commit -m 'Ignore Conflux local setup files'
```

Si añades ambos al repositorio:

```bash
git add .claude/skills/cflx-* .cflx.jsonc
git commit -m 'Add Conflux setup files'
```

## 7. Crear el primer change proposal

Conflux procesa changes de OpenSpec.

Aunque todavía no estés familiarizado con OpenSpec, no pasa nada. Las bundled skills ya están instaladas, así que puedes pedirle a Claude Code que cree el proposal.

Por ejemplo:

```text
/cflx-proposal mostrar hello world en python
```

Esto generará un directorio de change como `openspec/changes/add-hello-world/`, con al menos estos dos archivos.

- `proposal.md`: qué se va a cambiar
- `tasks.md`: qué se va a implementar

Para ir por el camino más corto, basta con revisar rápidamente estos dos archivos y hacer commit si están bien.

Puntos a comprobar:

- El contenido de `proposal.md` coincide con el cambio que quieres hacer
- Las tareas de implementación en `tasks.md` están completas y no sobran
- No se han mezclado cambios innecesarios

Si hace falta, corrige el proposal o las tasks; cuando el contenido esté bien, haz commit.

```bash
git add openspec/changes/add-hello-world
git commit -m 'proposal: add-hello-world'
```

La estructura detallada será así:

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

## 8. Confirmar que el workspace está limpio

Antes de iniciar la TUI, confirma que el árbol de trabajo está limpio.

```bash
git status
```

Si está limpio, verás algo como esto:

```text
On branch main
nothing to commit, working tree clean
```

## 9. Iniciar la TUI

Inicia Conflux en modo TUI.

```bash
cflx
```

Verás una pantalla como esta.

![Conflux TUI ready screen](docs/images/quickstart/tui-ready-screen.png)

## 10. Ejecutarlo desde la TUI

Controles básicos:

- `↑/↓` o `j/k`: seleccionar un change
- `Space`: marcarlo para ejecutar
- `F5`: iniciar la ejecución
- `Ctrl+C`: salir

Flujo más corto:

1. Inicia `cflx`
2. Muévete al change que quieras procesar
3. Pulsa `Space` para seleccionarlo
4. Pulsa `F5` para ejecutarlo

En este ejemplo solo hay un change, así que ejecútalo con `Space` → `F5`.

![Conflux TUI running screen](docs/images/quickstart/tui-running-screen.png)

Conflux ejecutará automáticamente el siguiente bucle.

- apply
- accept
- archive
- resolve / merge

Cuando llegue a `merged`, habrá terminado.

![Conflux TUI merged screen](docs/images/quickstart/tui-merged-screen.png)

## 11. Comprobar el resultado

Confirma que la implementación está presente.

```bash
tree
cat hello.py
```

Ejemplo:

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

La parte de OpenSpec también se actualiza.

```bash
tree openspec -L 10
```

Ejemplo:

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

Puedes ver que el change proposal se ha archivado y que la especificación final se ha promovido a `openspec/specs`.

Por ejemplo:

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

Con esta spec, Conflux puede entender rápidamente el comportamiento del software y avanzar con mayor estabilidad al siguiente cambio.

---

Con esto termina el ciclo de implementación más sencillo.

Este QUICKSTART se detiene en el punto de completar la primera ejecución por la vía más corta.
En un uso real, puede que necesites técnicas más detalladas para refinar proposals, ajustar la configuración, ejecutar en paralelo y resolver problemas.
Para continuar, consulta el README o `cflx --help`.

Si tienes comentarios o preguntas, abre un [GitHub Issue](https://github.com/tumf/conflux/issues) o menciona a `@tumf` en X.

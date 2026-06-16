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

Conflux es una herramienta que orquesta el desarrollo autónomo de agentes de codificación de IA basado en desarrollo guiado por especificaciones. Sin necesidad de supervisión humana constante, mantiene los cambios avanzando a través de todo el flujo: aplicación, validación de aceptación, archivado y fusión final.

El objetivo no es la generación puntual de código. La idea es definir primero la especificación y, a partir de ella, seguir acumulando cambios para hacer crecer de forma continua un producto terminado de cierta escala y pensado para uso real.

Además, Conflux no depende de un proveedor específico de IA. Está diseñado para poder intercambiar herramientas como [Claude Code](https://docs.anthropic.com/en/docs/claude-code), [Codex](https://openai.com/index/openai-codex/) y [OpenCode](https://opencode.ai/).

## Conceptos básicos de Conflux

- **Desarrollo autónomo que avanza mientras duermes**: Aunque no haya una persona pendiente todo el tiempo, los agentes de IA procesan los cambios uno por uno y hacen avanzar el desarrollo.
- **Desarrollo guiado por especificaciones**: Con [OpenSpec](https://github.com/openspec/openspec), primero se define la especificación y después se avanza en implementación, validación y mejora conforme a ella.
- **Hacer crecer continuamente un producto terminado de cierta escala**: No se queda en una generación puntual; Conflux acumula cambios y acerca progresivamente el proyecto a un producto completo.

## Mecanismos que lo hacen posible

- **Bucles Ralph múltiples**: Mejora mediante iteraciones repetidas, manteniendo al mínimo el contexto que se transfiere en cada iteración para usar los LLM de forma más eficiente.
- **Desarrollo en paralelo con git worktree**: Al asignar un worktree independiente a cada change, Conflux permite avanzar con seguridad en varios cambios en paralelo.
- **Elección de agentes independiente del proveedor**: No queda atado a un proveedor concreto como [Claude Code](https://docs.anthropic.com/en/docs/claude-code), [Codex](https://openai.com/index/openai-codex/) u [OpenCode](https://opencode.ai/). Puedes sustituir los agentes de implementación o evaluación según el objetivo.
- **Separación entre implementación y aceptación**: Al separar el rol que impulsa la implementación del rol que valida el resultado, puedes combinar un codificador rápido con un revisor más inteligente. Así se aprovechan mejor los LLM y también se acelera el desarrollo en conjunto.

En resumen, Conflux es un **orquestador para operar el desarrollo autónomo basado en especificaciones como un flujo de desarrollo práctico, con ejecución en paralelo y separación de roles, y para seguir impulsando de forma continua un producto terminado de cierta escala**.

## Uso principal

| Uso | Comando |
|------|---------|
| TUI | `cflx` |
| Ejecución headless | `cflx run` |

Teclas útiles del TUI:

| Tecla | Acción |
|------|--------|
| `Space` | Marcar o desmarcar changes |
| `F5` | Iniciar, reanudar, reintentar o continuar el procesamiento |
| `x` | Poner en cola los changes `not queued` elegibles mientras el procesamiento está en marcha |

Para el modo servidor, la TUI remota, la API REST y `cflx service`, consulta la [guía del modo servidor (en inglés)](docs/guides/SERVER.md).

## Inicio rápido

Para la configuración inicial, consulta [QUICKSTART.es.md](QUICKSTART.es.md).

## Comandos básicos

```bash
# TUI
cflx

# Ejecución headless
cflx run

# Ejecutar solo un cambio específico
cflx run --change add-feature-x

# Inicializar el archivo de configuración
cflx init

# Instalar bundled skills
cflx install-skills

# Instalar bundled skills para Claude Code
cflx install-skills --claude

# Instalar bundled skills globalmente para Claude Code
cflx install-skills --claude --global
```

## Configuración

El archivo de configuración usa formato JSONC.

- `.cflx.jsonc`
- `~/.config/cflx/config.jsonc`
- `--config <PATH>`

Las preferencias de usuario del TUI están separadas intencionalmente de la configuración de orquestación. La tecla predeterminada para iniciar, reanudar, reintentar o continuar es `F5`; para cambiar solo el binding local de inicio del TUI, usa `~/.config/cflx/tui.jsonc`:

```jsonc
{
  "keybindings": {
    "start": ["F5", "!"]
  }
}
```

Generación de plantillas de configuración:

```bash
cflx init
cflx init --template opencode
cflx init --template codex
cflx init --force
```

Para ejemplos detallados de configuración, hooks, ejecución en workspaces y la cola de comandos, consulta [docs/guides/USAGE.md](docs/guides/USAGE.md).

## Instalación

```bash
cargo install cflx
```

## Documentación

| Documento | Descripción |
|----------|-------------|
| [QUICKSTART.es.md](QUICKSTART.es.md) | Configuración inicial |
| [Guía del modo servidor (en inglés)](docs/guides/SERVER.md) | Modo servidor, TUI remota, Web UI, API REST, servicio en segundo plano |
| [README.md](README.md) | Documentación completa (inglés) |
| [docs/guides/USAGE.md](docs/guides/USAGE.md) | Ejemplos de uso |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Guía de contribución |
| [docs/guides/DEVELOPMENT.md](docs/guides/DEVELOPMENT.md) | Guía de desarrollo |
| [docs/guides/RELEASE.md](docs/guides/RELEASE.md) | Guía de lanzamiento |
| [docs/openapi.yaml](docs/openapi.yaml) | Especificación de API |

## Licencia

MIT

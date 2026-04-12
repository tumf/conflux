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

Die schnellste Anleitung, um `cflx` erstmals zu installieren, ein Projekt einzurichten, einen OpenSpec-Change zu erstellen und die Implementierung in der TUI vollständig durchzuführen.

Conflux wird über den Befehlsnamen `cflx` bereitgestellt.

## 0. Voraussetzungen

- Rust / Cargo sind verfügbar: [Rust installieren](https://rust-lang.org/tools/install/)
- [Claude Code](https://claude.com/product/claude-code) ist verfügbar
- Ein mit Git verwaltetes Projekt wie `~/myproject` ist vorhanden

> Conflux ist ein Orchestrator zum Starten und Steuern von KI-Agenten. Es ist selbst kein Coding-Agent.
> Es kann CLIs wie [Claude Code](https://claude.com/product/claude-code), [OpenCode](https://opencode.ai/) und [Codex](https://developers.openai.com/codex/cli) verwenden.
> In diesem QUICKSTART wird Claude Code als Beispiel verwendet.

Voraussetzungen prüfen:

```bash
cargo --version
claude --version
claude -p 'hi'
```

## 1. `cflx` installieren

Installieren Sie es von crates.io.

```bash
cargo install cflx
```

Installation prüfen:

```bash
cflx --version
```

## 2. Projekt vorbereiten

Ab hier arbeiten Sie im Projektverzeichnis. Als Beispiel verwenden wir `~/myproject`.

Da Conflux `git worktree` verwendet, muss das Projekt unter Git-Verwaltung stehen.

```bash
cd ~/myproject
```

Für ein neues Projekt:

```bash
mkdir -p ~/myproject
cd ~/myproject
git init
```

## 3. Bundled Skills für Claude Code installieren

Fügen Sie die bundled skills von Conflux für Claude Code dem Projekt hinzu.

```bash
cflx install-skills --claude
```

Dadurch werden `cflx-*`-Skills unter `./.claude/skills` abgelegt.

Ob sie zusammen mit `.cflx.jsonc` in Git aufgenommen werden sollen, entscheiden Sie anschließend.

## 4. Konfigurationsdatei erstellen

Die Konfigurationsdatei heißt `.cflx.jsonc`, nicht `.cflx.conf`.

Am schnellsten geht es mit der Vorlagengenerierung.

```bash
cflx init
```

Dadurch wird `.cflx.jsonc` im aktuellen Verzeichnis erstellt.

## 5. `.cflx.jsonc` prüfen

Mindestens müssen die Befehle für den gewünschten Agenten eingetragen sein.

Beispielvorlage für Claude Code:

```jsonc
{
  "analyze_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p {prompt}",
  "apply_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '{prompt}'",
  "archive_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '{prompt}'",
  "acceptance_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '{prompt}'",
  "resolve_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p {prompt}"
}
```

Beim ersten Mal reicht der von `cflx init` erzeugte Inhalt völlig aus.

## 6. Festlegen, was in Git aufgenommen wird

Beim initialen Setup entscheiden Sie, ob die folgenden zwei Elemente in Git aufgenommen werden sollen:

- `./.claude/skills/cflx-*`
- `./.cflx.jsonc`

Empfehlung:

- Wenn Sie dasselbe Verhalten im Team oder auf mehreren Rechnern reproduzieren möchten, committen Sie beide.
- Wenn die Nutzung nur lokal und eher temporär ist, nehmen Sie beide in `.gitignore` auf.

Wenn Sie unsicher sind, können Sie zunächst beide committen. Praktisch ist es, keine Geheimnisse direkt in `.cflx.jsonc` zu speichern.

Wenn beide zu `.gitignore` hinzugefügt werden sollen:

```bash
printf ".claude/skills/cflx-*\n.cflx.jsonc\n" >> .gitignore
git add .gitignore
git commit -m 'Ignore Conflux local setup files'
```

Wenn beide ins Repository aufgenommen werden sollen:

```bash
git add .claude/skills/cflx-* .cflx.jsonc
git commit -m 'Add Conflux setup files'
```

## 7. Die erste Change Proposal erstellen

Conflux verarbeitet OpenSpec-Changes.

Auch wenn Sie mit OpenSpec noch nicht vertraut sind, ist das kein Problem. Die bundled skills sind bereits installiert, sodass Sie Claude Code eine Proposal erstellen lassen können.

Zum Beispiel:

```text
/cflx-proposal python で hello world と表示する
```

Dann wird ein Change-Verzeichnis wie `openspec/changes/add-hello-world/` erzeugt, das mindestens die folgenden beiden Dateien enthält:

- `proposal.md`: was geändert werden soll
- `tasks.md`: was implementiert werden soll

Für den kürzesten Weg reicht es in der Regel, diese beiden Dateien kurz zu prüfen und sie bei korrektem Inhalt direkt zu committen.

Prüfpunkte:

- Der Inhalt von `proposal.md` entspricht der gewünschten Änderung
- Die Implementierungsaufgaben in `tasks.md` sind vollständig und weder zu knapp noch überladen
- Es sind keine unnötigen Änderungen enthalten

Passen Sie Proposal oder Tasks bei Bedarf an und committen Sie, sobald der Inhalt stimmt.

```bash
git add openspec/changes/add-hello-world
git commit -m 'proposal: add-hello-world'
```

Die detaillierte Struktur sieht wie folgt aus:

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

## 8. Prüfen, ob der Workspace sauber ist

Bevor Sie die TUI starten, prüfen Sie, ob der Arbeitsbaum sauber ist.

```bash
git status
```

Wenn alles sauber ist, sieht die Ausgabe etwa so aus:

```text
On branch main
nothing to commit, working tree clean
```

## 9. Die TUI starten

Starten Sie Conflux im TUI-Modus.

```bash
cflx
```

Es wird ein Bildschirm wie der folgende angezeigt.

![Conflux TUI ready screen](docs/images/quickstart/tui-ready-screen.png)

## 10. In der TUI ausführen

Grundlegende Bedienung:

- `↑/↓` or `j/k`: Change auswählen
- `Space`: zur Ausführung markieren
- `F5`: Ausführung starten
- `Ctrl+C`: beenden

Minimaler Ablauf:

1. `cflx` starten
2. Zum gewünschten Change wechseln
3. Mit `Space` auswählen
4. Mit `F5` ausführen

In diesem Beispiel gibt es nur einen Change, daher genügt `Space` → `F5`.

![Conflux TUI running screen](docs/images/quickstart/tui-running-screen.png)

Conflux durchläuft automatisch die folgende Schleife:

- apply
- accept
- archive
- resolve / merge

Sobald der Status `merged` erreicht ist, ist der Vorgang abgeschlossen.

![Conflux TUI merged screen](docs/images/quickstart/tui-merged-screen.png)

## 11. Ergebnis prüfen

Prüfen Sie, ob die Implementierung vorhanden ist.

```bash
tree
cat hello.py
```

Beispiel:

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

Auch die OpenSpec-Seite wurde aktualisiert.

```bash
tree openspec -L 10
```

Beispiel:

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

Daran ist zu erkennen, dass die Change Proposal archiviert wurde und die endgültige Spezifikation nach `openspec/specs` übernommen wurde.

Zum Beispiel:

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

Mit dieser Spec kann Conflux das Verhalten der Software schnell verstehen und auch bei den nächsten Änderungen stabil weiterarbeiten.

---

Damit ist der einfachste Implementierungszyklus abgeschlossen.

Dieses QUICKSTART endet an dem Punkt, an dem der erste Durchlauf auf dem kürzesten Weg erfolgreich abgeschlossen ist.
Im praktischen Einsatz können weitere Techniken nötig werden, etwa das Verfeinern von Proposals, das Anpassen der Konfiguration, parallele Ausführung oder Troubleshooting.
Weitere Informationen finden Sie im README oder über `cflx --help`.

Feedback oder Fragen gern über [GitHub Issues](https://github.com/tumf/conflux/issues) oder per Erwähnung von `@tumf` auf X.

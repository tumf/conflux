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

Le guide le plus rapide pour installer `cflx` pour la première fois, configurer un projet, créer un change OpenSpec et terminer l’implémentation dans la TUI.

Conflux est implémenté sous le nom de commande `cflx`.

## 0. Prérequis

- Rust / Cargo disponibles : [Installer Rust](https://rust-lang.org/tools/install/)
- [Claude Code](https://claude.com/product/claude-code) disponible
- Un projet versionné avec git, comme `~/myproject`

> Conflux est un orchestrateur qui lance et contrôle des agents IA. Ce n’est pas lui-même un agent de programmation.
> Il peut utiliser des CLI comme [Claude Code](https://claude.com/product/claude-code), [OpenCode](https://opencode.ai/) et [Codex](https://developers.openai.com/codex/cli).
> Dans ce QUICKSTART, nous utilisons Claude Code comme exemple.

Vérifiez les prérequis :

```bash
cargo --version
claude --version
claude -p 'hi'
```

## 1. Installer `cflx`

Installez-le depuis crates.io.

```bash
cargo install cflx
```

Vérifiez ensuite l’installation :

```bash
cflx --version
```

## 2. Préparer le projet

À partir d’ici, travaillez dans le répertoire du projet. Nous utiliserons `~/myproject` comme exemple.

Comme Conflux s’appuie sur `git worktree`, le projet doit être géré avec git.

```bash
cd ~/myproject
```

Pour un nouveau projet :

```bash
mkdir -p ~/myproject
cd ~/myproject
git init
```

## 3. Installer les bundled skills

Ajoutez les bundled skills de Conflux au projet.

```bash
cflx install-skills
```

Cela place les skills `cflx-*` sous `./.agents/skills`.

Vous déciderez ensuite, avec `.cflx.jsonc`, s’il faut les inclure dans Git.

## 4. Créer le fichier de configuration

Le nom du fichier de configuration est `.cflx.jsonc`, pas `.cflx.conf`.

Le plus simple est de générer le modèle.

```bash
cflx init
```

Cela crée `.cflx.jsonc` dans le répertoire courant.

## 5. Vérifier `.cflx.jsonc`

Au minimum, il suffit d’avoir les commandes correspondant à l’agent que vous souhaitez utiliser.

Exemple de modèle pour Claude Code :

```jsonc
{
  "analyze_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p {prompt}",
  "apply_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '{prompt}'",
  "archive_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '{prompt}'",
  "acceptance_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '{prompt}'",
  "resolve_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p {prompt}"
}
```

Pour la première exécution, le contenu généré par `cflx init` suffit largement.

## 6. Décider quoi versionner dans Git

Lors de la configuration initiale, décidez si vous souhaitez ajouter ces deux éléments à Git :

- `./.agents/skills/cflx-*`
- `./.cflx.jsonc`

Recommandation :

- Si vous voulez reproduire le même comportement en équipe ou sur plusieurs machines, versionnez les deux.
- Si l’usage reste purement local et plutôt jetable, ajoutez les deux à `.gitignore`.

En cas d’hésitation, versionner les deux est un bon choix de départ. Il est plus pratique de ne pas écrire directement d’informations sensibles dans `.cflx.jsonc`.

Pour ajouter les deux à `.gitignore` :

```bash
printf ".agents/skills/cflx-*\n.cflx.jsonc\n" >> .gitignore
git add .gitignore
git commit -m 'Ignore Conflux local setup files'
```

Pour ajouter les deux au dépôt :

```bash
git add .agents/skills/cflx-* .cflx.jsonc
git commit -m 'Add Conflux setup files'
```

## 7. Créer la première change proposal

Conflux traite des changes OpenSpec.

Même si vous ne connaissez pas encore bien OpenSpec, ce n’est pas un problème. Les bundled skills sont déjà installés, vous pouvez donc demander à Claude Code de générer une proposal.

Par exemple :

```text
/cflx-proposal python で hello world と表示する
```

Un répertoire de change comme `openspec/changes/add-hello-world/` sera alors créé, avec au moins les deux fichiers suivants :

- `proposal.md` : ce qui va être modifié
- `tasks.md` : ce qui doit être implémenté

Pour le chemin le plus court, il suffit généralement de parcourir rapidement ces deux fichiers et, si tout semble correct, de les valider tels quels.

Points à vérifier :

- Le contenu de `proposal.md` correspond bien au changement souhaité
- Les tâches d’implémentation dans `tasks.md` sont complètes, sans manque ni excès
- Aucun changement parasite ne s’y est glissé

Si nécessaire, modifiez la proposal ou les tasks, puis committez une fois le contenu validé.

```bash
git add openspec/changes/add-hello-world
git commit -m 'proposal: add-hello-world'
```

La structure détaillée ressemble à ceci :

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

## 8. Vérifier que l’espace de travail est propre

Avant de lancer la TUI, assurez-vous que l’arborescence de travail est propre.

```bash
git status
```

Si tout est propre, vous verrez quelque chose comme :

```text
On branch main
nothing to commit, working tree clean
```

## 9. Lancer la TUI

Démarrez Conflux en mode TUI.

```bash
cflx
```

L’écran suivant s’affiche.

![Conflux TUI ready screen](docs/images/quickstart/tui-ready-screen.png)

## 10. Exécuter dans la TUI

Commandes de base :

- `↑/↓` or `j/k` : sélectionner une change
- `Space` : marquer pour exécution
- `F5` : démarrer l’exécution
- `Ctrl+C` : quitter

Flux minimal :

1. Lancez `cflx`
2. Déplacez-vous jusqu’à la change à traiter
3. Sélectionnez-la avec `Space`
4. Exécutez avec `F5`

Dans cet exemple, il n’y a qu’une seule change ; utilisez donc `Space` → `F5`.

![Conflux TUI running screen](docs/images/quickstart/tui-running-screen.png)

Conflux exécute automatiquement la boucle suivante :

- apply
- accept
- archive
- resolve / merge

Quand l’état passe à `merged`, c’est terminé.

![Conflux TUI merged screen](docs/images/quickstart/tui-merged-screen.png)

## 11. Vérifier le résultat

Vérifiez que l’implémentation a bien été ajoutée.

```bash
tree
cat hello.py
```

Exemple :

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

Le côté OpenSpec a également été mis à jour.

```bash
tree openspec -L 10
```

Exemple :

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

On voit que la change proposal a été archivée et que la spécification finale a été promue vers `openspec/specs`.

Par exemple :

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

Grâce à cette spec, Conflux peut comprendre rapidement le comportement du logiciel et avancer de façon stable sur les modifications suivantes.

---

Vous avez ainsi terminé le cycle d’implémentation le plus simple.

Ce QUICKSTART s’arrête au parcours le plus court pour la toute première exécution.
En usage réel, vous pourrez avoir besoin de techniques plus fines : affiner la proposal, ajuster la configuration, exécuter en parallèle ou résoudre des problèmes.
Consultez la suite dans le README ou avec `cflx --help`.

Pour vos retours ou questions, utilisez les [GitHub Issues](https://github.com/tumf/conflux/issues) ou mentionnez `@tumf` sur X.

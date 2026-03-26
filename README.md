# ghx

A TUI for browsing GitHub from the terminal, powered by the `gh` CLI.

## Features

- Browse your repositories
- View issues and pull requests for any repo
- Read issue/PR details with rendered markdown and comments
- Check GitHub notifications
- Open any item in the browser (`o`)
- Read issue/PR body in mdr (`r`)
- Repo context mode — run `ghx` inside a repo to jump straight to its issues
- Vim-style navigation (j/k, enter, esc)
- Tokyo Night Moon color theme

## Requirements

- [Go 1.21+](https://go.dev/)
- [GitHub CLI (`gh`)](https://cli.github.com/) — must be installed and authenticated (`gh auth login`)
- [mdr](https://github.com/jrf/mdr) (optional) — for reading issue/PR bodies in the full markdown reader

## Install

```bash
git clone <repo-url>
cd ghx
just          # builds and installs to ~/.local/bin/
```

Or manually:

```bash
go build -o ghx .
mv ghx ~/.local/bin/
```

## Usage

```bash
ghx              # browse all your repos
cd some-repo
ghx              # jump straight to issues/PRs for this repo
```

### Key Bindings

| Key | Action |
|-----|--------|
| `j` / `k` | Navigate up/down |
| `Enter` | Select / drill down |
| `Esc` / `Backspace` | Go back |
| `Tab` | Switch tabs |
| `o` | Open in browser |
| `r` | Read body in mdr (detail view) |
| `?` | Toggle help |
| `q` / `Ctrl+C` | Quit |

### Navigation

```
Repos → Select repo → Issues / Pull Requests → Select → Detail view
```

Use `Tab` on the home screen to switch between Repos and Notifications. Use `Tab` on the issues screen to switch between Issues and Pull Requests.

# ghx

A TUI for browsing GitHub from the terminal, powered by the `gh` CLI.

## Features

- Browse your repositories
- Browse starred repositories and GitHub Lists
- Switch repository sources from a searchable picker instead of tabbing through organizations
- Search repositories globally
- View issues and pull requests for any repo
- Read issue/PR details with rendered markdown and comments
- Review pull request diffs
- Check GitHub notifications
- Open notification issues and pull requests directly
- Mark notifications as read
- Open any item in the browser (`o`)
- Switch between authenticated GitHub CLI accounts without restarting (`A`)
- Toggle a body-only reader for issues and pull requests (`r`)
- Repo context mode — run `ghx` inside a repo to jump straight to its issues
- Vim-style navigation (`j`/`k`, `Enter`, `Esc`)
- Breadcrumb titles and context-aware action hints on every screen
- Built-in and custom color themes

### Theme roles

Custom themes live in `~/.config/ghx/themes/`. Their `[ui]` section can assign `accent` for branded chrome, `selection` for active tabs and rows, `key` for shortcut and filter prompts, and `heading` for titles. Themes without the newer `selection` or `key` entries remain compatible through palette-based fallbacks.

## Requirements

- [Rust and Cargo](https://www.rust-lang.org/tools/install) with Rust 2024 Edition support
- [GitHub CLI (`gh`)](https://cli.github.com/) — must be installed and authenticated (`gh auth login`)
- [just](https://github.com/casey/just) (optional) — for build and install recipes

## Install

```bash
git clone <repo-url>
cd ghx
just          # lists available recipes
just install  # builds and installs to ~/.local/bin/
```

Or manually:

```bash
cargo build --release
cp target/release/ghx ~/.local/bin/
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
| `s` | Choose My Repos, Starred, or an organization |
| `[` / `]` | Switch to the previous or next repository source |
| `o` | Open in browser |
| `r` | Toggle body-only reader in issue/PR detail |
| `m` | Mark the selected notification as read |
| `/` | Filter the current list or edit repository search |
| `A` | Switch the active GitHub CLI account and reload |
| `a` | Show actions available on the current screen |
| `t` | Open the theme picker |
| `?` | Show screen-aware help |
| `q` / `Ctrl+C` | Quit |

### Navigation

```
Repos / Lists / Search → Repository → Issues / Pull Requests → Conversation / Diff
Notifications → Issue / Pull Request → Conversation / Diff
```

Use `Tab` and `Shift+Tab` for primary home sections, repository tabs, and pull-request conversation/diff tabs. Use `s` to search repository sources or `[` and `]` to cycle them.

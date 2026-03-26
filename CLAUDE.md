# CLAUDE.md

## Project Overview

ghx is a TUI for browsing GitHub, built with Go + Bubbletea. It shells out to `gh` (GitHub CLI) for all data access.

## Build Commands

```bash
just          # Build release + install to ~/.local/bin
just build    # Debug build
just run      # Run directly
just clean    # Remove build artifacts
```

## Architecture

### `main.go`
Entry point. Detects current repo via `gh repo view` for context mode, then creates a Bubbletea program with alt-screen.

### `internal/gh/`
- **`gh.go`** — Wrapper around the `gh` CLI. All GitHub data flows through here via `gh` subprocess calls with JSON output. Types: `Repo`, `Issue`, `PR`, `IssueDetail`, `Comment`, `Notification`.
- **`context.go`** — Repo context detection (`CurrentRepo()`) and browser-open helpers (`OpenRepoInBrowser`, `OpenIssueInBrowser`, `OpenPRInBrowser`).

### `internal/ui/`
- **`app.go`** — Root Bubbletea model. Manages screen stack (repos → issues → detail), tab switching, global key handling, open-in-browser (`o`), and open-in-mdr (`r` via `tea.ExecProcess`). Supports context mode (skip to issues when launched inside a repo).
- **`repo_list.go`** — Repository list view with scrolling and selection.
- **`issue_list.go`** — Issue/PR list with sub-tab toggle between issues and PRs.
- **`repo_detail.go`** — Repository detail view with description, stats, language, license, topics, and glamour-rendered README. Scrollable.
- **`detail.go`** — Full issue/PR detail view with glamour-rendered markdown body, labels, comments, and scrolling. Caches rendered lines per width.
- **`search.go`** — GitHub repo search with query input and results list. Supports fuzzy filtering within results.
- **`notifications.go`** — GitHub notifications list with drill-down to issue/PR detail and mark-as-read support.
- **`styles.go`** — Tokyo Night Moon color palette and lipgloss styles.
- **`keys.go`** — Key bindings (vim-style j/k, enter, esc, tab, o, r, etc.).

### Navigation Flow

```
Home (repos/search/notifications tabs)
  → Select repo → Repo Detail (description, stats, README)
    → Enter → Issues/PRs (sub-tabbed)
      → Select issue/PR → Detail view (body + comments)
  → Select notification → Detail view (issue/PR body + comments)
```

Context mode: when run inside a git repo, skips straight to that repo's issues (repo detail pre-loaded for back navigation).

PRs show CI check status indicators: ✓ pass, ✗ fail, ● pending.

`esc`/`backspace` goes back one level. `q` quits from the top level. `o` opens current item in browser. `r` opens issue/PR body in mdr (detail view only). `m` marks a notification as read. `c` clones the selected repo to `~/repos`.

## Dependencies

- `github.com/charmbracelet/bubbletea` — TUI framework (Elm architecture)
- `github.com/charmbracelet/bubbles` — Pre-built components (key bindings)
- `github.com/charmbracelet/lipgloss` — Terminal styling
- `github.com/charmbracelet/glamour` — Markdown rendering in terminal
- External: `gh` CLI must be installed and authenticated
- External (optional): `mdr` for full markdown reading experience

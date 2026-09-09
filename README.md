# zygorguide-lsp

A Language Server for the Zygor Guide DSL used in WoW addon guide files. Provides diagnostics, completions, hover info, navigation, and quick-fixes by validating guide content against the pfQuest/pfQuest-epoch game database.

## Setup

### Build

```bash
cd zygorguide-lsp
cargo build --release
```

The binary is at `target/release/zygorguide-lsp`.

### Neovim

Add to your Neovim config. The LSP attaches to `.lua` files inside `Guides/` directories:

```lua
vim.api.nvim_create_autocmd('FileType', {
  pattern = 'lua',
  callback = function(args)
    local path = vim.api.nvim_buf_get_name(args.buf)
    if not path:match('Guides/') then return end

    local root = vim.fs.dirname(
      vim.fs.find({ 'pfQuest-epoch', 'pfQuest' }, {
        upward = true,
        path = vim.fs.dirname(path),
      })[1] or ''
    )
    if root == '' then return end

    vim.lsp.start({
      name = 'zygorguide-lsp',
      cmd = { '/path/to/zygorguide-lsp' },
      root_dir = root,
    })
  end,
})
```

### Database Location

The LSP loads quest/NPC/item/object/zone data from the pfQuest database at startup. It auto-detects `pfQuest/` and `pfQuest-epoch/` directories by searching at the workspace root and up to two parent directories.

To set explicit paths:

```lua
vim.lsp.start({
  -- ...
  settings = {
    zygorguide = {
      databasePaths = {
        pfQuest = '/path/to/pfQuest',
        pfQuestEpoch = '/path/to/pfQuest-epoch',
      }
    }
  }
})
```

## Features

### Diagnostics

Errors and warnings appear automatically as you type. No action needed — the LSP publishes diagnostics on every file open and edit.

| What it catches | Severity | Example |
|---|---|---|
| Entity ID doesn't exist in database | Error | `accept Fake Quest##99999` |
| Quest ID in `\|q` doesn't exist | Error | `\|q 99999/1` |
| Unmatched `stickystart` (no `label` or `stickystop`) | Error | `stickystart "X"` with no `label "X"` |
| ID exists but in wrong table for the action | Warning | Quest ID used with `kill` |
| Name doesn't match database | Warning | `WANTED:Dustpaw` vs `WANTED: Dustpaw` |
| Zone name in `\|goto` not found | Warning | `\|goto Fake Zone 50,50` |
| Coordinates outside 0–100 range | Warning | `\|goto Durotar 150,50` |
| `label` with no matching `stickystart` | Warning | orphan label |

The type checker is ID-namespace-aware — quests, NPCs, items, and objects can share the same numeric ID without false positives. The LSP checks the expected table first based on the action keyword (e.g., `kill` checks units, `accept` checks quests).

### Completions

Trigger completions by typing. The LSP offers context-aware suggestions:

| What you type | What you get |
|---|---|
| Start of a line | Action keywords (`accept`, `turnin`, `talk`, `kill`, `collect`, ...) |
| `\|` | Pipe directives (`goto`, `q`, `only`, `c`, `future`, `tip`, ...) |
| `accept ` | Quest names from database — selecting inserts `Name##ID` |
| `turnin ` | Quest names |
| `talk ` / `kill ` | NPC names — selecting inserts `Name##ID` |
| `collect ` / `use ` / `buy ` | Item names — selecting inserts `Name##ID` |
| `click ` | Object names — selecting inserts `Name##ID` |
| `map ` | Zone names |
| `label "` | Label names from `stickystart` declarations in the current guide |

Completions are prefix-filtered and case-insensitive. Entity completions show the ID and level in the detail text. A leading count number is handled — `kill 10 Ko` searches for names starting with "Ko".

### Hover

Hover over any `Name##ID` reference to see database info:

- **Quest**: title, level, min level, objectives, start/turn-in NPCs, prerequisites
- **NPC**: name, level, faction (Alliance/Horde/Both), up to 3 spawn locations with zone and coordinates
- **Item**: name, up to 5 drop sources with drop %, up to 3 vendors
- **Object**: name, up to 3 spawn locations
- **`|q 123/1`**: quest title, level, objective number
- **`|goto Zone`**: zone name and ID
- **Labels**: label name

### Go-to-Definition

Use go-to-definition (typically `gd` in Neovim) on label references to jump between paired lines:

| Cursor on | Jumps to |
|---|---|
| `stickystart "X"` | `label "X"` |
| `label "X"` | `stickystart "X"` |
| `\|next "X"` | `label "X"` |

### Find References

Use find-references (typically `gr` in Neovim) to locate all occurrences:

| Cursor on | Finds |
|---|---|
| Any `Name##ID` | All references to the same entity ID across all guides in the file |
| `\|q 123` | All `\|q` pipes and `Name##123` refs with that quest ID |
| A label name | All `stickystart`, `label`, and `\|next` refs with that label |

### Code Actions (Quick Fixes)

When the cursor is on a `Name##ID` with a diagnostic, use code actions (typically `<leader>ca` in Neovim):

- **"Fix name to 'Correct Name'"** — when the guide uses a different name than the database for a valid ID. Replaces the entire `Name##ID` with the correct name.
- **"Did you mean 'Name' (ID 123)?"** — when the ID doesn't exist in the expected table, suggests the closest match by name. Replaces with the suggested name and correct ID.

### Workspace Indexing

On startup, the LSP scans all `Guides/**/*.lua` files containing `RegisterGuide` calls and parses them into memory. This means find-references and diagnostics work across all guide files immediately, not just open files.

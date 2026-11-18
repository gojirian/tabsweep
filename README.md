# Tab Sweep

A small desktop app that looks at the tabs open in Microsoft Edge, scores each one for how likely it is to be junk, and closes the ones you tick.

There is no AI in it. Each tab's score comes from plain rules applied to its title, and every row says which rules matched.

## What it does

1. Reads the title of every tab in every Edge window.
2. Gives each tab a score from 0 to 100.
3. Lists them in three groups: **Close** (60 and up, ticked for you), **Worth a look** (35 to 59) and **Keep**.
4. Closes the ticked tabs when you press **Close tabs**.

Each row has an **Open** button that switches Edge to that tab, so you can check a tab before deciding. Clicking anywhere else on a row ticks or unticks it.

The filter box narrows the list by title. **Tick shown** and **Untick** apply to whatever the filter leaves on screen, which makes "close everything from one site" two clicks.

## How tabs are scored

| Rule | Points |
|---|---|
| Error page | +80 |
| Blank tab | +70 |
| Duplicate of another tab | +60 |
| Page source view | +60 |
| A single image | +55 |
| One-off notice, such as "installation successful" | +50 |
| Sign-in page | +45 |
| Old search results | +40 |
| Social feed | +25 |
| Shopping | +20 |
| Discarded by the browser to save memory | +15 |
| Sleeping | +8 |
| Using 500 MB or more | +10 |
| One of four or more tabs from the same site | +10 |
| Pinned | −60 |
| The tab on screen in its window | −40 |
| In a tab group | −25 |
| In a split view | −10 |

Of a set of duplicates, one is kept: the tab on screen, then one that is pinned or in a tab group, then one that is still loaded, then the first.

The rules are in `src/score.rs`.

### Your own rules

The **Rules** button opens `rules.txt` in a text editor. Each line names a word to look for in tab titles, in any case:

```
close: gadget shop
keep: acme portal
```

A title containing a `close` word is always suggested. A title containing a `keep` word never is. Press **Rescan** after saving.

## Closing tabs

Edge has no way for another program to close a tab, so Tab Sweep selects each tab and sends the close-tab shortcut. While it does this:

- Edge comes to the front and tabs close one at a time. Leave Edge alone until it finishes.
- The shortcut is only sent after checking that Edge is in front and the right tab is selected.
- A tab that has moved or changed title since the scan is skipped.
- If a page asks "Leave site?", closing stops there.

The reopen-closed-tab shortcut in Edge brings tabs back, most recent first.

## Platforms

Tab Sweep reads and closes tabs on **Windows**. It builds and opens on macOS and Linux, where it says that tab reading is not available yet.

The browser code is behind three functions in `src/edge/mod.rs`. `src/edge/windows.rs` implements them with PowerShell scripts that use UI Automation (`scripts/`). Supporting another system means adding a backend file with the same three functions.

On Windows it needs PowerShell, which is always present. PowerShell 7 is used when installed.

## Files

Tab Sweep keeps two folders, found through each system's own environment variables:

| | Settings (`rules.txt`) | Working files |
|---|---|---|
| Windows | `%APPDATA%\tabsweep` | `%LOCALAPPDATA%\tabsweep` |
| macOS | `~/Library/Application Support/tabsweep` | `~/Library/Caches/tabsweep` |
| Linux | `$XDG_CONFIG_HOME/tabsweep` or `~/.config/tabsweep` | `$XDG_CACHE_HOME/tabsweep` or `~/.cache/tabsweep` |

The working folder holds copies of the scripts, written on each run. It can be deleted at any time.

On Windows, `tabsweep.ico` is written beside the executable so a shortcut can use it.

## Build and run

```sh
cargo run --release
```

The executable is `target/release/tabsweep` (`tabsweep.exe` on Windows). It is self-contained: copy it anywhere and run it.

`tabsweep --preview-closing` opens on the closing animation and touches nothing, for looking at it.

## Development

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

CI runs these on Windows and macOS.

| File | Role |
|---|---|
| `src/main.rs` | The window: rows, buttons, and the worker thread |
| `src/score.rs` | The scoring rules |
| `src/edge/` | Reading, showing and closing tabs, one backend per system |
| `src/paths.rs` | Folders, text editor and shortcut names per system |
| `src/logo.rs` | The mark and its animation, drawn from shapes |
| `src/icon.rs` | The mark as pixels for the window icon |
| `src/theme.rs`, `src/fonts.rs` | Colours and fonts |
| `scripts/` | The PowerShell scripts the Windows backend runs |

## Fonts

Fira Sans and Fira Code are embedded from `assets/` under the SIL Open Font License. The licence texts are beside them.

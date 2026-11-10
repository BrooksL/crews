# herdr polished shell: implementation spec

Nov 6, 2026 · Lee Brooks

## Goal and scope

Make herdr's TUI read like a desktop app: layered surfaces, rounded cards and panels, buttons with hover, and floating toasts and menus. Build it from terminal cells plus kitty-graphics images only, and never change what apps draw inside their panes.

- **Design reference:** the "Polished window" and "Who draws what" artboards on the [herdr spaces pane canvas](https://claude.ai/code/artifact/32de5ea8-9911-464e-a568-e2d0d1d1bc36). The canvas is private, so this spec carries every value Claude Code needs.
- **In scope:** the sidebar, a pane panel with a header that replaces the tab bar, floating layers (toasts, the global menu, a dimming scrim), the status bar, hover and mouse pointer shapes, rounded shapes with a square fallback, Claude config labels, theme tokens and config keys.
- **Out of scope:** restyling app content inside panes, Ghostty itself, upstream pull requests, and a third sidebar pane (deferred).
- **Target:** Lee's fork of herdr, built and run locally. Everything sits behind an opt-in setting, `ui.style = "app"`. The default, `"classic"`, keeps today's look.

## Ground rules for Claude Code

Plan each phase before writing code, build one phase at a time, and keep classic rendering untouched.

- **Read first:** `AGENTS.md` and `CONTRIBUTING.md`. Follow "render is pure", the multiplicative performance paths, and the external contributor guardrail: no pushes and no pull requests to herdrdev/herdr unless Lee asks.
- **Plan, then wait:** before each phase in Delivery phases, reply with the files to touch, new types, config keys and tests. Wait for Lee's OK.
- **Opt-in only:** every visual change is gated by `ui.style = "app"`. With `"classic"`, output and existing tests stay exactly as they are.
- **Reuse what herdr already has:**
  - Images: `src/kitty_graphics/`, `src/server/client_shell_graphics.rs`, `client_supports_direct_graphics`, `HostCellSize`, the `kitty_graphics_enabled` setting.
  - Hover: `src/client/shell/mouse.rs` already handles mouse movement for menus; `link_hover.rs` for links.
  - Toasts: `render_notification_card` in `src/client/shell/notifications.rs`.
  - Menu: `src/client/shell/global_menu.rs` and `context_menu.rs`.
  - Sidebar: `src/client/shell/sidebar.rs`, `agent_sidebar.rs`; tabs: `tabs.rs`; mode bar: `render_mode_bar` in `render.rs`.
  - Process environment: `parse_agent_env_hint` in `src/platform/mod.rs`.
- **Colors:** every color comes from a Palette token (`src/app/state.rs`), overridable through `theme.custom`. No hex values in render code.
- **Checks:** `just test` and `just lint` pass at the end of every phase. Run `just bench-render-scale` whenever render loops do more work, and report the difference.
- **When the spec is silent:** ask. List assumptions in each phase's summary.

## Grid, units and shapes

Everything snaps to the cell grid. Softness comes from half-row caps and small images in corner cells, with a square fallback when images aren't available.

- **Units:** layout code works in columns and rows only. Pixels appear only inside image generation, taken from `HostCellSize`. Lee's Ghostty cell is about 9.45 × 21 px.
- **One shared shape module** (for example `src/client/shell/shape.rs`) that every component uses:
  1. **Card** (one or more rows): content rows filled with the card color. The row above is `▄` (U+2584) with fg = card color and bg = the surface beneath; the row below is `▀` (U+2580). That gives half a row of padding top and bottom, and adjacent cards read as a one-row gap.
  2. **Pill** (one row): the label plus 1 column of padding on each side, all filled. The padding cells become the rounded ends.
  3. **Panel:** a card that spans a whole region, with a larger radius.
  4. **Floating surface** (toast, popover): a card plus a shadow.
- **Rounded mode:** applies when the client supports direct kitty graphics, its cell size in pixels is known, and `ui.corners` isn't `"square"`. Each corner cell gets a small RGBA image placed under the text that paints the fill with a rounded corner; the cell's own background is the surface beneath. Pill ends get a half-ellipse the full height of the cell.
- **Radii:** cards 8 px, panel 10 px, pills fully round. Clamp any radius to the smaller of the cell width and half the cell height.
- **Square mode:** corner cells use the same `▄` / `▀` as the rest of the edge, and pill ends are plain filled cells. Never use quadrant characters (`▗▖▝▘`).
- **Shadows** (rounded mode only): toasts and popovers get a soft shadow image around them, offset 10 px down, about 28 px blur, black at 45%, blended over whatever is beneath. Square mode has no shadow.
- **Image cache:** key images by fill color, color beneath, radius, cell size and which corner. Resend when any of those change (theme reload, font size change) and delete them when the shape goes away.
- **Config:** `ui.corners = "auto" | "rounded" | "square"`, default `"auto"`.

## Colors and theme tokens

Add these tokens to the Palette, each overridable through `theme.custom`. The defaults below are Lee's theme; reuse an existing Palette token wherever one already holds the same value.

| Token | Lee's value | Used for |
| --- | --- | --- |
| `chrome_bg` | #1f2228 | Window behind the panel: sidebar, gutters, status bar |
| `pane_bg` | #292c33 | Panel fill. Must equal Ghostty's background so the app's default-background cells blend in |
| `card_bg` | #2b2e36 | Space and agent cards, off chips, pills |
| `card_hover_bg` | #32363f | Any card or button under the mouse |
| `card_focus_bg` | #3a3f4d | Focused space card, active tab |
| `segment_bg` | #23262c | Track behind the tabs |
| `hairline` | #353943 | Line under the panel header |
| `rule` | #33363e | Rules beside section headings |
| `float_bg` | #363b47 | Toasts and popovers |
| `float_hover_bg` | #454b5b | Hovered popover row |
| `float_divider` | #4a4f5c | Divider inside popovers |
| `scrim` | #0a0b0e at 50% | Dims everything behind an open menu |
| `text_strong` | #eceef8 | Focused names, header title |
| `text` | #c3c8dd | Names |
| `text_secondary` | #a0a4b8 | Inactive tabs, off chips, icon buttons |
| `text_muted` | #9a9eb2 | Tab and crew names in agent cards, the word "idle" |
| `label` | #8e92a8 | Section headings, key hints, session name |
| `toast_body` | #b4b8c9 | Toast body text |
| accent (`mauve`) | #c5a7f2 | On chips, branch name, primary button. Text on it: #1e1e2d |
| idle (`green`) | #b3e1a7 | `○` |
| done (`teal`) | #a5e0d5 | `✓` |
| working (`yellow`) | #f9e2af | `◐` |
| blocked (`red`) | #f38ba8 | `×` and "needs you" |

- **Built-in themes:** derive defaults from each theme's existing tokens (`chrome_bg` a step darker than `sidebar_bg`, cards a step lighter than `chrome_bg`).
- **Contrast:** every text color must reach 4.5:1 on the surface it sits on. Add a unit test that checks this for every built-in theme.
- **Badge colors** come from the config labels in Claude config labels, not from the theme.

## Window layout

The app's panel sits on the darker chrome between the sidebar and the status bar, with one-column gutters around it.

```
┌─────────────────────────────────────────────────────────────────┐
│ sidebar · 36 cols  │ ╭─ panel ────────────────────────────────╮ │
│ [spaces] [agents]  │ │ header: space · branch · badge · tabs  │ │
│ spaces ──────────  │ ├──────────────── hairline ──────────────┤ │
│ ▢ space card       │ │                          ╭─ toast ─╮   │ │
│ ▢ space card       │ │                          ╰─────────╯   │ │
│ agents ──────────  │ │   pane content, drawn by the app       │ │
│ ▢ agent group card │ │   2 cols padding each side             │ │
│ (+ new space)(menu)│ ╰────────────────────────────────────────╯ │
│ status bar: mode · key hints · counts · session                 │
└─────────────────────────────────────────────────────────────────┘
```

W and H are herdr's grid in columns and rows. The mockup is 177 × 48; Lee's full window is about 270 × 64.

| Region | Columns | Rows |
| --- | --- | --- |
| Sidebar | 0–35, cards in 1–34 | 0 to H−2 |
| Gutters | 35 and W−1 | 0 to H−2 |
| Panel | 36 to W−2 | caps at 0 and H−2 |
| Header | panel +2 to panel −2 | 1, with the hairline under it |
| App content | panel +2 to panel −2 | 3 to H−4 (2 and H−3 are padding) |
| Status bar | 1 to W−2 | H−1 |

## Sidebar

The sidebar is 36 columns on `chrome_bg`: a chip row, then the spaces and agents sections as cards, then the new space and menu buttons. Row numbers below are for Lee's 48-row window; the Window layout section gives them for any height.

**Rows**

- Row 0 blank. Row 1: chips `spaces` and `agents` from column 1, 1 column apart; a `«` collapse button in columns 32–34.
- Row 2 blank. Sections start at row 3.
- Both sections on: spaces heading at row 3 with its list in rows 4–24; agents heading at row 25 with its list in rows 26–43. Spaces gets about 54% of the rows, floored to whole rows; once the user drags the existing section split, use `sidebar_section_split` instead.
- One section on: heading at row 3, list in rows 4–43. Both off: `all panes hidden` in the `label` color at row 3.
- Row 45: a `+ new space` pill from column 1, and a `≡ menu` pill whose right edge is column 34. Row 46 blank.

**Chips**

- Pill shape. On: accent fill, bold #1e1e2d text. Off: `card_bg` fill, `text_secondary` text; `card_hover_bg` on hover.
- A click toggles the section. Add bindable actions `toggle_sidebar_spaces` and `toggle_sidebar_agents`. Save visibility where herdr saves `sidebar_section_split`.

**Section headings**

- Bold heading in the `label` color at column 1, then a 1-column gap and a `─` rule in the `rule` color out to column 34.
- The agents heading ends with the existing `grouped` toggle, drawn as a pill.

**Space cards**

- Card shape, columns 1–34, one content row: status symbol at column 3, name at column 5, config badge ending at column 32. Shorten the name with `…` before the badge ever gets cut.
- States: `card_bg` by default; `card_hover_bg` on hover; focused = `card_focus_bg` with a bold `text_strong` name. Navigate-mode selection and drag keep today's colors, applied to the card fill.
- The click target is the whole card, cap rows included. Update row heights and scroll metrics for the 2 cap rows.
- If a user's `ui.sidebar.spaces.rows` adds a second row (for example the branch), it lines up with the name in `text_muted`, or the accent when focused.
- Status symbols are always on in app style: `○` idle, `◐` working, `✓` done, `×` blocked, colored as in the token table. A space shows herdr's existing combined status.

**Agent cards**

- Grouped mode: one card per group herdr already makes (a space, or a space's tab or crew). The header row has the space name in bold `text`, the tab or crew name in `text_muted`, and the badge on the right. Drop the tab name when fewer than 8 columns are left for it.
- One row per agent, indented 2 columns: status symbol and agent name on the left; on the right the state word ("needs you", "working", "done", "idle") in its status color, with "idle" in `text_muted`.
- Ungrouped mode: one single-row card per agent with its status symbol, name, the space name in `text_muted`, and the badge.

**Scrolling and collapsed mode**

- Lists scroll by whole rows with the wheel and the keyboard. App style draws no scrollbar; content is clipped at the bottom of its section.
- The collapsed (compact) sidebar keeps today's behavior, without chips.

## Pane panel and header

The pane area becomes one rounded panel inset from the window. Its first row is a header that replaces the tab bar.

**Panel**

- Panel shape with `pane_bg`, from column 36 to column W−2, leaving 1-column gutters at column 35 and column W−1. Cap rows at row 0 and row H−2. Exact geometry is in Window layout.
- Row 1 is the header, row 2 is padding, the app's content fills rows 3 to H−4, and row H−3 is padding.
- The app gets 2 columns of padding on each side. The size reported to the pane's terminal is that inner area; resize the PTY to match.

**Header (row 1, 2 columns in from each panel edge)**

- Left side, in order with 1-column gaps:
  1. The space name in bold `text_strong`.
  2. The branch in the accent color, when there is one.
  3. The config badge.
  4. A 2-column gap, then the tabs as a segmented control: one `segment_bg` pill behind all tabs, each tab a pill reading `<number> <name>`. Active tab: `card_focus_bg`, bold `text_strong`, number in the accent color. Inactive: no fill, `text_secondary`, number in `label`. A zoomed tab keeps today's ` Z` suffix.
  5. A `+` new tab button, 3 columns wide.
- Too many tabs: scroll them as `tabs.rs` does today (`max_tab_scroll`), with `‹` and `›` at the edges.
- Right side: icon buttons 3 columns wide, 1 column apart: `◫` split, `⤢` zoom, `⋯` more (opens the pane's existing context menu). Each runs the existing action and shows a `card_hover_bg` pill on hover.
- Under the header: a 1-pixel line made by underlining every header cell with the underline color set to `hairline` (SGR 4 plus SGR 58). It costs no row. Skip it on terminals without colored underlines.
- The header always shows in app style, even with one tab, because it carries the space name. `ui.tab_bar_position` and `hide_tab_bar_when_single_tab` apply only to classic.

**Splits**

- Not in the mockup. Proposed: each pane in a split gets its own panel, separated by a 1-column (side by side) or 1-row (stacked) gutter in `chrome_bg`, with one header per tab. See Open questions.
- Proposed: unfocused panes dimmed by blending each cell's foreground 20% toward its background, set by `ui.dim_unfocused` (0 turns it off).

## Floating layers

Toasts and menus float above everything as `float_bg` cards with shadows. An open menu dims everything else.

**Toast**

- Built on herdr's existing notification events and `render_notification_card`. In app style the default position is the panel's top-right corner: 44 columns wide, right edge 2 columns in from the panel's right edge, top cap on row 2.
- Three content rows:
  1. Status symbol and a bold `text_strong` title (for example `× check-ins needs you`), with the config badge on the right.
  2. Body in `toast_body`, indented 2 columns (for example `lead is blocked on a permission prompt`).
  3. Actions, indented 2 columns: a primary `open` pill (accent fill, bold #1e1e2d text) and a text-only `dismiss` button.
- `open` focuses that space and pane and closes the toast. Several toasts stack downward with a one-row gap, at most 3 visible. Auto-dismiss follows the existing notification settings.

**Global menu popover**

- Opens from the `≡ menu` button. Right edge aligned with the button's right edge (column 34); bottom edge half a row above the button. 25 columns wide, 1 column of inner padding.
- Rows: the existing global menu items in their current order: `settings`, `keybinds`, `reload config`, `what's new` or `update ready` when present, a divider row (`─` in `float_divider`), then `detach` with its key hint right-aligned (`⌃B q` by default, read from the keybinds rather than hard-coded).
- The hovered or keyboard-selected row gets a `float_hover_bg` pill across the inner width. The first row is selected on open. Up, down, Enter and Esc work as they do today.
- Pane context menus use the same popover, opening at the pointer or the `⋯` button.

**Scrim**

- While a menu is open, recolor every cell outside it, pane content and toasts included: blend foreground and background 50% toward `scrim`. A click on the scrim closes the menu.
- Larger overlays (settings, keybinds, onboarding, release notes) use the same floating surface and scrim, centered in the panel.

## Status bar

The last row (H−1) is a status bar on `chrome_bg`, from column 1 to column W−2. It replaces the mode bar in app style.

- **Left:** the current mode as a pill, upper case (`TERMINAL`, `PREFIX`, `NAVIGATE`), `card_bg` fill with bold `text_secondary` text. Then 2 spaces and key hints in `label`, for example `⌃B prefix · ⌃B q detach`. Build the hints from the configured keybinds. In prefix mode, show what the mode bar shows today.
- **Right:** counts across all spaces, 3 spaces apart, each a status symbol in its color plus a number and word in `text`: `× 1 needs you`, `◐ 1 working`, `✓ 2 done`, `○ 6 idle`. Hide any count that is zero. The session name in `label` comes last.
- Clicking a count does nothing for now.

## Hover and pointer

Everything herdr draws that can be clicked reacts to the mouse, and the pointer turns into a hand over it.

- **Hover targets:** space cards, chips, the collapse button, tabs, `+`, the header icon buttons, the bottom pills, popover rows, toast buttons.
- **Hover style:** one step lighter. Cards and pills go to `card_hover_bg`; buttons with no fill get a `card_hover_bg` pill; popover rows get `float_hover_bg`.
- **Mechanics:** extend the mouse-move handling that menus already use. Keep the hovered target's id in the client shell state and redraw only when it changes. Clear it when the mouse leaves the window or focus is lost.
- **Pointer shape:** send OSC 22 (`ESC ] 22 ; pointer ST`) on entering a clickable target and `default` on leaving it. Send only on change. Never send over pane content, which belongs to the app. Restore `default` on detach and exit.
- **Config:** `ui.pointer_shapes = "auto" | "on" | "off"`. `auto` turns it on when the attached terminal is Ghostty or kitty (`TERM_PROGRAM` on the client side).

## Claude config labels

Every card, panel header and toast shows which Claude config is running: `claude` (personal) or `pco` (work, launched as `pco-claude`).

- **Why detection is needed:** herdr identifies both as the `claude` agent because it only looks at the process name. `pco-claude` starts `claude` with `CLAUDE_CONFIG_DIR` set to the work config folder. Check Lee's shell config for the exact path. If `pco-claude` turns out to be a wrapper script, say so and match on the parent process name instead.
- **How:** read `CLAUDE_CONFIG_DIR` from the agent process's environment, the same way herdr reads `HERDR_AGENT` (`parse_agent_env_hint`: `/proc/<pid>/environ` on Linux, procargs2 on macOS). Do it in the detection and state layer and cache it per pane. Never inspect processes during render.
- **Mapping:** a config table matches the folder to a label and colors; a `default = true` entry covers `claude` with no `CLAUDE_CONFIG_DIR`.
- **Token:** a new row token, `agent_profile`, usable in both `ui.sidebar.spaces.rows` and `ui.sidebar.agents.rows`. It draws right-aligned on its row as a pill in the profile's colors. Shorten the name, never the badge.
- **Where it shows:** space cards, agent group headers, ungrouped agent cards, the panel header and toasts.
- A space with no Claude agent shows no badge. A space running both configs shows both badges if they fit, otherwise the first plus `+1`.
- Lee's target rows for both spaces and agents: `[["state_icon", "workspace", "agent_profile"]]`.

## Config reference

Everything new that a user can set, with defaults. Update herdr's config reference docs to match.

| Key | Values | Default | Effect |
| --- | --- | --- | --- |
| `ui.style` | `"classic"`, `"app"` | `"classic"` | Turns the whole polished shell on |
| `ui.corners` | `"auto"`, `"rounded"`, `"square"` | `"auto"` | Rounded corners when the terminal can show images |
| `ui.pointer_shapes` | `"auto"`, `"on"`, `"off"` | `"auto"` | Hand pointer over clickable things |
| `ui.dim_unfocused` | 0.0 to 1.0 | 0.2 (proposed) | How much unfocused panes are dimmed |
| `[[ui.sidebar.agent_profiles]]` | `label`, `config_dir`, `default`, `fg`, `bg` | none | Claude config labels |
| `agent_profile` row token | in `ui.sidebar.spaces.rows` and `ui.sidebar.agents.rows` | not in default rows | Draws the config badge |
| `toggle_sidebar_spaces`, `toggle_sidebar_agents` | bindable actions | unbound | Show or hide each sidebar section |
| `theme.custom.<token>` | any token from Colors and theme tokens | derived per theme | Overrides a color |

Lee's config:

```toml
[ui]
style = "app"

[ui.sidebar.spaces]
rows = [["state_icon", "workspace", "agent_profile"]]

[ui.sidebar.agents]
rows = [["state_icon", "workspace", "agent_profile"]]

[[ui.sidebar.agent_profiles]]
label = "pco"
config_dir = "<the folder pco-claude sets>"
fg = "#9fc3fb"
bg = "#2c3d58"

[[ui.sidebar.agent_profiles]]
label = "claude"
default = true
fg = "#f6b891"
bg = "#4a3a36"
```

## Ghostty settings

These live in Lee's Ghostty config, not in herdr, and finish the look.

```
adjust-cell-height = 15%
window-padding-balance = true
window-padding-color = extend
```

- Ghostty's `background` must equal `pane_bg` (#292c33), so app content and the panel share one color.
- Keep native macOS tabs in the title bar; herdr's design assumes they stay.

## Accessibility and performance

The polished shell must stay readable, fully usable from the keyboard, and as fast as classic.

- **Contrast:** every text color reaches 4.5:1 on its surface, checked by a unit test for every built-in theme.
- **Not color alone:** status always shows a symbol, and agent rows also show a word.
- **Keyboard parity:** everything clickable also has an action or keybinding.
- **Render cost:** no new allocation per cell per frame. Generate images once and reuse them from the cache. Hover redraws only when the hovered target changes. No process inspection in render.
- **Graphics traffic:** send images only when they change, and remove them when their shape goes away.
- **Resize:** recompute layout on every resize, and resend images when the cell size changes.
- **Benchmark:** run `just bench-render-scale` before and after the phases that touch render loops, and report the difference.

## Delivery phases

Eight phases, each one buildable, tested and runnable on its own. Every phase ends with `just test` and `just lint` passing and a short summary Lee can try in Ghostty.

1. **Foundations:** the `ui.style` switch, new tokens, the shape module (square mode only), and the window regions: chrome, panel with caps, status bar row. Done when classic output is unchanged and buffer tests cover the region geometry at three window sizes.
2. **Sidebar:** chips, section headings, space cards, agent cards, bottom pills (square mode). Done when tests cover card geometry and states, click targets including cap rows, scroll metrics, all four section visibility combinations, and chip state surviving a restart.
3. **Panel header and padding:** title, branch, badge slot, tabs as a segmented control, icon buttons, hairline, 2-column content padding with the PTY resized to match. Done when tests cover header layout, tab overflow and the reported pane size.
4. **Rounded mode:** corner images, pill ends, shadows, per-client capability check, image cache, `ui.corners`. Done when tests cover graphics on and off, cache keys, and resending after a theme reload.
5. **Floating layers:** toast, menu popover, context menus, scrim. Done when tests cover popover placement, keyboard navigation, scrim recoloring and toast stacking.
6. **Status bar:** mode pill, hints from keybinds, counts. Done when tests cover each mode and hidden zero counts.
7. **Hover and pointer:** hover states on every target and OSC 22. Done when tests show hover changes on enter and leave, and OSC 22 is sent only on change, only when enabled, and reset on detach.
8. **Claude config labels:** environment detection, profile table, `agent_profile` token, badges everywhere. Done when tests cover environment parsing on Linux and macOS, profile matching, and badge truncation.

## Open questions

Confirm these with Lee before building the phase they affect.

- [x] Splits: one panel per pane with gutters (proposed), or one panel per tab with thin dividers? (Phase 3) **Answer:** one panel per pane with gutters.
- [x] Dim unfocused panes by default at 20%, or off by default? (Phase 3) **Answer:** off by default.
- [x] Grouped child spaces (today's `├─` tree) as cards: indent child cards 2 columns and drop the tree lines? (Phase 2) **Answer:** yes, indented cards.
- [x] Light themes: derive tokens now, or make app style dark-only at first? (Phase 1) **Answer:** dark-only at first.
- [x] Toast timing: reuse the existing notification timeout, or keep "needs you" toasts until acted on? (Phase 5) **Answer:** a setting, `ui.toast.herdr.keep_needs_attention`, default `false` (use the timeout).

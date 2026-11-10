# Polished shell: progress

Tracks the eight delivery phases in [spec.md](spec.md). Each phase is planned first, approved by Lee, then built on the `polished-shell` branch. Work stays in this fork; nothing goes to herdrdev/herdr.

## Checklist

- [x] **1. Foundations:** `ui.style` switch, app tokens, shape module, window regions (`2942a4f2`)
- [x] **2. Sidebar:** chips, section headings, space and agent cards, bottom pills (`b4f9f830`)
- [x] **3. Panel header and padding:** header, tabs as a segmented control, icon buttons, content padding, split gutters (`f494bebf`)
- [x] **4. Rounded mode:** corner images, pill ends, shadows, image cache, `ui.corners` (`bb6c5aba`)
- [x] **5. Floating layers:** menu popover, context menus, scrim, floating overlays, stacked toasts (`d79b26a8`, `a064726d`)
- [ ] **6. Status bar:** mode pill, hints from keybinds, counts
- [ ] **7. Hover and pointer:** hover states on every target, OSC 22 pointer shapes
- [ ] **8. Claude config labels:** `CLAUDE_CONFIG_DIR` detection, profile table, `agent_profile` token, badges

## Decisions

Answers to the spec's open questions, plus choices made along the way.

- Splits get one panel per pane, with dark gutters between them.
- Dimming unfocused panes is off by default.
- Grouped child spaces are cards indented 2 columns, with no tree lines.
- App style is dark-only for now.
- The sidebar is 36 columns wide.
- Toast timing is a setting: `ui.toast.herdr.keep_needs_attention`, default `false`. When false, toasts close after the existing timeouts (8s needs attention, 5s finished, 3s other). When true, "needs attention" toasts stay until you click `open` or `dismiss`, or the agent stops waiting. It applies to both styles.

## Assumptions to confirm

From Phase 5:

- The command palette (navigator) opens without the scrim, as in classic.
- The confirm-close dialog loses its red border in app style, since floating overlays have no border.
- Toast titles keep herdr's wording ("pi needs attention") rather than the spec's "× check-ins needs you".
- Toasts with no body keep an empty body row.
- The open-notification keybind acts on the oldest toast.
- Images that programs draw inside panes are not dimmed by the scrim.

## Next: Phase 6 plan (awaiting OK)

- Replace the mode bar with a status bar on the last row, on `chrome_bg`, in app style only.
- Left: the mode as a pill (`TERMINAL`, `PREFIX`, `NAVIGATE`, `RESIZE`), then key hints built from the configured keybinds in the compact `⌃B q` style. In prefix mode, show what the mode bar shows today.
- Right: counts across all spaces (`× needs you`, `◐ working`, `✓ done`, `○ idle`), zero counts hidden, then the session name.
- Tests: each mode, hidden zero counts, hints following keybind changes, classic unchanged.

## Testing notes

- Build with Zig 0.16.0: `export ZIG=$HOME/.local/share/zig-0.16.0/zig`.
- `just test` stops at the first failure. For a full picture, run `cargo nextest run --locked --no-fail-fast --retries 2`.
- Three tests fail on Lee's Mac on unmodified upstream too: `reload_aborts_an_in_flight_command_task_and_its_descendants`, `federated_client_starts_without_local_and_survives_its_restart`, `ssh_check_message_is_visible_while_authentication_waits`. Worktree and machine_setup tests are flaky under load.
- Benchmark render changes with `just bench-render-scale`.

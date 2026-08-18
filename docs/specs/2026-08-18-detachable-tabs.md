# Detachable tabs across windows

Status: draft for review. No implementation until approved.

## Goal

Make the chat area and the right panel both tab strips whose tabs can be
dragged to reorder, torn off into a new native window, and dragged between open
windows — with smooth motion throughout. Users work across several sessions and
surfaces (terminal, diff, file, browser) laid out over multiple windows the way
VS Code / Zed / a browser let them.

## Approach

**Shared `Workspace` model + thin pane windows** (chosen). Extract the state
that is conceptually app-wide out of the single `Waku` entity into one shared
`Workspace` model. Each OS window becomes a light **pane view** that renders a
tab strip plus the active tab's body and holds only window-local presentation
state. Tabs are owned by the `Workspace`; a window references the tabs assigned
to it. Moving a tab between windows is a reassignment in the shared model, not a
transfer of entity ownership.

Rejected: **one `Waku` per window sharing the daemon.** Smaller upfront refactor,
but every cross-window move becomes ad-hoc state transfer between peer entities,
and "which window owns session X" has no single source of truth. The sync debt
shows up exactly where this feature lives (P3).

This mirrors Zed's `Workspace` / `Pane` / `Item` split; use Zed as the reference
for the GPUI idioms (drag, pane, window) at the pinned `egoist/zed` revision.

## What a "tab" is

One tab = one **Item**, an enum over the two existing content families:

- `Item::Session(Uuid)` — a chat session (today: `state.sessions` + the single
  `selected_session`). Chat becomes multi-tab; "selected session" becomes
  "active tab of the chat-hosting pane".
- `Item::Surface(RightPanelSurface)` — the right panel's existing surfaces
  (`Browser`, `Terminal`, `BackgroundWork`, `Files`, `Diff`, `File`). These are
  already a `Vec` + active index with reuse logic; that logic moves onto the pane
  unchanged.

Both families already exist and are keyed by value; unifying them under `Item`
is the smallest model that supports one drag/tab code path for both.

## Data model migration (the core change)

Today `Waku` owns: `state.sessions`, `selected_session`, `right_panel_surfaces`
+ `right_panel_active_surface`, `daemon`, `composer`, and all UI state, and it
renders the one window via `WakuPane`.

Target:

- `Workspace` (shared, app-global via an `Entity<Workspace>` or a global):
  owns `daemon`, `sessions`, per-session runtime, persistence, and the set of
  `Pane`s and `Window`s. Single source of truth for where every `Item` lives.
- `Pane` (per tab strip, window-local): ordered `Vec<Item>` + `active: usize` +
  drag/scroll state. A window has one or more panes (start with one; splits are
  out of scope, see below).
- `PaneWindow` (per OS window): the root view. Renders its pane(s), the tab
  strip, window chrome. Holds only window-local presentation (focus, transient
  drag proxy).

Migration is incremental: introduce `Workspace` behind the current single window
first (Waku's fields move to it, `WakuPane` reads from it), so P1 lands without
any second window existing yet.

## Scope

In:
- Tab strips for chat and right panel, unified drag/tab code.
- Drag-to-reorder within a strip (animated).
- Tear-off to a new window on release outside any strip.
- Drag a tab onto another window's strip.
- Smooth motion: reorder slide, tab lift on grab, window spawn.

Out (YAGNI unless asked):
- Split panes inside one window (side-by-side within a window).
- Persisting/restoring the multi-window layout across app restarts (P4 later).
- Merging windows / stacking, tab groups, pinned tabs.
- Windows on Linux/Windows drag parity beyond what GPUI gives for free (macOS
  first; the platform-specific bit is only the P3 proxy window).

## Phases

### P1 — Tabs + smooth reorder (one window)

- Introduce `Workspace`, `Pane`, `Item`; move the relevant `Waku` state onto
  them. One window, one chat pane, one right pane.
- Chat pane shows open sessions as tabs; clicking the sidebar opens/activates a
  tab instead of swapping the single selection.
- Right pane reuses its surface logic as pane tabs.
- Drag-to-reorder within a strip using GPUI `on_drag`/`on_drop`; reorder animates
  (neighbours slide, dragged tab follows cursor within the window).

Acceptance:
- Open 3 sessions → 3 chat tabs; switching is instant, no reload.
- Drag a tab left/right → order changes, others slide, drop lands where shown.
- Right-panel surfaces behave identically to today, now via `Pane`.

### P2 — Tear-off to a new window

- On drag release **outside** any tab strip's bounds, `open_window` a new
  `PaneWindow`, move that `Item` into its pane, position the window at the drop
  point.
- Closing the last tab of a non-primary window closes the window; the primary
  window persists empty.

Acceptance:
- Drag a chat tab out and release on empty desktop → new window with that
  session; the source strip loses it.
- The torn-off session keeps its live turn/stream (state lives in `Workspace`,
  not the window).

### P3 — Drag between existing windows (the hard part)

GPUI element drags are window-clipped, so a tab cannot natively float across the
desktop into another window. Two mechanisms:

- **P3a — floating proxy window (recommended):** on grab, spawn a borderless,
  transparent, click-through, always-on-top window rendering the tab preview; it
  follows the global cursor. On release, hit-test all `PaneWindow`s: over a strip
  → insert there (P3 move); over none → tear-off (reuse P2). All in GPUI; the
  only platform surface is the proxy window flags.
- **P3b — OS-native drag** (`NSDraggingSession` via `gpui_platform`): heavier,
  macOS-specific, and GPUI may not expose arbitrary in-process payloads. Fallback
  only if the proxy window proves unstable.

Acceptance:
- Drag a tab from window A onto window B's strip → it inserts at the drop index
  in B and leaves A.
- Release over no strip → tear-off (P2) instead.
- Proxy preview tracks the cursor smoothly across windows/displays.

## Motion

- Grab: tab lifts (scale ~1.02, shadow) — respects `cx.reduce_motion()`.
- Reorder: neighbours animate to new slots (`with_animation`), not instant jumps.
- Spawn: new window fades/scales in at the drop point where the platform allows.
- All decorative motion checks reduce-motion and degrades to instant.

## Risks / open questions

- **P3 proxy window** is the main unknown: click-through + always-on-top +
  global cursor tracking across displays on the pinned GPUI fork. Prototype this
  in isolation before committing P3's UI.
- **Shared state boundary:** confirm what must be `Workspace`-global vs
  window-local (focus, command palette, find bar are window-local). Getting this
  line right is most of P1's value.
- **Persistence:** layout restore is out of scope now; note the seam so P4 can
  add it without another migration.
- **Reduce-motion + accessibility:** every drag affordance needs a keyboard path
  (move-tab-to-next-window command) — required, not optional.

## Acceptance (overall)

Chat and right panel are tab strips; a tab can be reordered, torn into a new
window keeping its live state, and dragged onto another window's strip, with
motion that honors reduce-motion and a keyboard equivalent for every drag.

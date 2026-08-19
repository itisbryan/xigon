# orca tab-panes & drag-splits — learnings for Xigon

Study of `stablyai/orca` @ `73f7767e`. Question: how does orca do tab panes +
splitting, to improve Xigon's split-pane work (`src/app/split_pane.rs`,
`chat_tabs.rs`, `render.rs`).

Stack: Electron + React + `@dnd-kit` + Zustand (no tiling lib — custom).
Clone (study only): `/tmp/orca-study`.

## TL;DR

orca splits via a **binary layout tree** (`leaf | {split, direction, first,
second}` with a `root` + `activeLeafId` + `expandedLeafId`), offers **both a
menu split** (Split Right/Down + shortcuts) **and a drag split** (with a blue
insertion bar), decouples the split trigger with a **window event**, and — the
non-obvious part — **prunes orphaned leaves** and collapses single-child splits
when a pane's content dies. Xigon today is a single `split_session: Option<Uuid>`
(2 panes max, drag-to-transcript that already crashed once). The cheap, reliable
wins: a **menu split** and **drag polish** (insertion bar + restore-on-cancel).
The binary tree is the real upgrade for >2 panes.

## What they built (cited)

- **Binary layout tree** — `TabGroupLayoutNode = {type:'leaf'} | {type:'split',
  direction, first, second}` (`src/shared/tab-types.ts:6`). A layout is
  `{root, activeLeafId, expandedLeafId}` (`src/shared/runtime-types.ts:130,557`).
- **Two split entry points, one action:**
  - Menu: `TerminalTabSplitMenuSection` with Split Right / Split Down + keyboard
    shortcuts (`.../tab-bar/TerminalTabSplitMenuSection.tsx:22,60`). Naming:
    *right = `'vertical'`, down = `'horizontal'`* (split axis, not layout axis).
  - Drag: `useTabDragSplit` (`.../tab-group/useTabDragSplit.ts`) with `@dnd-kit`.
- **Decoupled trigger** — the menu dispatches a `window` CustomEvent
  (`requestActiveTerminalPaneSplit` → `.../request-active-terminal-pane-split.ts`);
  the pane layout listens in `use-terminal-pane-split-actions.ts`. Menu never
  touches the layout store.
- **Orphan recovery** — `prunePaneLayout(node, retainedLeafIds)` drops leaves
  whose content is gone and collapses a split when one child becomes null
  (`.../runtime/web-session-terminal-orphan-topology.ts:19-33`). There's a whole
  orphan-topology/recovery module.
- **Zoom a pane** — `expandedLeafId` temporarily maximizes one leaf
  (`runtime-types.ts`, pruned/preserved in orphan-topology `:91`).
- **Drag UX polish** (`useTabDragSplit.ts`):
  - **Insertion bar** — `tab-insertion.ts` computes before/after the hovered tab
    (`side: 'right' ? +1 : 0`) and renders a blue bar; **suppresses on drop-onto-
    self** (`tab-insertion.ts:6,34,64`).
  - **Restore active tab on cancel** — `preDragActivationSnapshotRef` +
    `restorePreDragActivation` so a drag that ends up elsewhere doesn't leave the
    active tab switched (`useTabDragSplit.ts:105,156`).
  - Custom `collisionDetection` (pointer first, `closestCenter` fallback) (`:66`).
- **Split inherits context** — new pane gets the source's cwd
  (`splitTerminalPaneWithInheritedCwd`, `use-terminal-pane-split-actions.ts:57`).

## Techniques → adopt in Xigon

Ranked by effort:payoff. Files are real Xigon paths.

### 1. Menu split with directions + shortcuts (LOW effort, HIGH payoff) — do first

- **What / why:** orca's primary, reliable split is a menu (Split Right/Down +
  keys), not drag. Xigon's only split is drag-a-tab-into-the-transcript, which
  already SIGABRT'd once (OS-drag) and is finicky. A menu split can't crash.
- **Adopt:** reuse the existing chat-tab right-click `context_menu` (already in
  `chat_tabs.rs`) — add "Split right" calling `open_split(session, on_left:false)`
  and "Split left" (`on_left:true`). You already have `open_split`; this is a
  menu item + wiring. Add `⌘\`-style keybindings later.
- **Cost:** trivial; a menu section.
- **Portability:** direct (concept). Their dnd/React code doesn't port; the
  menu-first UX does.

### 2. Drag polish: insertion bar + restore-on-cancel (LOW-MED, MED payoff)

- **What / why:** Xigon's drag has no "where will it land" indicator and can
  leave the wrong tab selected. orca renders a blue insertion bar and snapshots
  the pre-drag active tab, restoring it on cancel.
- **Adopt:** (a) the split-drop overlay in `render.rs` already frames the target
  half — keep that (it *is* their insertion-bar idea). (b) On drag start, snapshot
  `selected_session`; if the drop no-ops/cancels, restore it. Small guard in the
  chat-tab drag path.
- **Cost:** one snapshot field + restore on cancel.
- **Portability:** direct.

### 3. Binary layout tree for >2 panes (HIGH effort, HIGH payoff) — the real upgrade

- **What / why:** Xigon caps at 2 panes (`split_session: Option<Uuid>`). orca's
  `leaf | {split, direction, first, second}` gives arbitrary tiling with one small
  recursive type. This is spec item 3 ("more than 2 panes") done right.
- **Adopt:** replace `split_session`/`split_on_left`/`split_ratio` with a
  `PaneNode` enum (`Leaf(session)` | `Split{dir, ratio, a: Box<PaneNode>, b}`) on
  `Waku`, render it recursively, and drop on an edge zone to insert a split.
  Reuse the existing per-leaf list/selection/markdown store keyed by leaf id.
- **Cost:** the render + drop routing become recursive; a real refactor. Do it
  only when two panes proves too few.
- **Portability:** the data model + recursion port; nothing else.

### 4. Prune / orphan recovery (MED, MED — pairs with #3)

- **What / why:** once you have a tree, closing a leaf must collapse its parent
  split and re-home the active leaf. orca's `prunePaneLayout` + orphan-topology
  does exactly this; Xigon's current "session gone → `split_session = None`" is the
  2-pane special case of it.
- **Adopt with #3:** a `prune(node, live_ids) -> Option<PaneNode>` that drops dead
  leaves and unwraps single-child splits. One recursive fn.
- **Cost:** only meaningful alongside the tree.
- **Portability:** direct (it's an algorithm).

### 5. Split inherits context + zoom a pane (LOW each, nice-to-have)

- **Inherit:** a new split pane should inherit the source session's project/cwd
  (orca inherits cwd). Xigon's `open_split(session)` already carries the session,
  so this is mostly free.
- **Zoom:** `expandedLeafId` temporarily maximizes one pane (double-click the
  divider / a key). Add `expanded: Option<leafId>` when the tree lands.

## Sidebars (left + right)

- **App shell** (`app-shell/AppWorkspaceShell.tsx:4,5,115`): one `flex flex-row`
  of `<Sidebar>` (left) | center column | `<RightSidebar>`. Its own comment:
  "left sidebar + titlebar + workbench content + right sidebar."
- **One resize hook for every panel** — `useSidebarResize`
  (`hooks/useSidebarResize.ts`) drives the left sidebar, right sidebar, editor
  TOC, and diff file tree (6 call sites). Direction is a single param
  `deltaSign: 1 | -1` (left `+1` `sidebar/index.tsx:100`, right `-1`
  `right-sidebar/index.tsx:75`): `delta = (clientX - startX) * deltaSign`
  (`useSidebarResize.ts:47`), clamped by `clampSidebarResizeWidth(w, min, max)`.
- **Live width bypasses state** — during a drag the width is written straight to
  the DOM (`container.style.width` + a `--workspace-sidebar-live-width` CSS var)
  and only committed to the store on release; the code says sidebars
  "intentionally keep live drag width out of" the store to avoid per-frame
  re-renders (`useSidebarResize.ts:91-95`, `sidebar/index.tsx:74`).
- **Collapse = width 0, not unmount** — the collapsed sidebar stays mounted at
  `w-0 overflow-visible` so its resize handle/header stay reachable and scroll/
  state survive the toggle (`AppWorkspaceShell.tsx:131`), toggled from the
  titlebar (`RightSidebarToggle`).
- **Right sidebar = activity bar + routed tabs** — VS Code-style activity bar
  with configurable position (`activityBarPosition`), tab routing
  (`useRightSidebarTabRouting`), plugin-contributed tabs
  (`installedPluginTabKeys`), responsive max width from the window
  (`computeMaxRightSidebarPanelWidth`) — `right-sidebar/index.tsx`.
- **Left sidebar = virtualized list + toolbar** — `WorktreeList` (virtualized,
  scroll-anchored) + `SidebarToolbar` + a project drop-zone affordance
  (`useSidebarProjectDrop`) — `sidebar/index.tsx`.

### Adopt in Xigon

- **Validation, no change:** Xigon already has the right shape — one resize
  machinery (`PanelResizeTarget::{Sidebar,RightPanel,FileTree,Split}` in
  `render.rs`/`sessions.rs`) with per-target sign, and responsive max-width
  clamps (`RIGHT_PANEL_MAX_WIDTH.min(viewport - main_min - sidebar)`). orca's
  `deltaSign` is exactly Xigon's per-target `start_width ± delta`.
- **Resize perf (note, don't over-fix):** orca keeps live drag width OUT of
  state; Xigon's `resize_panel_mouse_move` sets `self.sidebar_width` +
  `cx.notify()` every move → GPUI re-renders each frame. GPUI can't mutate width
  without a re-render (width *is* the layout input), and Xigon already skips
  <0.5px deltas — keep that; only gate the notify harder if resize feels heavy
  under a long transcript. `ponytail:` the DOM-bypass trick doesn't port to GPUI.
- **Collapse = width 0 (LOW, minor):** Xigon unmounts on toggle (`sidebar_visible`
  bool). Keeping the pane at width 0 preserves scroll/state and animates cleaner
  — only worth it if the toggle drops state you care about.
- **Skip (YAGNI):** configurable activity-bar position + plugin tabs — orca is an
  extensible platform; Xigon's fixed right-panel surfaces don't need it.

## Honest caveats

- orca is React + `@dnd-kit` + Zustand; Xigon is GPUI/Rust. **Only the model and
  UX patterns port — none of the code.** Their drag lives in `@dnd-kit`; Xigon's
  in GPUI `on_drag`/`on_drop` (which already bit us cross-window — see the
  `perform_drag_operation` SIGABRT). That's the strongest argument for #1: a
  menu split sidesteps GPUI's drag sharp edges entirely.
- orca splits *terminals* in a tab group; Xigon splits *sessions*. The tree and
  recovery are identical in shape; the leaf payload differs.

## Start tomorrow

Item 1 (menu split) — add "Split left/right" to the chat-tab context menu in
`chat_tabs.rs`, calling the existing `open_split`. Reliable, uncrashable, and it
gives users a split path that doesn't depend on the fragile transcript-drop.

# Split panes — roadmap

In-app split panes (multiplexer-style), no detached windows. This is the plan
for extending the shipped first slice.

## Shipped

Drag a chat tab onto the transcript → that session opens in a live split pane on
the right (`src/app/split_pane.rs`): its own `ListState` / selection /
markdown-parse cache so it never touches the main transcript's singletons. **X**
on the pane header closes it. State on `Waku`: `split_session: Option<Uuid>`,
`split_rows`, `split_selection`, `split_markdown`. Layout in
`render.rs::transcript_pane_content` (two `flex_1 min_w_0` columns).

Ceilings today: one right-split, read-only (compose targets the main pane),
fixed 50/50, message-text only, mouse-only.

## Items

### 1. Compose into the split pane

- **What:** click a pane to focus it; the composer submits to the focused pane's
  session, not always the main one.
- **Why:** the split is only watchable now — this makes it workable.
- **Approach:** add `focused_pane: Pane` (enum `Main | Split`) on `Waku`. The
  composer's submit path resolves its target session from `focused_pane` instead
  of `selected_session` directly. Click on either pane sets `focused_pane`; show
  a focus ring on the active pane. The split's composer can reuse the existing
  composer entity retargeted, or (lazy first) keep one composer that routes to
  the focused pane.
- **Acceptance:** click the split pane, type, submit → the message and stream
  land in the split session; the main pane is unaffected. Focus is visible.

### 2. Resizable divider

- **What:** drag the border between the two panes to change the ratio.
- **Why:** 50/50 is rarely what you want.
- **Approach:** `split_ratio: f32` on `Waku` (default 0.5). Replace the two
  `flex_1` columns with `flex_basis` from the ratio; a thin draggable handle on
  the border updates `split_ratio` on `on_drag_move` (reuse the panel-resize
  pattern already in `PanelResizeDrag`). Clamp to a sane min per pane.
- **Acceptance:** drag the divider → panes resize live; ratio holds until changed.

### 3. More than two panes

- **What:** drop on the left / top / bottom edge (not just right) and allow more
  than one split — a real pane tree, tmux-grid style.
- **Why:** two panes is the common case, but a grid is the multiplexer promise.
- **Approach:** replace `split_session: Option<Uuid>` with a layout tree
  (`Leaf(session)` | `Split{dir, ratio, a, b}`). Drop zones on each pane's four
  edge bands pick the split direction. Each leaf owns its own list/selection/
  markdown state (promote the three `split_*` fields into a per-leaf struct in a
  `HashMap` keyed by a pane id). This is the largest item; do it only when two
  panes proves too few.
- **Acceptance:** drop a tab on a pane's top edge → horizontal split within that
  pane; arbitrary nesting works; closing a leaf collapses its parent.

### 4. Persistence

- **What:** open tabs (`chat_tabs`) + the active split survive an app restart.
- **Why:** re-tearing your layout every launch is friction.
- **Approach:** serialize `chat_tabs` + the split layout into the existing
  `PersistedState` (`app.json`) with `#[serde(default)]`; restore on launch,
  dropping any session id that no longer exists. Pairs naturally with item 3's
  tree once that lands.
- **Acceptance:** quit with a split open → relaunch restores the tabs and the
  split; a deleted session is silently skipped.

### 5. Keyboard access for the tab strip

- **What:** the strip is mouse-only — make it keyboard operable.
- **Why:** accessibility requirement; every mouse control needs a keyboard path.
- **Approach:** `track_focus` + `tab_index` + `focus_visible` on each tab;
  arrow-left/right to move focus, `enter`/`space` to switch, a close key, and
  `shift-f10` to open any tab context menu. Add a "move focus between panes"
  action once item 1 lands. One coherent pass over the whole strip, not
  piecemeal.
- **Acceptance:** with no mouse, focus the strip, arrow between tabs, switch and
  close them, and move focus to the split pane.

### 6. Tool-activity fidelity in the split

- **What:** the split renders message text only; the main transcript shows tool
  calls, turn folding, reasoning.
- **Why:** parity — a split should look like the real transcript.
- **Approach:** the blocker is that `transcript_row`/`transcript_row_kinds` are
  bound to the main singletons. Either parameterize that row builder by a
  target (session + its own row-kind cache + list state), or accept the
  lightweight renderer as "preview" and only upgrade if users need full fidelity
  in a split. Largest-risk item; measure the need first.
- **Acceptance:** a streaming turn with tool calls renders in the split the same
  way it does in the main pane.

## Order

1 → 2 first (make the split usable + sizable). Then 4 (cheap, high comfort) and
5 (accessibility). 3 and 6 are the big ones — do them only when two-pane,
text-only splits demonstrably fall short.

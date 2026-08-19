# t3code design system — learnings for Xigon

Study of `pingdotgg/t3code` @ `f2d5fc91`. Question: what UI/design-system
choices to adopt in Xigon (GPUI/Rust, `src/theme.rs` + `src/ui`).

Clone (study only): `/tmp/t3code-study`. Every claim cites the clone.

## TL;DR

t3code's design system is a flat set of **58 semantic color roles** defined in
**OKLCH**, scoped per subsystem (toolbar/sidebar/message/terminal/code), shared
across desktop+web+mobile from one file, with **5 built-in themes** and a
**guided editor that generates the whole palette from 2 inputs** (canvas +
accent). Xigon's `theme.rs` is ~32 ad-hoc Hsla tokens hardcoded into two
`dark()`/`light()` fns. The three cheap wins for Xigon: a **single radius knob**,
**status triads**, and **message/transcript tokens** — all directly relevant to
the split-pane work. OKLCH + generated themes is the expensive, optional win.

## What they built (cited)

- **One canonical role list**, `THEME_COLOR_ROLES` — 58 roles, `packages/shared/src/themePalettes.ts:18`.
  Consumed identically by desktop, web, mobile (it's in `packages/shared`).
- **Subsystem-scoped tokens.** Not just `border`/`text`; each surface gets its
  own set: `toolbar*` (6), `sidebar*` (8: row hover/active/selected split out),
  `message*` (5), `terminal*` (6), `code*` (2). Same file, the roles array.
- **Layered surfaces**: `canvas → surface → surfaceRaised → surfaceOverlay`
  (4 explicit elevation levels) — the roles array.
- **Status triads**: every status is `{base, foreground, surface}` —
  `error/errorForeground/errorSurface`, same for `warning`, `update`
  (`T3_CHAT_THEME`, `packages/shared/src/themePalettes.ts` ~L110-135).
- **OKLCH everywhere** — 570 `oklch(L C H)` literals in `themePalettes.ts`.
  Perceptually uniform, so light↔dark and tint generation are lawful math, not
  eyeballing.
- **Single radius knob → derived scale**: `--radius: 0.625rem` (`apps/web/src/index.css:1017`);
  `--radius-sm..4xl = radius ± 2/4/8/12/16px` (`:193-199`). One value sets the
  app's whole rounding density.
- **Dedicated code font size**: `--font-size-code` (`apps/web/src/index.css:1541`).
- **5 themes** — T3_CHAT / GROVE / OCEAN / EMBER / IRIS, each `{colors, variants}`
  (per-appearance override) + `managed` flag "generated from the guided editor's
  canvas and accent roles" (`packages/shared/src/themePalettes.ts:91`). Pick 2
  colors → 58 derived.

## Techniques → adopt in Xigon

Ranked by effort:payoff. Files are real Xigon paths.

### 1. Single radius knob (LOW effort, HIGH payoff) — do first

- **What / why:** t3code drives all rounding from one `--radius`. Xigon
  sprinkles literal `px(6.0)`, `px(8.0)`, `px(10.0)`, `px(4.0)` across
  `chat_tabs.rs`, `split_pane.rs`, `render.rs`, `right_panel.rs` — the split-pane
  work alone added a dozen. Inconsistent and un-tunable.
- **Adopt:** add `radius: f32` (+ `radius_sm`, `radius_lg` derived `± 2/4`) to
  `src/theme.rs`; replace the literal `px(n)` in `.rounded(...)` calls with
  `px(theme.radius)`. One knob tunes the app's density.
- **Cost:** a mechanical find/replace of rounding literals; GPUI has no CSS
  `calc`, so derive in Rust.
- **Portability:** direct — it's a value, not web code.

### 2. Status triads (LOW effort, MED payoff)

- **What / why:** t3code status = `{base, foreground, surface}`. Xigon is
  inconsistent: `warning`, `success`, `danger`, `danger_soft` (only danger has a
  surface). Building status chips/badges means re-deriving tints ad hoc.
- **Adopt:** in `src/theme.rs` give `warning`/`success`/`danger` each a
  `_foreground` and `_surface` sibling (mirror the existing `danger_soft`).
- **Cost:** ~9 new token values × 2 themes.
- **Portability:** direct.

### 3. Message/transcript tokens (LOW effort, MED payoff) — relevant now

- **What / why:** t3code has `messageSurface/messageForeground/messageAction/
  messageActionHover`. Xigon's transcript reuses `theme.raised` for user bubbles
  (`src/app/split_pane.rs` split_row, and the main transcript) — ad hoc, and the
  split vs main panes can drift.
- **Adopt:** add `message_surface` (+ `message_action`, `message_action_hover`)
  to `theme.rs`; use it in both the main transcript and `split_row` so the two
  panes share one source of truth. Fixes the split/main styling drift directly.
- **Cost:** a few tokens; touch the two transcript renderers.
- **Portability:** direct.

### 4. Subsystem-scoped tokens (MED effort, MED payoff)

- **What / why:** t3code scopes tokens per surface so a subsystem restyles
  without leaking. Xigon reuses `overlay_strong` for tab-active, split-pill,
  right-panel-tab, close-button-hover — one change ripples everywhere.
- **Adopt opportunistically:** when a surface needs to diverge (e.g. the split
  pane / tab strip), give it its own token instead of borrowing `overlay_strong`.
  Don't pre-scope everything (YAGNI) — split when it actually needs to differ.
- **Cost:** more tokens; only worth it at real divergence points.
- **Portability:** direct.

### 5. OKLCH + generated themes (HIGH effort, HIGH payoff) — defer

- **What / why:** OKLCH makes light/dark and tint generation lawful; the guided
  editor derives 58 roles from canvas+accent (`themePalettes.ts:91`), enabling
  user themes. Xigon hardcodes two Hsla palettes — no user theming, tints
  eyeballed.
- **Adopt only if theming becomes a product goal:** store palettes in OKLCH
  (convert to GPUI `Hsla`/`Rgba` at load), and write a `derive_theme(canvas,
  accent) -> Theme`. Then Xigon gets multiple themes + a picker cheaply.
- **Cost:** OKLCH→Rgba conversion in Rust (no stdlib; ~1 small fn or a crate),
  a palette generator, and re-authoring `dark()`/`light()` as data. Real work.
- **Portability:** concept transfers; their TS/CSS pipeline does not.

## Honest caveats

- t3code is web (Tailwind + CSS vars); Xigon is GPUI. **Only the design
  decisions and values port — none of the code.** The 58-role list is a naming
  reference, not a target to match 1:1 (Xigon has no toolbar, different chrome).
- Their palette is tuned for OKLCH; pasting oklch values into Xigon needs a
  conversion step (item 5), so items 1–3 (structure, not raw colors) are the
  cheap wins, not "copy their pink."

## Start tomorrow

Item 1 (radius knob) in `src/theme.rs` — one field + a find/replace of
`.rounded(px(...))` literals. Highest consistency-per-line, and it cleans up the
radius guesses the split-pane work left behind.

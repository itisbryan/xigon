use super::*;

/// Root view of a torn-off window: a small tab strip over its own sessions plus
/// the active session's live transcript. It shares the one `Waku` entity but
/// keeps its own list/scroll/selection state, so it never touches the main
/// window's singletons.
///
/// ponytail: a session lives in exactly one place. `reconcile` drops any tab
/// that moved back to the main strip, so a drop that adds it there self-heals
/// here without a central owner registry. Detached↔detached moves and dragging
/// a detached tab back out (vs. its X button) are deferred to the pane model.
///
/// ponytail: transcript is message text only (markdown, live streaming). Tool
/// activity, turn folding, actions and text selection stay in the main
/// transcript; add them when a detached window needs full parity.
pub(super) struct DetachedSessionView {
    waku: WeakEntity<Waku>,
    tabs: Vec<Uuid>,
    active: Uuid,
    rows: ListState,
    selection: TranscriptSelection,
    markdown: RefCell<HashMap<Uuid, MarkdownView>>,
}

impl DetachedSessionView {
    fn new(waku: &Entity<Waku>, session_id: Uuid, cx: &mut Context<Self>) -> Self {
        // Re-render whenever shared app state changes, so streaming stays live.
        cx.observe(waku, |_, _, cx| cx.notify()).detach();
        // Closing this window returns every tab to the main strip (no delete,
        // no selection steal), completing the tear-off round-trip.
        cx.on_release(|this, cx| {
            if let Some(waku) = this.waku.upgrade() {
                let tabs = this.tabs.clone();
                waku.update(cx, |waku, cx| {
                    for id in tabs {
                        waku.readd_chat_tab(id, cx);
                    }
                });
            }
        })
        .detach();
        Self {
            waku: waku.downgrade(),
            tabs: vec![session_id],
            active: session_id,
            // Bottom alignment keeps the tail pinned as a turn streams in.
            rows: ListState::new(0, ListAlignment::Bottom, px(2048.0)),
            selection: TranscriptSelection::default(),
            markdown: RefCell::new(HashMap::new()),
        }
    }

    fn detached_row(
        &mut self,
        index: usize,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = Theme::current(cx);
        let palette = MarkdownPalette::from_theme(&theme);
        let active = self.active;
        let Some((id, role, text, streaming)) = self.waku.upgrade().and_then(|waku| {
            let waku = waku.read(cx);
            let session = waku.state.sessions.iter().find(|session| session.id == active)?;
            let message = session.messages.get(index)?;
            Some((
                message.id,
                message.role,
                message.visible_content().to_owned(),
                message.streaming,
            ))
        }) else {
            return div().into_any_element();
        };
        if text.trim().is_empty() {
            return div().into_any_element();
        }
        let is_user = matches!(role, MessageRole::User);
        let is_markdown = matches!(role, MessageRole::User | MessageRole::Assistant);
        let body = if is_markdown {
            let mut cache = self.markdown.borrow_mut();
            let view = cache.entry(id).or_insert_with(MarkdownView::new);
            view.set_text(&text, streaming);
            let metrics = if is_user {
                MarkdownMetrics::USER_MESSAGE
            } else {
                MarkdownMetrics::BODY
            };
            let ctx = MarkdownCtx::new(format!("dm-{id}"), &palette, metrics, self.selection.clone());
            div()
                .children(md::render::markdown(view, &ctx))
                .into_any_element()
        } else {
            div()
                .text_size(px(12.0))
                .text_color(theme.text_tertiary)
                .child(SharedString::from(text))
                .into_any_element()
        };
        div()
            .w_full()
            .px(px(20.0))
            .py(px(6.0))
            .child(
                div()
                    .when(is_user, |el| {
                        el.rounded(px(8.0)).bg(theme.raised).px(px(12.0)).py(px(8.0))
                    })
                    .child(body),
            )
            .into_any_element()
    }

    /// One tab chip in the strip: click to switch, X returns it to the main
    /// window's strip.
    fn detached_tab(
        &self,
        session_id: Uuid,
        title: SharedString,
        glyph: &'static str,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let active = self.active == session_id;
        div()
            .id(SharedString::from(format!("detached-tab-{session_id}")))
            .h(px(26.0))
            .min_w(px(96.0))
            .max_w(px(200.0))
            .px(px(8.0))
            .rounded(px(6.0))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(6.0))
            .cursor_default()
            .when(active, |el| el.bg(theme.overlay_strong))
            .when(!active, |el| el.hover(|el| el.bg(theme.overlay)))
            .child(icon(glyph, 13.0, theme.text_secondary))
            .child(
                div()
                    .min_w_0()
                    .flex_1()
                    .line_clamp(1)
                    .text_ellipsis()
                    .text_size(px(12.0))
                    .text_color(if active { theme.text } else { theme.text_secondary })
                    .child(title),
            )
            .child(
                div()
                    .id(SharedString::from(format!("detached-close-{session_id}")))
                    .w(px(16.0))
                    .h(px(16.0))
                    .rounded(px(4.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .hover(|el| el.bg(theme.overlay_strong))
                    .child(icon("icons/x.svg", 10.0, theme.text_tertiary))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.close_detached_tab(session_id, cx);
                    })),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.active = session_id;
                cx.notify();
            }))
            .into_any_element()
    }

    /// Session id of the currently shown tab (for a target window's label).
    pub(super) fn active(&self) -> Uuid {
        self.active
    }

    /// Take a session into this window's pane and drop it from the main strip.
    pub(super) fn accept_session(&mut self, incoming: Uuid, cx: &mut Context<Self>) {
        if !self.tabs.contains(&incoming) {
            self.tabs.push(incoming);
        }
        self.active = incoming;
        cx.notify();
        if let Some(waku) = self.waku.upgrade() {
            waku.update(cx, |waku, cx| waku.close_chat_tab(incoming, cx));
        }
    }

    /// Return a tab to the main strip and drop it from this window.
    fn close_detached_tab(&mut self, session_id: Uuid, cx: &mut Context<Self>) {
        self.tabs.retain(|id| *id != session_id);
        if self.active == session_id {
            if let Some(first) = self.tabs.first().copied() {
                self.active = first;
            }
        }
        if let Some(waku) = self.waku.upgrade() {
            waku.update(cx, |waku, cx| waku.readd_chat_tab(session_id, cx));
        }
        cx.notify();
    }
}

impl Render for DetachedSessionView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::current(cx);
        let drop_tint = theme.accent.opacity(0.06);
        // Pull the shared data this frame needs, then drop the borrow before
        // mutating our own tab list.
        let snapshot = self.waku.upgrade().map(|waku| {
            let waku = waku.read(cx);
            let main: Vec<Uuid> = waku.chat_tabs.clone();
            let infos: Vec<(Uuid, SharedString, &'static str)> = self
                .tabs
                .iter()
                .filter_map(|id| {
                    waku.state.sessions.iter().find(|s| s.id == *id).map(|s| {
                        (
                            *id,
                            SharedString::from(super::sidebar::localized_session_title(s)),
                            provider_icon(s.provider),
                        )
                    })
                })
                .collect();
            let active_count = waku
                .state
                .sessions
                .iter()
                .find(|s| s.id == self.active)
                .map_or(0, |s| s.messages.len());
            (main, infos, active_count)
        });
        let Some((main, infos, active_count)) = snapshot else {
            return div().size_full().bg(theme.canvas).into_any_element();
        };
        // A session lives in exactly one place: drop tabs that moved to the main
        // strip or no longer exist, and keep the active pointer valid.
        let live: Vec<Uuid> = infos.iter().map(|(id, _, _)| *id).collect();
        self.tabs
            .retain(|id| live.contains(id) && !main.contains(id));
        if !self.tabs.contains(&self.active) {
            if let Some(first) = self.tabs.first().copied() {
                self.active = first;
            }
        }
        if self.tabs.is_empty() {
            // Emptied by moves; the window is now inert until the user closes it.
            return div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .bg(theme.canvas)
                .child(div().text_color(theme.text_tertiary).child("—"))
                .into_any_element();
        }
        // Reconcile the virtual list length to the active session's messages.
        let current = self.rows.item_count();
        if active_count > current {
            self.rows.splice(current..current, active_count - current);
        } else if active_count < current {
            self.rows.splice(active_count..current, 0);
        }
        let strip_tabs: Vec<AnyElement> = infos
            .into_iter()
            .filter(|(id, _, _)| self.tabs.contains(id))
            .map(|(id, title, glyph)| self.detached_tab(id, title, glyph, &theme, cx))
            .collect();
        let entity = cx.entity().downgrade();
        div()
            .id("detached-window")
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.canvas)
            .text_color(theme.text)
            // A tab dragged from another window lands here and joins this pane.
            .drag_over::<super::chat_tabs::ChatTabDrag>(move |style, _, _, _| style.bg(drop_tint))
            .on_drop(cx.listener(
                |this, drag: &super::chat_tabs::ChatTabDrag, _window, cx| {
                    this.accept_session(drag.session_id, cx);
                },
            ))
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .h(px(38.0))
                    .px(px(8.0))
                    .border_b_1()
                    .border_color(theme.border)
                    .children(strip_tabs),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .relative()
                    // Reset the private selection registry each frame so it
                    // cannot grow unbounded across renders.
                    .child(md::render::frame_reset(self.selection.clone()))
                    .child(
                        list(self.rows.clone(), move |index, window, cx| {
                            entity
                                .upgrade()
                                .map(|entity| {
                                    entity.update(cx, |this, cx| {
                                        this.detached_row(index, window, cx)
                                    })
                                })
                                .unwrap_or_else(|| div().into_any_element())
                        })
                        .size_full(),
                    ),
            )
            .into_any_element()
    }
}

impl Waku {
    /// Tear a chat tab out of the strip into its own native window. The window
    /// shares this `Waku` entity and renders the session live, then the tab
    /// leaves this window's strip.
    pub(super) fn tear_off_chat_tab(&mut self, session_id: Uuid, cx: &mut Context<Self>) {
        let Some(title) = self
            .state
            .sessions
            .iter()
            .find(|session| session.id == session_id)
            .map(super::sidebar::localized_session_title)
        else {
            return;
        };
        let waku = cx.entity();
        // open_window draws the new window synchronously; running that inside the
        // current event/draw (this is called from a drop or menu handler)
        // re-enters GPUI's App borrow and aborts (SIGABRT mid-drag). Defer it so
        // it runs after the event completes. ponytail: fixed placement; drop-point
        // placement needs the OS drag payload we can't use, so it opens at an offset.
        cx.defer(move |cx| {
            let view_waku = waku.clone();
            let bounds = gpui::Bounds {
                origin: gpui::point(px(140.0), px(140.0)),
                size: gpui::size(px(760.0), px(620.0)),
            };
            let opened = cx.open_window(
                gpui::WindowOptions {
                    titlebar: Some(gpui::TitlebarOptions {
                        title: Some(title.into()),
                        ..Default::default()
                    }),
                    window_bounds: Some(gpui::WindowBounds::Windowed(bounds)),
                    window_min_size: Some(gpui::size(px(420.0), px(320.0))),
                    ..Default::default()
                },
                move |_window, cx| {
                    cx.new(|cx| DetachedSessionView::new(&view_waku, session_id, cx))
                },
            );
            if let Ok(handle) = opened {
                let _ = waku.update(cx, |waku, cx| {
                    if let Ok(view) = handle.entity(cx) {
                        waku.detached_views.retain(|v| v.upgrade().is_some());
                        waku.detached_views.push(view.downgrade());
                    }
                    waku.close_chat_tab(session_id, cx);
                });
            }
        });
    }

    /// Return a session to the chat strip if it still exists and is not already
    /// a tab. Does not change the selection, so reclaiming a detached window is
    /// non-disruptive.
    pub(super) fn readd_chat_tab(&mut self, session_id: Uuid, cx: &mut Context<Self>) {
        if self.state.sessions.iter().any(|session| session.id == session_id)
            && !self.chat_tabs.contains(&session_id)
        {
            self.chat_tabs.push(session_id);
            cx.notify();
        }
    }
}

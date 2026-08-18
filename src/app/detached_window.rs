use super::*;

/// Root view of a torn-off session window. It shares the one `Waku` entity and
/// renders the session's live transcript with its own list/scroll/selection
/// state, so it never touches the main window's singletons.
///
/// ponytail: message text only (markdown, live streaming). Tool activity, turn
/// folding, message actions and text selection are the main transcript's; add
/// them when a detached window needs full parity.
pub(super) struct DetachedSessionView {
    waku: WeakEntity<Waku>,
    session_id: Uuid,
    rows: ListState,
    selection: TranscriptSelection,
    markdown: RefCell<HashMap<Uuid, MarkdownView>>,
}

impl DetachedSessionView {
    fn new(waku: &Entity<Waku>, session_id: Uuid, cx: &mut Context<Self>) -> Self {
        // Re-render whenever shared app state changes, so streaming stays live.
        cx.observe(waku, |_, _, cx| cx.notify()).detach();
        // Closing this window returns the session to the main strip (no delete,
        // no selection steal), completing the tear-off round-trip.
        cx.on_release(|this, cx| {
            let session_id = this.session_id;
            if let Some(waku) = this.waku.upgrade() {
                waku.update(cx, |waku, cx| waku.readd_chat_tab(session_id, cx));
            }
        })
        .detach();
        Self {
            waku: waku.downgrade(),
            session_id,
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
        let Some((id, role, text, streaming)) = self.waku.upgrade().and_then(|waku| {
            let waku = waku.read(cx);
            let session = waku
                .state
                .sessions
                .iter()
                .find(|session| session.id == self.session_id)?;
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
}

impl Render for DetachedSessionView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::current(cx);
        let header = self.waku.upgrade().and_then(|waku| {
            let waku = waku.read(cx);
            waku.state
                .sessions
                .iter()
                .find(|session| session.id == self.session_id)
                .map(|session| {
                    (
                        super::sidebar::localized_session_title(session),
                        session.provider,
                        session.model.clone().unwrap_or_default(),
                        session.messages.len(),
                    )
                })
        });
        let Some((title, provider, model, count)) = header else {
            return div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .bg(theme.canvas)
                .child(div().text_color(theme.text_tertiary).child("—"))
                .into_any_element();
        };
        // Reconcile the virtual list length to the live message count.
        let current = self.rows.item_count();
        if count > current {
            self.rows.splice(current..current, count - current);
        } else if count < current {
            self.rows.splice(count..current, 0);
        }
        let entity = cx.entity().downgrade();
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.canvas)
            .text_color(theme.text)
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .h(px(38.0))
                    .px(px(16.0))
                    .border_b_1()
                    .border_color(theme.border)
                    .child(icon(provider_icon(provider), 15.0, theme.text_secondary))
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .line_clamp(1)
                            .text_ellipsis()
                            .text_size(px(13.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.text)
                            .child(SharedString::from(title)),
                    )
                    .when(!model.is_empty(), |el| {
                        el.child(
                            div()
                                .flex_none()
                                .text_size(px(11.5))
                                .text_color(theme.text_tertiary)
                                .child(SharedString::from(model)),
                        )
                    }),
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
        // ponytail: fixed placement; drop-point / desktop-drop needs P3's
        // external drag payload, so v1 opens at a fixed offset.
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
            move |_window, cx| cx.new(|cx| DetachedSessionView::new(&waku, session_id, cx)),
        );
        if opened.is_ok() {
            self.close_chat_tab(session_id, cx);
        }
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

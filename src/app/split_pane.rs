use super::*;

// A single secondary pane beside the transcript (multiplexer-style): drag a tab
// onto the transcript to show that session live next to the current one. It uses
// its own list/scroll/selection state so it never touches the main transcript's
// singletons.
//
// ponytail: one right-split, read-only (compose still targets the main session),
// message text only. Add: 4-edge splits + a pane tree, focus + compose-to-pane,
// resizable splitter, tool activity, when a real tiling workflow needs them.
impl Waku {
    pub(super) fn open_split(&mut self, session_id: Uuid, on_left: bool, cx: &mut Context<Self>) {
        let changed = self.split_session != Some(session_id) || self.split_on_left != on_left;
        self.split_on_left = on_left;
        if self.split_session != Some(session_id) {
            self.split_session = Some(session_id);
            self.split_markdown.borrow_mut().clear();
        }
        if changed {
            cx.notify();
        }
    }

    pub(super) fn close_split(&mut self, cx: &mut Context<Self>) {
        self.split_session = None;
        cx.notify();
    }

    /// Make the split session the active one (so the composer targets it) and
    /// push the previously active session into the split pane. Reuses
    /// `select_session`, so the composer/footer/draft all follow correctly with
    /// no retargeting. ponytail: this swaps the two panes' sessions rather than
    /// focusing in place; add in-place focus if keeping positions matters.
    pub(super) fn promote_split(&mut self, cx: &mut Context<Self>) {
        let Some(split) = self.split_session else {
            return;
        };
        // activate_session swaps the panes when the split's session becomes
        // selected, so selecting it is the whole promote.
        self.select_session(split, cx);
    }

    /// The secondary pane, or `None` when nothing is split.
    pub(super) fn render_split_pane(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let session_id = self.split_session?;
        let theme = Theme::current(cx);
        let info = self
            .state
            .sessions
            .iter()
            .find(|session| session.id == session_id)
            .map(|session| {
                (
                    SharedString::from(super::sidebar::localized_session_title(session)),
                    provider_icon(session.provider),
                    session.messages.len(),
                )
            });
        let Some((title, glyph, count)) = info else {
            // Session gone; collapse the split on the next frame.
            self.split_session = None;
            return None;
        };
        let current = self.split_rows.item_count();
        if count > current {
            self.split_rows.splice(current..current, count - current);
        } else if count < current {
            self.split_rows.splice(count..current, 0);
        }
        let entity = cx.entity().downgrade();
        Some(
            div()
                .flex()
                .flex_col()
                .size_full()
                .child(
                    div()
                        // Click the header to make this pane the active session
                        // (its transcript + composer move to the main pane).
                        .id("split-header")
                        .cursor_default()
                        .hover(|el| el.bg(theme.overlay))
                        .on_click(cx.listener(|this, _, _, cx| this.promote_split(cx)))
                        .flex_none()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .h(px(38.0))
                        .px(px(12.0))
                        .border_b_1()
                        .border_color(theme.border)
                        .child(icon(glyph, 15.0, theme.text_secondary))
                        .child(
                            div()
                                .min_w_0()
                                .flex_1()
                                .line_clamp(1)
                                .text_ellipsis()
                                .text_size(px(13.0))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme.text)
                                .child(title),
                        )
                        .child(
                            div()
                                .id("split-close")
                                .w(px(20.0))
                                .h(px(20.0))
                                .rounded(px(5.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_default()
                                .hover(|el| el.bg(theme.overlay))
                                .child(icon("icons/x.svg", 11.0, theme.text_tertiary))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    cx.stop_propagation();
                                    this.close_split(cx);
                                })),
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .w_full()
                        .relative()
                        // Reset the private selection registry each frame so it
                        // cannot grow unbounded across renders.
                        .child(md::render::frame_reset(self.split_selection.clone()))
                        .child(
                            list(self.split_rows.clone(), move |index, window, cx| {
                                entity
                                    .upgrade()
                                    .map(|entity| {
                                        entity.update(cx, |this, cx| {
                                            this.split_row(index, window, cx)
                                        })
                                    })
                                    .unwrap_or_else(|| div().into_any_element())
                            })
                            .size_full(),
                        ),
                )
                .into_any_element(),
        )
    }

    fn split_row(&mut self, index: usize, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::current(cx);
        let palette = MarkdownPalette::from_theme(&theme);
        let Some(active) = self.split_session else {
            return div().into_any_element();
        };
        let Some((id, role, text, streaming)) = self
            .state
            .sessions
            .iter()
            .find(|session| session.id == active)
            .and_then(|session| session.messages.get(index))
            .map(|message| {
                (
                    message.id,
                    message.role,
                    message.visible_content().to_owned(),
                    message.streaming,
                )
            })
        else {
            return div().into_any_element();
        };
        if text.trim().is_empty() {
            return div().into_any_element();
        }
        let is_user = matches!(role, MessageRole::User);
        let is_markdown = matches!(role, MessageRole::User | MessageRole::Assistant);
        let body = if is_markdown {
            let mut cache = self.split_markdown.borrow_mut();
            let view = cache.entry(id).or_insert_with(MarkdownView::new);
            view.set_text(&text, streaming);
            let metrics = if is_user {
                MarkdownMetrics::USER_MESSAGE
            } else {
                MarkdownMetrics::BODY
            };
            let ctx = MarkdownCtx::new(format!("sp-{id}"), &palette, metrics, self.split_selection.clone());
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

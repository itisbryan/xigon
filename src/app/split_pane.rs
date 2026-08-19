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

    /// The secondary pane, or `None` when nothing is split.
    pub(super) fn render_split_pane(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let session_id = self.split_session?;
        let theme = Theme::current(cx);
        let Some(count) = self
            .state
            .sessions
            .iter()
            .find(|session| session.id == session_id)
            .map(|session| session.messages.len())
        else {
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
                .relative()
                .flex()
                .flex_col()
                .size_full()
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
                .child(
                    // The strip already labels the sessions, so the split needs
                    // no header bar — just a small close affordance.
                    div()
                        .id("split-close")
                        .absolute()
                        .top(px(6.0))
                        .right(px(10.0))
                        .w(px(22.0))
                        .h(px(22.0))
                        .rounded(px(6.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_default()
                        .bg(theme.overlay)
                        .hover(|el| el.bg(theme.overlay_strong))
                        .child(icon("icons/x.svg", 11.0, theme.text_tertiary))
                        .on_click(cx.listener(|this, _, _, cx| this.close_split(cx))),
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

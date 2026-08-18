use super::*;

/// A chat tab being dragged to reorder within the strip.
struct ChatTabDrag {
    session_id: Uuid,
}

/// Floating preview rendered under the cursor while a chat tab is dragged.
struct ChatTabDragPreview {
    label: SharedString,
    glyph: &'static str,
}

impl Render for ChatTabDragPreview {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::current(cx);
        div()
            .flex()
            .items_center()
            .gap(px(6.0))
            .h(px(28.0))
            .px(px(10.0))
            .rounded(px(6.0))
            .border_1()
            .border_color(theme.border)
            .bg(theme.overlay_strong)
            .text_size(px(12.0))
            .text_color(theme.text)
            .child(icon(self.glyph, 13.0, theme.text_secondary))
            .child(self.label.clone())
    }
}

impl Waku {
    /// The chat tab strip above the transcript. One tab per open session:
    /// click to switch, drag to reorder, X to close (the session stays).
    /// Hidden until a second session is open — a lone session needs no tabs.
    pub(super) fn render_chat_tab_strip(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.chat_tabs.len() < 2 {
            return None;
        }
        let theme = Theme::current(cx);
        let hover_bg = theme.accent.opacity(0.16);
        let selected = self.state.selected_session;
        let mut strip = div()
            .id("chat-tabs")
            .w_full()
            .h(px(36.0))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(4.0))
            .px(px(8.0))
            .border_b_1()
            .border_color(theme.border)
            .overflow_x_scroll()
            .track_scroll(&self.chat_tabs_scroll_handle);
        for (index, session_id) in self.chat_tabs.iter().copied().enumerate() {
            let Some(session) = self.state.sessions.iter().find(|s| s.id == session_id) else {
                continue;
            };
            let active = selected == Some(session_id);
            let label = SharedString::from(super::sidebar::localized_session_title(session));
            let glyph = provider_icon(session.provider);
            let drag_label = label.clone();
            strip = strip.child(
                div()
                    .id(SharedString::from(format!("chat-tab-{index}")))
                    .h(px(28.0))
                    .min_w(px(110.0))
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
                            .child(label),
                    )
                    .child(
                        div()
                            .id(SharedString::from(format!("close-chat-tab-{index}")))
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
                                this.close_chat_tab(session_id, cx);
                            })),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.select_session(session_id, cx);
                    }))
                    .on_drag(ChatTabDrag { session_id }, move |_, _, _, cx| {
                        cx.new(|_| ChatTabDragPreview {
                            label: drag_label.clone(),
                            glyph,
                        })
                    })
                    .drag_over::<ChatTabDrag>(move |style, _, _, _| style.bg(hover_bg))
                    .on_drop(cx.listener(move |this, drag: &ChatTabDrag, _, cx| {
                        this.reorder_chat_tab(drag.session_id, index, cx);
                    })),
            );
        }
        Some(strip.into_any_element())
    }

    fn reorder_chat_tab(&mut self, session_id: Uuid, to_index: usize, cx: &mut Context<Self>) {
        let Some(from) = self.chat_tabs.iter().position(|id| *id == session_id) else {
            return;
        };
        if from == to_index {
            return;
        }
        let id = self.chat_tabs.remove(from);
        let to = to_index.min(self.chat_tabs.len());
        self.chat_tabs.insert(to, id);
        cx.notify();
    }

    /// Close a chat tab without deleting the session; activate a neighbour.
    pub(super) fn close_chat_tab(&mut self, session_id: Uuid, cx: &mut Context<Self>) {
        let Some(index) = self.chat_tabs.iter().position(|id| *id == session_id) else {
            return;
        };
        self.chat_tabs.remove(index);
        if self.state.selected_session != Some(session_id) {
            cx.notify();
            return;
        }
        match self
            .chat_tabs
            .get(index)
            .or_else(|| self.chat_tabs.last())
            .copied()
        {
            Some(next) => self.select_session(next, cx),
            None => {
                self.state.selected_session = None;
                cx.notify();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    // Reorder is index arithmetic with a shift when dragging left→right; guard it.
    fn reorder(tabs: &mut Vec<u8>, from_val: u8, to_index: usize) {
        let Some(from) = tabs.iter().position(|v| *v == from_val) else {
            return;
        };
        if from == to_index {
            return;
        }
        let v = tabs.remove(from);
        let to = to_index.min(tabs.len());
        tabs.insert(to, v);
    }

    #[test]
    fn reorder_moves_tab_to_target_slot() {
        let mut tabs = vec![1u8, 2, 3, 4];
        reorder(&mut tabs, 1, 2); // drag 1 rightwards onto index 2
        assert_eq!(tabs, vec![2, 3, 1, 4]);

        let mut tabs = vec![1u8, 2, 3, 4];
        reorder(&mut tabs, 4, 0); // drag 4 to the front
        assert_eq!(tabs, vec![4, 1, 2, 3]);

        let mut tabs = vec![1u8, 2, 3];
        reorder(&mut tabs, 2, 1); // no-op onto own slot
        assert_eq!(tabs, vec![1, 2, 3]);
    }
}

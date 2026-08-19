use super::*;

/// A chat tab being dragged to reorder within the strip, or onto the
/// transcript to open a split pane.
pub(super) struct ChatTabDrag {
    pub(super) session_id: Uuid,
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
    /// Visible whenever a session is open, so opening more from the sidebar
    /// visibly grows the strip; hidden only on the empty new-task screen.
    pub(super) fn render_chat_tab_strip(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.chat_tabs.is_empty() {
            return None;
        }
        let theme = Theme::current(cx);
        let hover_bg = theme.accent.opacity(0.16);
        let selected = self.focused_session_id();
        let main_id = self.state.selected_session;
        let split_id = self.split_session;
        let has_split = split_id.is_some();
        let is_on = |id: Uuid| main_id == Some(id) || split_id == Some(id);
        let mut strip = div()
            .id("chat-tabs")
            .w_full()
            .h(px(36.0))
            .flex_none()
            .flex()
            .items_center()
            // Tabs space themselves via left margin so a split pair can sit
            // flush and read as one joined unit.
            .gap(px(0.0))
            .px(px(8.0))
            .border_b_1()
            .border_color(theme.border)
            .overflow_x_scroll()
            .track_scroll(&self.chat_tabs_scroll_handle);
        for (index, session_id) in self.chat_tabs.iter().copied().enumerate() {
            let Some(session) = self.state.sessions.iter().find(|s| s.id == session_id) else {
                continue;
            };
            // Both panes' tabs read as on-screen when split; the focused one
            // is strongest.
            let focused = selected == Some(session_id);
            let on_screen = is_on(session_id);
            // Join adjacent on-screen tabs (the split pair) into one pill.
            let next_on = self.chat_tabs.get(index + 1).copied().is_some_and(&is_on);
            let prev_on = index
                .checked_sub(1)
                .and_then(|i| self.chat_tabs.get(i))
                .copied()
                .is_some_and(&is_on);
            let join_left = on_screen && next_on;
            let join_right = on_screen && prev_on;
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
                    .ml(if index == 0 || join_right { px(0.0) } else { px(4.0) })
                    .when(!join_left && !join_right, |el| el.rounded(px(6.0)))
                    .when(join_left && !join_right, |el| {
                        el.rounded_tl(px(6.0)).rounded_bl(px(6.0))
                    })
                    .when(join_right && !join_left, |el| {
                        el.rounded_tr(px(6.0))
                            .rounded_br(px(6.0))
                            .border_l_1()
                            .border_color(theme.border)
                    })
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .cursor_default()
                    // Both split halves share one fill so the pill reads as a
                    // single unit; focus is shown by the accent label below.
                    .when(on_screen, |el| el.bg(theme.overlay_strong))
                    .when(!on_screen, |el| el.hover(|el| el.bg(theme.overlay)))
                    .child(icon(glyph, 13.0, theme.text_secondary))
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .line_clamp(1)
                            .text_ellipsis()
                            .text_size(px(12.0))
                            .text_color(if focused && has_split {
                                theme.accent
                            } else if on_screen {
                                theme.text
                            } else {
                                theme.text_secondary
                            })
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
                        // select_session focuses the split pane in place when the
                        // session is already there, else opens it in the main pane.
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
        if self.split_session == Some(session_id) {
            self.split_session = None;
            self.split_focused = false;
        }
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
    fn reorder(tabs: &mut Vec<u8>, val: u8, to_index: usize) {
        let Some(from) = tabs.iter().position(|v| *v == val) else {
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

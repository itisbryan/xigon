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
        let main_id = self.state.selected_session;
        let split_id = self.split_session;
        let is_on = |id: Uuid| main_id == Some(id) || split_id == Some(id);
        // Render the split pair adjacent so they always fuse into one pill:
        // keep the main tab in place and pull the split tab right next to it.
        let order: Vec<Uuid> = match (main_id, split_id) {
            (Some(main), Some(split)) if main != split => {
                let mut v = Vec::with_capacity(self.chat_tabs.len());
                for &id in &self.chat_tabs {
                    if id == split {
                        continue;
                    }
                    v.push(id);
                    if id == main {
                        v.push(split);
                    }
                }
                v
            }
            _ => self.chat_tabs.clone(),
        };
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
        let weak = cx.entity().downgrade();
        for (index, session_id) in order.iter().copied().enumerate() {
            let Some(session) = self.state.sessions.iter().find(|s| s.id == session_id) else {
                continue;
            };
            let on_screen = is_on(session_id);
            // Join adjacent on-screen tabs (the split pair) into one pill.
            let next_on = order.get(index + 1).copied().is_some_and(&is_on);
            let prev_on = index
                .checked_sub(1)
                .and_then(|i| order.get(i))
                .copied()
                .is_some_and(&is_on);
            let join_left = on_screen && next_on;
            let join_right = on_screen && prev_on;
            let label = SharedString::from(super::sidebar::localized_session_title(session));
            let glyph = provider_icon(session.provider);
            let drag_label = label.clone();
            let menu = self.menu_handle(format!("chat-tab-menu-{session_id}"), cx);
            let tab = div()
                    .id(SharedString::from(format!("chat-tab-{index}")))
                    .h(px(28.0))
                    .min_w(px(110.0))
                    .max_w(px(200.0))
                    .px(px(8.0))
                    .ml(if index == 0 || join_right { px(0.0) } else { px(4.0) })
                    .when(!join_left && !join_right, |el| el.rounded(px(6.0)))
                    // Joined pair reads as one segmented-control pill: outline the
                    // outer edges, divide the seam.
                    .when(join_left && !join_right, |el| {
                        el.rounded_tl(px(6.0))
                            .rounded_bl(px(6.0))
                            .border_t_1()
                            .border_b_1()
                            .border_l_1()
                            .border_color(theme.border_strong)
                    })
                    .when(join_right && !join_left, |el| {
                        el.rounded_tr(px(6.0))
                            .rounded_br(px(6.0))
                            .border_t_1()
                            .border_b_1()
                            .border_r_1()
                            .border_l_1()
                            .border_color(theme.border_strong)
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
                            .text_color(if on_screen { theme.text } else { theme.text_secondary })
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
                        this.reorder_chat_tab(drag.session_id, session_id, cx);
                    }));
            let is_split = split_id == Some(session_id);
            let menu_weak = weak.clone();
            strip = strip.child(context_menu(
                tab,
                SharedString::from(format!("chat-tab-menuwrap-{index}")),
                &menu,
                move |_| {
                    let w = menu_weak.clone();
                    let mut items = vec![
                        MenuItem::new(tr!("tabs.split_left"), {
                            let w = w.clone();
                            move |_, cx| {
                                let _ =
                                    w.update(cx, |waku, cx| waku.open_split(session_id, true, cx));
                            }
                        }),
                        MenuItem::new(tr!("tabs.split_right"), {
                            let w = w.clone();
                            move |_, cx| {
                                let _ =
                                    w.update(cx, |waku, cx| waku.open_split(session_id, false, cx));
                            }
                        }),
                    ];
                    if is_split {
                        items.push(MenuItem::Separator);
                        let w = w.clone();
                        items.push(MenuItem::new(tr!("tabs.close_split"), move |_, cx| {
                            let _ = w.update(cx, |waku, cx| waku.close_split(cx));
                        }));
                    }
                    items
                },
            ));
        }
        Some(strip.into_any_element())
    }

    fn reorder_chat_tab(&mut self, dragged: Uuid, target: Uuid, cx: &mut Context<Self>) {
        if dragged == target {
            return;
        }
        let Some(from) = self.chat_tabs.iter().position(|id| *id == dragged) else {
            return;
        };
        let id = self.chat_tabs.remove(from);
        let to = self
            .chat_tabs
            .iter()
            .position(|x| *x == target)
            .unwrap_or(self.chat_tabs.len());
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
    // Drop `dragged` onto `target`: it takes target's slot.
    fn reorder(tabs: &mut Vec<u8>, dragged: u8, target: u8) {
        if dragged == target {
            return;
        }
        let Some(from) = tabs.iter().position(|v| *v == dragged) else {
            return;
        };
        let v = tabs.remove(from);
        let to = tabs.iter().position(|x| *x == target).unwrap_or(tabs.len());
        tabs.insert(to, v);
    }

    fn pair_order(tabs: &[u8], main: Option<u8>, split: Option<u8>) -> Vec<u8> {
        match (main, split) {
            (Some(m), Some(s)) if m != s => {
                let mut v = Vec::with_capacity(tabs.len());
                for &id in tabs {
                    if id == s {
                        continue;
                    }
                    v.push(id);
                    if id == m {
                        v.push(s);
                    }
                }
                v
            }
            _ => tabs.to_vec(),
        }
    }

    #[test]
    fn reorder_moves_dragged_before_target() {
        let mut tabs = vec![1u8, 2, 3, 4];
        reorder(&mut tabs, 1, 3); // drop 1 onto 3
        assert_eq!(tabs, vec![2, 1, 3, 4]);

        let mut tabs = vec![1u8, 2, 3, 4];
        reorder(&mut tabs, 4, 1); // drop 4 onto 1 (to front)
        assert_eq!(tabs, vec![4, 1, 2, 3]);

        let mut tabs = vec![1u8, 2, 3];
        reorder(&mut tabs, 2, 2); // onto itself: no-op
        assert_eq!(tabs, vec![1, 2, 3]);
    }

    #[test]
    fn pair_order_puts_split_next_to_main() {
        assert_eq!(pair_order(&[1, 2, 3, 4], Some(1), Some(3)), vec![1, 3, 2, 4]);
        assert_eq!(pair_order(&[1, 2, 3], Some(2), None), vec![1, 2, 3]);
    }
}

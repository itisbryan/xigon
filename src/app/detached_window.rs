use super::*;

/// Root view of a torn-off session window. It shares the one `Waku` entity and
/// renders the session live. The body is a session card for now; the full live
/// transcript is the next P2 step — it needs per-session transcript state, since
/// the main window's list/scroll/selection state is a singleton.
pub(super) struct DetachedSessionView {
    waku: WeakEntity<Waku>,
    session_id: Uuid,
}

impl DetachedSessionView {
    fn new(waku: &Entity<Waku>, session_id: Uuid, cx: &mut Context<Self>) -> Self {
        // Re-render whenever shared app state changes, so the card stays live.
        cx.observe(waku, |_, _, cx| cx.notify()).detach();
        Self {
            waku: waku.downgrade(),
            session_id,
        }
    }
}

impl Render for DetachedSessionView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::current(cx);
        let session_id = self.session_id;
        let details = self.waku.upgrade().and_then(|waku| {
            let waku = waku.read(cx);
            waku.state
                .sessions
                .iter()
                .find(|session| session.id == session_id)
                .map(|session| {
                    (
                        super::sidebar::localized_session_title(session),
                        session.provider,
                        session.model.clone().unwrap_or_default(),
                    )
                })
        });
        let card = match details {
            Some((title, provider, model)) => div()
                .flex()
                .flex_col()
                .gap(px(8.0))
                .max_w(px(520.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(10.0))
                        .child(icon(provider_icon(provider), 20.0, theme.text_secondary))
                        .child(
                            div()
                                .min_w_0()
                                .text_size(px(18.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme.text)
                                .child(SharedString::from(title)),
                        ),
                )
                .when(!model.is_empty(), |el| {
                    el.child(
                        div()
                            .text_size(px(12.5))
                            .text_color(theme.text_secondary)
                            .child(SharedString::from(model)),
                    )
                })
                .into_any_element(),
            None => div()
                .text_size(px(13.0))
                .text_color(theme.text_tertiary)
                .child("—")
                .into_any_element(),
        };
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .p(px(24.0))
            .bg(theme.canvas)
            .text_color(theme.text)
            .child(card)
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
}

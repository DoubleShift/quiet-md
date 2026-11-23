//! Dialogs (shadcn's `Dialog`): a card over a dimmed window.
//!
//! ```ignore
//! scrim().child(Dialog::new("confirm").w(px(420.)).title("删除文档？").description("…")
//!     .footer(div().flex().justify_end().gap(px(8.)).child(Button::outline("no").label("取消")).child(Button::destructive("yes").label("删除"))))
//! ```

use super::base::{element, icon, label, under};
use super::{color, geo, shadow};
use gpui::prelude::FluentBuilder;
use gpui::*;

/// The veil over the window under a dialog (`black/10`), filling its parent, the dialog centred.
pub fn scrim() -> Div { div().absolute().inset_0().bg(color::overlay()).flex().items_center().justify_center() }

/// The dialog card (`DialogContent`: `popover`, `rounded-xl`, p-6, gap-6 between header, body and
/// footer) on the popover's edge over a deeper drop. Title (text-base) and description (text-sm,
/// `muted-foreground`) make its header; the caller's children its body; `footer` its buttons.
#[derive(IntoElement)]
pub struct Dialog {
    base: Stateful<Div>, title: Option<SharedString>, description: Option<SharedString>, footer: Option<AnyElement>, children: Vec<AnyElement>,
    close: Option<Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>>,
    escape: Option<Box<dyn Fn(&mut Window, &mut App)>>,
    enter: Option<Box<dyn Fn(&mut Window, &mut App)>>,
}
element!(Dialog);
impl ParentElement for Dialog {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) { self.children.extend(elements) }
}
impl Dialog {
    pub fn new(id: impl Into<ElementId>) -> Self { Dialog { base: div().id(id), title: None, description: None, footer: None, children: vec![], close: None, escape: None, enter: None } }
    pub fn title(mut self, s: impl Into<SharedString>) -> Self { self.title = Some(s.into()); self }
    pub fn description(mut self, s: impl Into<SharedString>) -> Self { self.description = Some(s.into()); self }
    pub fn footer(mut self, el: impl IntoElement) -> Self { self.footer = Some(el.into_any_element()); self }
    /// A ✕ in the top-right corner (`DialogClose`): a ghost icon button, `muted-foreground`,
    /// `foreground` under the pointer, level with the title; `on_close` runs on a click.
    pub fn closable(mut self, on_close: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self { self.close = Some(Box::new(on_close)); self }
    /// Holds the keyboard (`DialogFocus`'s handle, key context "Dialog"): the window's keys under it
    /// no longer reach the editor behind. `escape` cancels, `enter` runs the default button.
    pub fn keys(mut self, focus: &DialogFocus, escape: impl Fn(&mut Window, &mut App) + 'static, enter: Option<impl Fn(&mut Window, &mut App) + 'static>) -> Self {
        self.base = self.base.track_focus(&focus.handle).key_context("Dialog");
        self.escape = Some(Box::new(escape));
        self.enter = enter.map(|f| Box::new(f) as Box<dyn Fn(&mut Window, &mut App)>);
        self
    }
}

/// Where the keyboard goes while a dialog is open (shadcn's focus trap): `sync` it with whether
/// one is open each time the window draws; on opening it takes the focus from whoever had it, on
/// closing gives it back (unless something else took it meanwhile, as a page opened by 确定).
/// The dialog takes it with `Dialog::keys`.
pub struct DialogFocus { handle: FocusHandle, prev: Option<FocusHandle>, open: bool }
impl DialogFocus {
    pub fn new(cx: &mut App) -> Self { DialogFocus { handle: cx.focus_handle(), prev: None, open: false } }
    pub fn sync(&mut self, open: bool, window: &mut Window, cx: &mut App) {
        if open == self.open { return }
        self.open = open;
        if open {
            self.prev = window.focused(cx);
            window.focus(&self.handle, cx);
        } else if let Some(p) = self.prev.take() && (self.handle.is_focused(window) || window.focused(cx).is_none()) {
            window.focus(&p, cx);
        }
    }
}

/// The ✕ of `Dialog::closable`: 28pt, its centre on the title's line (padding 24, line 20).
fn close_button(on_close: Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>) -> Stateful<Div> {
    let g = geo();
    let at = g.dialog_pad + 10. - g.tool_h / 2.;
    div().id("dialog-close").group("dialog-close").absolute().top(px(at)).right(px(at)).size(px(g.tool_h)).flex().items_center().justify_center()
        .rounded(px(g.tool_r)).cursor_pointer().hover(|s| s.bg(color::accent_hover()))
        .child(icon("x", 16., color::muted_foreground()).group_hover("dialog-close", |s| s.text_color(color::foreground())))
        .on_click(move |e, window, cx| on_close(e, window, cx))
}
impl RenderOnce for Dialog {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let g = geo();
        let Dialog { base, title, description, footer, children, close, escape, enter } = self;
        let header = (title.is_some() || description.is_some()).then(|| div().flex().flex_col().gap(px(8.))
            .when_some(title, |d, t| d.child(div().text_size(px(16.)).line_height(px(20.)).text_color(color::foreground()).child(label(t))))
            .when_some(description, |d, t| d.child(div().text_size(px(g.text)).line_height(px(super::text::SM_LINE)).text_color(color::muted_foreground()).child(crate::ui::ui_para(t)))));
        under(base, |d| {
            d.occlude().bg(color::popover()).rounded(px(g.dialog_r)).shadow(shadow::dialog()).p(px(g.dialog_pad)).flex().flex_col().gap(px(g.dialog_gap))
                .font_family(crate::theme::ui_family()).text_size(px(g.text)).line_height(px(super::text::SM_LINE)).text_color(color::foreground())
                .children(header).children(children).children(footer)
                .when_some(close, |d, f| d.relative().child(close_button(f)))
                .when(escape.is_some() || enter.is_some(), |d| d.on_key_down(move |e, window, cx| {
                    let f = match e.keystroke.key.as_str() { "escape" => escape.as_ref(), "enter" if !e.keystroke.modifiers.modified() => enter.as_ref(), _ => None };
                    if let Some(f) = f { cx.stop_propagation(); f(window, cx); }
                }))
        })
    }
}

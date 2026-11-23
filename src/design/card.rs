//! Surfaces in a page (shadcn's `Card`, `Item`, `Separator`) and the transient note (Sonner's toast).
//!
//! ```ignore
//! Card::new("prefs").child(row).child(Separator::new()).child(row)
//! Toast::new(ToastTone::Success, "已复制")
//! ```

use super::base::{element, icon, label, under};
use super::{color, geo, shadow};
use gpui::prelude::FluentBuilder;
use gpui::*;

/// A card in a page (`Card`: `card`, `rounded-xl`) on affectUI's frame edge (`shadow::frame`): a
/// group of settings, a tile. It doesn't float, so no drop beyond 1–2pt. Children lay out as the
/// caller says (a column by default).
#[derive(IntoElement)]
pub struct Card { base: Stateful<Div> }
element!(Card, parent);
impl Card {
    pub fn new(id: impl Into<ElementId>) -> Self { Card { base: div().id(id) } }
}
impl RenderOnce for Card {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let g = geo();
        under(self.base, |d| d.bg(crate::theme::raised(color::card())).rounded(px(g.card_r)).shadow(shadow::frame()).flex().flex_col().text_color(color::foreground()))
    }
}

/// A rule in a page or a card (`Separator`: 1pt `border`), across by default; `vertical()` stands
/// it up (in a toolbar).
#[derive(IntoElement)]
pub struct Separator { base: Div, vertical: bool }
impl Styled for Separator { fn style(&mut self) -> &mut StyleRefinement { self.base.style() } }
impl Default for Separator { fn default() -> Self { Self::new() } }
impl Separator {
    pub fn new() -> Self { Separator { base: div(), vertical: false } }
    pub fn vertical(mut self) -> Self { self.vertical = true; self }
}
impl RenderOnce for Separator {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let mut d = self.base;
        let caller = std::mem::take(d.style());
        let mut d = d.flex_none().bg(color::border()).when(self.vertical, |d| d.w(px(1.)).h_full()).when(!self.vertical, |d| d.h(px(1.)).w_full());
        d.style().refine(&caller);
        d
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ToastTone { Success, Error, Info }

/// A toast (Sonner as shadcn ships it: `popover`, `rounded-lg`, p-4 at text-sm, a 16pt status glyph
/// disc in `foreground`, `destructive` for an error) on the popover's edge.
#[derive(IntoElement)]
pub struct Toast { base: Div, tone: ToastTone, text: SharedString }
impl Styled for Toast { fn style(&mut self) -> &mut StyleRefinement { self.base.style() } }
impl Toast {
    pub fn new(tone: ToastTone, text: impl Into<SharedString>) -> Self { Toast { base: div(), tone, text: text.into() } }
}
impl RenderOnce for Toast {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let g = geo();
        let (mark, size, c) = match self.tone {
            ToastTone::Success => ("check", 10., color::foreground()),
            ToastTone::Error => ("x", 10., color::destructive()),
            ToastTone::Info => ("info-mark", 12., color::foreground()),
        };
        // a filled disc with the mark cut out of it in the card's ground
        let glyph = div().relative().size(px(16.)).flex_none().flex().items_center().justify_center()
            .child(icon("circle-filled", 16., c).absolute().top_0().left_0()).child(icon(mark, size, color::popover()));
        let mut d = self.base;
        let caller = std::mem::take(d.style());
        let mut d = d.flex().items_center().gap(px(8.)).px(px(g.toast_px)).py(px(g.toast_py)).rounded(px(g.toast_r)).bg(color::popover()).shadow(shadow::popover())
            .font_family(crate::theme::ui_family()).text_size(px(g.text)).line_height(px(super::text::SM_LINE)).text_color(color::foreground())
            .child(glyph).child(label(self.text));
        d.style().refine(&caller);
        d
    }
}

/// A line in a card (shadcn's `Item`: `ItemTitle`, `ItemDescription`, `ItemActions`): a title and an
/// optional caption on the left, the control on the right; at least 48 tall, px-4 py-2.5.
#[derive(IntoElement)]
pub struct Item { base: Div, title: SharedString, description: Option<SharedString>, actions: Vec<AnyElement> }
impl Styled for Item { fn style(&mut self) -> &mut StyleRefinement { self.base.style() } }
impl ParentElement for Item { fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) { self.actions.extend(elements) } }
impl Item {
    pub fn new(title: impl Into<SharedString>) -> Self { Item { base: div(), title: title.into(), description: None, actions: vec![] } }
    pub fn description(mut self, s: Option<impl Into<SharedString>>) -> Self { self.description = s.map(Into::into); self }
}
impl RenderOnce for Item {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let g = geo();
        let Item { base: mut d, title, description, actions } = self;
        let caller = std::mem::take(d.style());
        let mut d = d.w_full().min_h(px(g.item_h)).px(px(g.item_px)).py(px(g.item_py)).flex().items_center().justify_between().gap(px(24.))
            .child(div().flex_1().min_w_0().flex().flex_col().gap(px(2.))
                .child(div().text_size(px(g.text)).line_height(px(super::text::SM_LINE)).text_color(color::foreground()).child(label(title)))
                .when_some(description, |d, s| d.child(div().text_size(px(g.text_xs)).line_height(px(super::text::XS_LINE)).text_color(color::muted_foreground()).child(crate::ui::ui_para(s)))))
            .child(div().flex_none().flex().items_center().gap(px(8.)).children(actions));
        d.style().refine(&caller);
        d
    }
}

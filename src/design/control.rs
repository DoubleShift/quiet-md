//! Choice controls (shadcn's `Checkbox`, `Switch`, `Tabs` as a segmented control). The checkbox and
//! the switch only draw; the caller's row or button takes the click.
//!
//! ```ignore
//! Checkbox::new(done)
//! Switch::new(on).disabled(!can)
//! Segmented::new().child(Segment::new("asc", "A → Z").selected(asc).on_click(…)).child(…)
//! ```

use super::base::{element, icon, label, under};
use super::{color, geo, shadow};
use gpui::prelude::FluentBuilder;
use gpui::*;

/// The checkbox: 16pt, corners 4, on an `input` hairline; on, `primary` with a `primary-foreground`
/// tick.
#[derive(IntoElement)]
pub struct Checkbox { base: Div, on: bool, partial: bool, disabled: bool }
impl Styled for Checkbox { fn style(&mut self) -> &mut StyleRefinement { self.base.style() } }
impl Checkbox {
    pub fn new(on: bool) -> Self { Checkbox { base: div(), on, partial: false, disabled: false } }
    /// Some of a set checked (`data-state=indeterminate`): filled like on, a dash for the tick.
    pub fn partial(mut self, on: bool) -> Self { self.partial = on; self }
    pub fn disabled(mut self, on: bool) -> Self { self.disabled = on; self }
}
impl RenderOnce for Checkbox {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let g = geo();
        let mut d = self.base;
        let caller = std::mem::take(d.style());
        let d = d.size(px(g.checkbox)).flex_none().flex().items_center().justify_center().rounded(px(g.checkbox_r)).when(self.disabled, |d| d.opacity(0.5));
        let mut d = if self.on { d.bg(color::primary()).child(icon("todo-check", 12., color::primary_foreground())) }
            else if self.partial { d.bg(color::primary()).child(div().w(px(8.)).h(px(2.)).rounded(px(1.)).bg(color::primary_foreground())) }
            else { d.border_1().border_color(color::input()).bg(color::field()).shadow(shadow::xs()) };
        d.style().refine(&caller);
        d
    }
}

/// A switch, 32×18: `primary` on, `input` off; a 16pt thumb (`background`, `primary-foreground` on in
/// the dark).
#[derive(IntoElement)]
pub struct Switch { base: Div, on: bool, disabled: bool }
impl Styled for Switch { fn style(&mut self) -> &mut StyleRefinement { self.base.style() } }
impl Switch {
    pub fn new(on: bool) -> Self { Switch { base: div(), on, disabled: false } }
    pub fn disabled(mut self, on: bool) -> Self { self.disabled = on; self }
}
impl RenderOnce for Switch {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let (g, dark, on) = (geo(), crate::theme::dark(), self.on);
        let thumb = if !dark { color::popover() } else if on { color::primary_foreground() } else { color::foreground() };
        let mut d = self.base;
        let caller = std::mem::take(d.style());
        let mut d = d.flex_none().w(px(g.switch_w)).h(px(g.switch_h)).px(px((g.switch_h - g.thumb) / 2.)).rounded_full().flex().items_center()
            .bg(if on { color::primary() } else { color::switch_off() }).when(on, |d| d.justify_end()).when(self.disabled, |d| d.opacity(0.5))
            .child(div().size(px(g.thumb)).rounded_full().bg(thumb));
        d.style().refine(&caller);
        d
    }
}

/// A segmented control's track (`TabsList`: `muted`, `rounded-lg`, p-[3px], h-8); its segments are
/// `Segment`s.
#[derive(IntoElement)]
pub struct Segmented { base: Div }
impl Styled for Segmented { fn style(&mut self) -> &mut StyleRefinement { self.base.style() } }
impl ParentElement for Segmented { fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) { self.base.extend(elements) } }
impl Default for Segmented { fn default() -> Self { Self::new() } }
impl Segmented {
    pub fn new() -> Self { Segmented { base: div() } }
    /// A track of `opts` with `picked` raised, each segment `(id, i)`.
    pub fn of(id: &'static str, opts: &[&str], picked: usize) -> Self {
        let mut t = Self::new();
        for (i, o) in opts.iter().enumerate() { t = t.child(Segment::new((id, i), o.to_string()).selected(i == picked)); }
        t
    }
}
impl RenderOnce for Segmented {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let g = geo();
        let mut d = self.base;
        let caller = std::mem::take(d.style());
        let mut d = d.flex_none().h(px(g.seg_h)).p(px(g.seg_pad)).rounded(px(g.seg_r)).bg(color::muted()).flex().items_center();
        d.style().refine(&caller);
        d
    }
}

/// One segment (`TabsTrigger`): picked, raised on the track (`background` with shadow-sm, inner
/// corners = track's − padding).
#[derive(IntoElement)]
pub struct Segment { base: Stateful<Div>, label: SharedString, icon: Option<SharedString>, selected: bool }
element!(Segment);
impl Segment {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self { Segment { base: div().id(id), label: label.into(), icon: None, selected: false } }
    /// A glyph before the label; with an empty label, an icon-only segment (a view switch).
    pub fn icon(mut self, name: impl Into<SharedString>) -> Self { self.icon = Some(name.into()); self }
    pub fn selected(mut self, on: bool) -> Self { self.selected = on; self }
}
impl RenderOnce for Segment {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let (g, on, t, ic) = (geo(), self.selected, self.label, self.icon);
        let fg = if on { color::foreground() } else { color::muted_foreground() };
        under(self.base, |d| {
            let d = d.flex_1().h_full().px(px(if t.is_empty() { 8. } else { 10. })).flex().items_center().justify_center().gap(px(6.)).rounded(px(g.seg_r - g.seg_pad)).cursor_pointer().whitespace_nowrap()
                .text_size(px(g.text)).text_color(fg)
                .when_some(ic, |d, n| d.child(icon(&n, 16., fg))).when(!t.is_empty(), |d| d.child(label(t)));
            if on { d.bg(crate::theme::raised(color::background())).shadow(shadow::sm()) } else { d.hover(|s| s.text_color(color::foreground())) }
        })
    }
}

/// A radio (`RadioGroupItem`: 16pt disc on an `input` hairline with shadow-xs; picked, a `primary`
/// rim round an 8pt `primary` dot). The caller's row takes the click.
#[derive(IntoElement)]
pub struct Radio { base: Div, on: bool, disabled: bool }
impl Styled for Radio { fn style(&mut self) -> &mut StyleRefinement { self.base.style() } }
impl Radio {
    pub fn new(on: bool) -> Self { Radio { base: div(), on, disabled: false } }
    pub fn disabled(mut self, on: bool) -> Self { self.disabled = on; self }
}
impl RenderOnce for Radio {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let g = geo();
        let mut d = self.base;
        let caller = std::mem::take(d.style());
        let mut d = d.size(px(g.checkbox)).flex_none().flex().items_center().justify_center().rounded_full().border_1().bg(color::field()).shadow(shadow::xs())
            .border_color(if self.on { color::primary() } else { color::input() }).when(self.disabled, |d| d.opacity(0.5))
            .when(self.on, |d| d.child(div().size(px(8.)).rounded_full().bg(color::primary())));
        d.style().refine(&caller);
        d
    }
}

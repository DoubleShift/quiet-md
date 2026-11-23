//! Form fields (shadcn's `Input`, `SelectTrigger`, `Textarea`, `Label`): the box a text input or a
//! value sits in. The text editing itself stays with the caller's input (`crate::input`); these draw
//! its box.
//!
//! ```ignore
//! Field::new("title").focused(editing).child(input)
//! Field::select("type").sized(FieldSize::Sm).open(menu_open).child(value)
//! ```

use super::base::{element, icon, label, under};
use super::{color, geo, shadow};
use gpui::prelude::FluentBuilder;
use gpui::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FieldSize {
    /// h-9: forms and dialogs.
    Default,
    /// h-8: a search box in a menu, a value in a panel line.
    Sm,
}

/// A one-line field: `input` hairline with shadow-xs, `rounded-md`, px-2.5 (white in the light look,
/// `input/30` in the dark). With the keyboard in it (`focused`), a `ring` edge and a 3pt `ring/50`
/// halo. A select (`Field::select`) adds a chevron and the pointer's fill; open, it looks focused.
#[derive(IntoElement)]
pub struct Field { base: Stateful<Div>, size: FieldSize, focused: bool, select: bool, disabled: bool, children: Vec<AnyElement> }
element!(Field);
impl ParentElement for Field {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) { self.children.extend(elements) }
}

impl Field {
    pub fn new(id: impl Into<ElementId>) -> Self { Field { base: div().id(id), size: FieldSize::Default, focused: false, select: false, disabled: false, children: vec![] } }
    /// A select's trigger: the field's box with a chevron at the end.
    pub fn select(id: impl Into<ElementId>) -> Self { let mut f = Self::new(id); f.select = true; f }
    /// A select showing `value` (the small one, as in a panel line).
    pub fn select_value(id: impl Into<ElementId>, value: impl Into<SharedString>) -> Self {
        Self::select(id).sized(FieldSize::Sm).child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().child(label(value.into())))
    }
    pub fn sized(mut self, s: FieldSize) -> Self { self.size = s; self }
    /// The keyboard is in it; for a select, it is open.
    pub fn focused(mut self, on: bool) -> Self { self.focused = on; self }
    pub fn open(self, on: bool) -> Self { self.focused(on) }
    pub fn disabled(mut self, on: bool) -> Self { self.disabled = on; self }
}

impl RenderOnce for Field {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let g = geo();
        let h = match self.size { FieldSize::Default => g.field_h, FieldSize::Sm => g.field_sm_h };
        let r = match self.size { FieldSize::Default => g.field_r, FieldSize::Sm => g.field_sm_r };
        let (focused, select, off, children) = (self.focused, self.select, self.disabled, self.children);
        under(self.base, |d| {
            let d = d.flex_none().h(px(h)).px(px(g.field_px)).flex().items_center().gap(px(6.)).rounded(px(r)).border_1()
                .text_size(px(g.text)).text_color(if off { color::disabled() } else { color::foreground() })
                .bg(if off { super::alpha(color::input(), 0.5) } else { color::field() });
            let d = if focused { d.border_color(color::ring()).shadow(vec![shadow::ring(color::focus_ring(), 3.)]) } else { d.border_color(color::input()).shadow(shadow::xs()) };
            d.when(select && !off, |d| d.cursor_pointer().when(!focused, |d| d.hover(|s| s.bg(color::field_hover()))))
                .children(children)
                .when(select, |d| d.child(icon("chevron-down", 14., color::muted_foreground()).ml_auto()))
        })
    }
}

/// A box that holds more than a line (`Textarea`: `rounded-md`, padded 10).
#[derive(IntoElement)]
pub struct Textarea { base: Stateful<Div>, focused: bool }
element!(Textarea, parent);
impl Textarea {
    pub fn new(id: impl Into<ElementId>) -> Self { Textarea { base: div().id(id), focused: false } }
    pub fn focused(mut self, on: bool) -> Self { self.focused = on; self }
}
impl RenderOnce for Textarea {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let (g, focused) = (geo(), self.focused);
        under(self.base, |d| {
            let d = d.p(px(g.area_pad)).rounded(px(g.area_r)).bg(color::field()).border_1().text_size(px(g.text)).text_color(color::foreground());
            if focused { d.border_color(color::ring()).shadow(vec![shadow::ring(color::focus_ring(), 3.)]) } else { d.border_color(color::input()).shadow(shadow::xs()) }
        })
    }
}

/// A field's label (`Label`: text-sm, `foreground`), `label_gap` above its control.
#[derive(IntoElement)]
pub struct Label { text: SharedString, muted: bool }
impl Label {
    pub fn new(text: impl Into<SharedString>) -> Self { Label { text: text.into(), muted: false } }
    /// A caption rather than a label: `muted-foreground`, text-xs.
    pub fn muted(mut self) -> Self { self.muted = true; self }
}
impl RenderOnce for Label {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let g = geo();
        div().flex_none().whitespace_nowrap().when(!self.muted, |d| d.text_size(px(g.text)).text_color(color::foreground()))
            .when(self.muted, |d| d.text_size(px(g.text_xs)).text_color(color::muted_foreground())).child(label(self.text))
    }
}

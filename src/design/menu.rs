//! Menus and popovers (shadcn's `DropdownMenu` / `Popover`): the floating surface, its rows, labels
//! and separators.
//!
//! ```ignore
//! Popover::new("row-menu").menu().min_w(px(180.))
//!     .child(MenuItem::new("rename").icon("pen-line").label("重命名").on_click(…))
//!     .child(MenuSeparator::new())
//!     .child(MenuItem::new("delete").icon("bin").label("删除").destructive(true))
//! ```
//!
//! A menu's rows sit `PAD` in from every side; code that places a submenu beside a row adds up
//! `PAD`, `ROW_H` and `SEP_H` rather than measuring.

use super::base::{element, icon, label, under};
use super::{G, color, geo, shadow};
use gpui::prelude::FluentBuilder;
use gpui::*;

/// A menu's padding (p-1), all round.
pub const PAD: f32 = G.menu_pad;
/// A row's height.
pub const ROW_H: f32 = G.menu_row_h;
/// A separator with its margin (my-1).
pub const SEP_H: f32 = 1. + 2. * SEP_MY;
const SEP_MY: f32 = 4.;
/// A label row's height (py-1.5 over text-xs).
pub const LABEL_H: f32 = 28.;

/// The floating surface: `popover` on affectUI's edge (`shadow::popover`), `rounded-md`, 14pt text.
/// It has an opaque ground, which the shadow-drawn edge needs. `menu()` makes it a column of rows.
#[derive(IntoElement)]
pub struct Popover { base: Stateful<Div>, menu: bool, padded: bool }
element!(Popover, parent);

impl Popover {
    pub fn new(id: impl Into<ElementId>) -> Self { Popover { base: div().id(id), menu: false, padded: false } }
    /// A column of menu rows, `PAD` above and below them (rows keep `PAD` from the sides).
    pub fn menu(mut self) -> Self { self.menu = true; self }
    /// A popover holding a form: padded 16 (p-4), a column with 16 between its fields (gap-4).
    pub fn padded(mut self) -> Self { self.padded = true; self }
}

impl RenderOnce for Popover {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let (g, menu, padded) = (geo(), self.menu, self.padded);
        under(self.base, |d| {
            d.occlude().bg(color::popover()).rounded(px(g.menu_r)).shadow(shadow::popover()).font_family(crate::theme::ui_family())
                .text_size(px(g.text)).line_height(px(super::text::SM_LINE)).text_color(color::foreground())
                .when(menu, |d| d.py(px(PAD)).flex().flex_col())
                .when(padded, |d| d.p(px(g.popover_pad)).flex().flex_col().gap(px(g.popover_gap)))
        })
    }
}

/// A menu row (`DropdownMenuItem`: px-2 py-1.5, gap-2, `rounded-sm`, `accent` under the pointer): an
/// optional glyph, the label, then whatever the caller adds, then the trailing mark (a check, a
/// shortcut, a submenu's chevron).
#[derive(IntoElement)]
pub struct MenuItem {
    base: Stateful<Div>,
    icon: Option<SharedString>,
    leading: Option<AnyElement>,
    label: Option<SharedString>,
    description: Option<SharedString>,
    extra: Vec<AnyElement>,
    shortcut: Option<SharedString>,
    checked: Option<bool>,
    submenu: bool,
    inset: bool,
    selected: bool,
    destructive: bool,
    disabled: bool,
}
element!(MenuItem);
impl ParentElement for MenuItem {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) { self.extra.extend(elements) }
}

impl MenuItem {
    pub fn new(id: impl Into<ElementId>) -> Self {
        MenuItem { base: div().id(id), icon: None, leading: None, label: None, description: None, extra: vec![], shortcut: None, checked: None,
            submenu: false, inset: false, selected: false, destructive: false, disabled: false }
    }
    pub fn label(mut self, s: impl Into<SharedString>) -> Self { self.label = Some(s.into()); self }
    /// A 16pt glyph before the label, `muted-foreground`.
    pub fn icon(mut self, name: impl Into<SharedString>) -> Self { let n = name.into(); if !n.is_empty() { self.icon = Some(n) } self }
    /// Anything else before the label (an illustration, a swatch).
    pub fn leading(mut self, el: impl IntoElement) -> Self { self.leading = Some(el.into_any_element()); self }
    /// A second, smaller line under the label (`muted-foreground`, text-xs); the row grows to fit.
    pub fn description(mut self, s: impl Into<SharedString>) -> Self { self.description = Some(s.into()); self }
    /// A key hint at the end (`DropdownMenuShortcut`).
    pub fn shortcut(mut self, s: impl Into<SharedString>) -> Self { self.shortcut = Some(s.into()); self }
    /// One of a set of choices: a check at the end when on; when off, its room kept.
    pub fn checked(mut self, on: bool) -> Self { self.checked = Some(on); self }
    /// Opens a submenu: a chevron at the end.
    pub fn submenu(mut self, on: bool) -> Self { self.submenu = on; self }
    /// Keeps a glyph's room before the label though there is none (`data-inset`).
    pub fn inset(mut self) -> Self { self.inset = true; self }
    /// Under the pointer without it (a row whose submenu is open, the keyboard's row).
    pub fn selected(mut self, on: bool) -> Self { self.selected = on; self }
    pub fn destructive(mut self, on: bool) -> Self { self.destructive = on; self }
    pub fn disabled(mut self, on: bool) -> Self { self.disabled = on; self }
}

impl RenderOnce for MenuItem {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let g = geo();
        let (danger, off, sel) = (self.destructive, self.disabled, self.selected);
        let fg = if off { color::disabled() } else if danger { color::destructive() } else { color::foreground() };
        let glyph = if off { color::disabled() } else if danger { color::destructive() } else { color::muted_foreground() };
        let fill = if danger { color::destructive_soft() } else { color::accent() };
        let two_lines = self.description.is_some();
        let MenuItem { base, icon: ic, leading, label: lab, description, extra, shortcut, checked, submenu, inset, .. } = self;
        under(base, |d| {
            d.flex_none().when(!two_lines, |d| d.h(px(g.menu_row_h))).when(two_lines, |d| d.min_h(px(g.menu_row_h)).py(px(6.)))
                .mx(px(g.menu_pad)).px(px(g.menu_row_px)).flex().items_center().gap(px(g.menu_gap)).rounded(px(g.menu_row_r))
                .text_size(px(g.text)).line_height(px(super::text::SM_LINE)).text_color(fg)
                .when(!off, |d| d.cursor_pointer().hover(move |s| s.bg(fill)))
                .when(sel && !off, |d| d.bg(fill))
                .when_some(ic, |d, n| d.child(icon(&n, 16., glyph)))
                .when_some(leading, |d, el| d.child(el))
                .when(inset, |d| d.child(div().flex_none().w(px(16.))))
                .when_some(lab, |d, s| {
                    let text = div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().child(label(s));
                    match description {
                        Some(desc) => d.child(div().flex_1().min_w_0().flex().flex_col().child(text)
                            .child(div().text_size(px(g.text_xs)).line_height(px(super::text::XS_LINE)).text_color(color::muted_foreground()).child(label(desc)))),
                        None => d.child(text),
                    }
                })
                .children(extra)
                .when_some(shortcut, |d, s| d.child(div().flex_none().text_size(px(g.text_xs)).text_color(color::muted_foreground()).child(s)))
                .when_some(checked, |d, on| d.child(div().flex_none().w(px(16.)).flex().justify_center().when(on, |d| d.child(icon("check", 14., if off { glyph } else { color::foreground() })))))
                .when(submenu, |d| d.child(icon("chevron-right", 14., glyph)))
        })
    }
}

/// A small heading over a group of rows (`DropdownMenuLabel`: text-xs, `muted-foreground`).
#[derive(IntoElement)]
pub struct MenuLabel { base: Stateful<Div>, text: SharedString }
element!(MenuLabel);
impl MenuLabel {
    pub fn new(text: impl Into<SharedString>) -> Self { let text = text.into(); MenuLabel { base: div().id(ElementId::Name(text.clone())), text } }
}
impl RenderOnce for MenuLabel {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let g = geo();
        let t = self.text;
        under(self.base, |d| d.flex_none().h(px(LABEL_H)).px(px(g.menu_pad + g.menu_row_px)).flex().items_center()
            .text_size(px(g.text_xs)).text_color(color::muted_foreground()).whitespace_nowrap().child(label(t)))
    }
}

/// A rule between groups of rows (`DropdownMenuSeparator`: `border`, my-1, edge to edge).
#[derive(IntoElement, Default)]
pub struct MenuSeparator;
impl MenuSeparator { pub fn new() -> Self { MenuSeparator } }
impl RenderOnce for MenuSeparator {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        div().flex_none().h(px(1.)).my(px(SEP_MY)).mx(px(geo().rule_inset)).bg(color::border())
    }
}

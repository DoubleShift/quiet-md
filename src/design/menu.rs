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
use std::cell::{Cell, RefCell};
use std::rc::Rc;

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
pub struct Popover { id: ElementId, base: Stateful<Div>, menu: bool, padded: bool }
element!(Popover, parent);

impl Popover {
    pub fn new(id: impl Into<ElementId>) -> Self { let id = id.into(); Popover { base: div().id(id.clone()), id, menu: false, padded: false } }
    /// A column of menu rows, `PAD` above and below them (rows keep `PAD` from the sides).
    pub fn menu(mut self) -> Self { self.menu = true; self }
    /// A popover holding a form: padded 16 (p-4), a column with 16 between its fields (gap-4).
    pub fn padded(mut self) -> Self { self.padded = true; self }
}

impl RenderOnce for Popover {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let (g, menu, padded) = (geo(), self.menu, self.padded);
        let d = under(self.base, |d| {
            d.occlude().bg(color::popover()).rounded(px(g.menu_r)).shadow(shadow::popover()).font_family(crate::theme::ui_family())
                .text_size(px(g.text)).line_height(px(super::text::SM_LINE)).text_color(color::foreground())
                .when(menu, |d| d.py(px(PAD)).flex().flex_col())
                .when(padded, |d| d.p(px(g.popover_pad)).flex().flex_col().gap(px(g.popover_gap)))
        });
        MenuScope { id: self.id, child: d.into_any_element() }
    }
}

// ---- one lit row per menu ----
//
// Radix's rule: a menu highlights at most one row, the pointer's or the keyboard's, whichever moved
// last; the row whose submenu is open stays lit while the pointer is in the submenu or on no row,
// and goes out when the pointer is on a sibling. A popover is one menu (`MenuScope`): its rows
// (those not inside a nearer popover) check in while it prepaints, it picks the lit one before it
// paints, and each row paints its fill only if it is that one. A submenu is a popover of its own.
// A row outside any popover (a list in a dialog) keeps the plain `hover` fill.

/// A menu's rows this frame and what was lit, kept across frames under the popover's id.
#[derive(Default)]
struct Rows {
    /// (id, hitbox, `selected`) for each row, in order.
    now: Vec<(ElementId, Hitbox, bool)>,
    lit: Option<ElementId>,
    /// The pointer has the say (else the keyboard: the `selected` row).
    pointer: bool,
    /// The pointer moved over a row while the keyboard had the say.
    moved: bool,
    last_hover: Option<ElementId>,
    last_sel: Option<ElementId>,
}
type Shared = Rc<RefCell<Rows>>;

thread_local! {
    /// Popovers being laid out around the current element (rows inside one leave `hover` to it).
    static DEPTH: Cell<u32> = const { Cell::new(0) };
    /// The menu whose rows are prepainting or painting.
    static SCOPE: RefCell<Option<Shared>> = const { RefCell::new(None) };
}

fn in_scope<R>(rows: Shared, f: impl FnOnce() -> R) -> R {
    let prev = SCOPE.with(|s| s.replace(Some(rows)));
    let r = f();
    SCOPE.with(|s| *s.borrow_mut() = prev);
    r
}
fn scope() -> Option<Shared> { SCOPE.with(|s| s.borrow().clone()) }

struct MenuScope { id: ElementId, child: AnyElement }
impl Element for MenuScope {
    type RequestLayoutState = ();
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> { Some(self.id.clone()) }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> { None }
    fn request_layout(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
        DEPTH.with(|d| d.set(d.get() + 1));
        let l = self.child.request_layout(window, cx);
        DEPTH.with(|d| d.set(d.get() - 1));
        (l, ())
    }
    fn prepaint(&mut self, id: Option<&GlobalElementId>, _: Option<&InspectorElementId>, _: Bounds<Pixels>, _: &mut (), window: &mut Window, cx: &mut App) {
        let rows = window.with_element_state(id.unwrap(), |s: Option<Shared>, _| { let s = s.unwrap_or_default(); (s.clone(), s) });
        rows.borrow_mut().now.clear();
        in_scope(rows, || self.child.prepaint(window, cx));
    }
    fn paint(&mut self, id: Option<&GlobalElementId>, _: Option<&InspectorElementId>, _: Bounds<Pixels>, _: &mut (), _: &mut (), window: &mut Window, cx: &mut App) {
        let rows = window.with_element_state(id.unwrap(), |s: Option<Shared>, _| { let s = s.unwrap_or_default(); (s.clone(), s) });
        {
            let r = &mut *rows.borrow_mut();
            let hover = r.now.iter().find(|x| x.1.is_hovered(window)).map(|x| x.0.clone());
            let sel = r.now.iter().find(|x| x.2).map(|x| x.0.clone());
            // the pointer took a row, left them all, or moved on one: it has the say
            if hover != r.last_hover || r.moved { r.pointer = hover.is_some(); }
            // the keyboard (or the caller: a submenu opened) moved the selection: it has
            if sel != r.last_sel && sel.is_some() { r.pointer = false; }
            r.lit = if r.pointer { hover.clone() } else { sel.clone() };
            (r.last_hover, r.last_sel, r.moved) = (hover, sel, false);
        }
        in_scope(rows, || self.child.paint(window, cx));
    }
}

/// A row's fill under a menu's rule: checks in while prepainting (a disabled row too: the pointer on it
/// lights none), paints `fill` if lit, and redraws
/// when the pointer comes or goes (or moves on it while the keyboard has the say).
fn lit_fill(id: ElementId, selected: bool, off: bool, fill: Hsla, r: f32) -> impl IntoElement {
    canvas(move |bounds, window, _| {
        let rows = scope()?;
        let hit = window.insert_hitbox(bounds, HitboxBehavior::Normal);
        rows.borrow_mut().now.push((id.clone(), hit.clone(), selected));
        Some((rows, hit, id))
    }, move |bounds, state, window, _| {
        let Some((rows, hit, id)) = state else { return };
        if !off && rows.borrow().lit.as_ref() == Some(&id) { window.paint_quad(gpui::fill(bounds, fill).corner_radii(px(r))); }
        let was = hit.is_hovered(window);
        window.on_mouse_event(move |_: &MouseMoveEvent, phase, window, _| {
            if phase != DispatchPhase::Capture { return }
            let now = hit.is_hovered(window);
            if now != was { window.refresh(); }
            else if now && !rows.borrow().pointer { rows.borrow_mut().moved = true; window.refresh(); }
        });
    }).absolute().inset_0()
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
    fn render(mut self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let g = geo();
        let (danger, off, sel) = (self.destructive, self.disabled, self.selected);
        let fg = if off { color::disabled() } else if danger { color::destructive() } else { color::foreground() };
        let glyph = if off { color::disabled() } else if danger { color::destructive() } else { color::muted_foreground() };
        let fill = if danger { color::destructive_soft() } else { color::accent() };
        let two_lines = self.description.is_some();
        // inside a popover the menu decides which one row is lit; elsewhere the pointer and `selected`
        let scoped = DEPTH.with(|d| d.get()) > 0;
        let id = self.base.interactivity().element_id.clone();
        let MenuItem { base, icon: ic, leading, label: lab, description, extra, shortcut, checked, submenu, inset, .. } = self;
        under(base, |d| {
            d.flex_none().when(!two_lines, |d| d.h(px(g.menu_row_h))).when(two_lines, |d| d.min_h(px(g.menu_row_h)).py(px(6.)))
                .mx(px(g.menu_pad)).px(px(g.menu_row_px)).flex().items_center().gap(px(g.menu_gap)).rounded(px(g.menu_row_r))
                .text_size(px(g.text)).line_height(px(super::text::SM_LINE)).text_color(fg)
                .when(!off, |d| d.cursor_pointer())
                .when_some(id.filter(|_| scoped), |d, id| d.child(lit_fill(id, sel, off, fill, g.menu_row_r)))
                .when(!scoped && !off, |d| d.hover(move |s| s.bg(fill)).when(sel, |d| d.bg(fill)))
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

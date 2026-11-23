//! Buttons (shadcn's `Button`): five variants in four sizes, a label and / or a glyph.
//!
//! ```ignore
//! Button::primary("ok").label("确定").on_click(…)
//! Button::ghost("undo").icon("undo").sized(ButtonSize::Tool).tip("撤销", None)
//! ```

use super::base::{element, icon, label, under};
use super::{color, geo, shadow};
use gpui::prelude::FluentBuilder;
use gpui::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ButtonVariant {
    /// `primary` under `primary-foreground`; `primary/80` under the pointer.
    Primary,
    /// A `border` hairline with shadow-xs on the page (`input` on `input/30` in the dark); `muted`
    /// under the pointer.
    Outline,
    /// `secondary`, a shade deeper under the pointer.
    Secondary,
    /// Text only; `accent` under the pointer (on a popover too, where `muted` would not show).
    Ghost,
    /// `destructive` text on `destructive/10`.
    Destructive,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ButtonSize {
    /// h-9: dialogs and forms.
    Default,
    /// h-8: panels.
    Sm,
    /// 28: toolbars, a panel's tools.
    Tool,
    /// 24: the small ✕ and arrows inside a field or a bar.
    Xs,
}

impl ButtonSize {
    fn h(self) -> f32 { let g = geo(); match self { ButtonSize::Default => g.button_h, ButtonSize::Sm => g.button_sm_h, ButtonSize::Tool => g.tool_h, ButtonSize::Xs => 24. } }
    fn icon(self) -> f32 { match self { ButtonSize::Xs => 14., _ => 16. } }
    fn px(self) -> f32 { match self { ButtonSize::Default | ButtonSize::Sm => geo().button_px, ButtonSize::Tool => 8., ButtonSize::Xs => 6. } }
}

#[derive(IntoElement)]
pub struct Button {
    base: Stateful<Div>,
    variant: ButtonVariant,
    size: ButtonSize,
    icon: Option<SharedString>,
    icon_end: Option<SharedString>,
    label: Option<SharedString>,
    selected: bool,
    disabled: bool,
}
element!(Button, parent);

impl Button {
    pub fn new(id: impl Into<ElementId>, variant: ButtonVariant) -> Self {
        Button { base: div().id(id), variant, size: ButtonSize::Default, icon: None, icon_end: None, label: None, selected: false, disabled: false }
    }
    pub fn primary(id: impl Into<ElementId>) -> Self { Self::new(id, ButtonVariant::Primary) }
    pub fn outline(id: impl Into<ElementId>) -> Self { Self::new(id, ButtonVariant::Outline) }
    pub fn secondary(id: impl Into<ElementId>) -> Self { Self::new(id, ButtonVariant::Secondary) }
    /// A quiet button; its size is a toolbar's (28) unless asked otherwise.
    pub fn ghost(id: impl Into<ElementId>) -> Self { Self::new(id, ButtonVariant::Ghost).sized(ButtonSize::Tool) }
    pub fn destructive(id: impl Into<ElementId>) -> Self { Self::new(id, ButtonVariant::Destructive) }

    pub fn sized(mut self, s: ButtonSize) -> Self { self.size = s; self }
    /// Another look for a button built elsewhere (a shared ⋯ button floating on a tile).
    pub fn variant(mut self, v: ButtonVariant) -> Self { self.variant = v; self }
    pub fn label(mut self, s: impl Into<SharedString>) -> Self { self.label = Some(s.into()); self }
    /// A glyph before the label; alone, the button is square.
    pub fn icon(mut self, name: impl Into<SharedString>) -> Self { let n = name.into(); if !n.is_empty() { self.icon = Some(n) } self }
    /// A glyph after the label (a chevron).
    pub fn icon_end(mut self, name: impl Into<SharedString>) -> Self { self.icon_end = Some(name.into()); self }
    /// Pressed or in use (a toolbar tool with a filter on, a menu's trigger while it is open): `muted`
    /// (`accent` for a ghost button, as shadcn's `Toggle`).
    pub fn selected(mut self, on: bool) -> Self { self.selected = on; self }
    /// Not available: the whole button at 50% (`disabled:opacity-50`), no pointer.
    pub fn disabled(mut self, on: bool) -> Self { self.disabled = on; self }
}

impl RenderOnce for Button {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let (g, dark, v, sz) = (geo(), crate::theme::dark(), self.variant, self.size);
        let square = self.label.is_none();
        let (fg, glyph) = match v {
            ButtonVariant::Primary => (color::primary_foreground(), color::primary_foreground()),
            ButtonVariant::Destructive => (color::destructive(), color::destructive()),
            ButtonVariant::Ghost => (color::foreground(), if self.selected { color::foreground() } else { color::muted_foreground() }),
            _ => (color::foreground(), color::muted_foreground()),
        };
        let r = if sz == ButtonSize::Default || sz == ButtonSize::Sm { g.button_r } else { g.tool_r };
        let (icon_l, icon_r, label_s, disabled, selected) = (self.icon, self.icon_end, self.label, self.disabled, self.selected);
        under(self.base, |d| {
            let d = d.flex_none().h(px(sz.h())).flex().items_center().justify_center().gap(px(if sz == ButtonSize::Default { 6. } else { 4. })).rounded(px(r))
                .text_size(px(g.text)).text_color(fg).whitespace_nowrap()
                .when(square, |d| d.w(px(sz.h()))).when(!square, |d| d.px(px(sz.px())))
                .when(!disabled, |d| d.cursor_pointer()).when(disabled, |d| d.opacity(0.5));
            let d = match v {
                ButtonVariant::Primary => d.bg(color::primary()).when(!disabled, |d| d.hover(|s| s.bg(color::primary_hover()))),
                ButtonVariant::Outline => d.border_1().border_color(if dark { color::input() } else { color::border() })
                    .bg(if selected { color::muted() } else if dark { color::field() } else { color::background() }).shadow(shadow::xs())
                    .when(!disabled, |d| d.hover(|s| s.bg(if dark { color::field_hover() } else { color::muted() }))),
                ButtonVariant::Secondary => d.bg(color::secondary()).when(!disabled, |d| d.hover(|s| s.bg(color::secondary_hover()))),
                // on, like shadcn's `Toggle` (`data-[state=on]:bg-accent`): `muted` would vanish on a dark popover
                ButtonVariant::Ghost => d.when(selected, |d| d.bg(color::accent())).when(!disabled, |d| d.hover(|s| s.bg(color::accent()))),
                ButtonVariant::Destructive => d.bg(color::destructive_soft()).when(!disabled, |d| d.hover(|s| s.bg(color::destructive_hover()))),
            };
            d.when_some(icon_l, |d, n| d.child(icon(&n, sz.icon(), glyph)))
                .when_some(label_s, |d, s| d.child(label(s)))
                .when_some(icon_r, |d, n| d.child(icon(&n, 14., glyph)))
        })
    }
}

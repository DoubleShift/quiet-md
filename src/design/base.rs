//! What every component shares: it wraps one `Stateful<Div>` that the caller styles and wires up
//! (`.w()`, `.on_click()`, `.tip()` …) before it renders, and on render lays the component's own
//! look *under* the caller's, so the caller's sizes and colours win. The pointer's look (`hover`) is
//! the component's: ask for a state (`selected`, `disabled`) instead of restyling it.

use gpui::*;

/// `d` with `own` applied first and the caller's refinements (already on `d`) over it.
pub(crate) fn under(mut d: Stateful<Div>, own: impl FnOnce(Stateful<Div>) -> Stateful<Div>) -> Stateful<Div> {
    let caller = std::mem::take(d.style());
    let mut d = own(d);
    d.style().refine(&caller);
    d
}

/// Text in the UI face that keeps full-width punctuation at full width.
pub(crate) fn label(s: SharedString) -> crate::ui::UiText { crate::ui::ui_text(s) }

/// The traits that let a component take the caller's styling, handlers and tooltips.
macro_rules! element {
    ($t:ty) => {
        impl gpui::Styled for $t { fn style(&mut self) -> &mut gpui::StyleRefinement { gpui::Styled::style(&mut self.base) } }
        impl gpui::InteractiveElement for $t { fn interactivity(&mut self) -> &mut gpui::Interactivity { self.base.interactivity() } }
        impl gpui::StatefulInteractiveElement for $t {}
        impl crate::ui::TipExt for $t {
            fn tip_label(mut self, label: String, side: crate::ui::TipSide) -> Self { self.base = self.base.tip_label(label, side); self }
        }
    };
    ($t:ty, parent) => {
        element!($t);
        impl gpui::ParentElement for $t {
            fn extend(&mut self, elements: impl IntoIterator<Item = gpui::AnyElement>) { self.base.extend(elements) }
        }
    };
}
pub(crate) use element;

/// Glyphs the editor's icon set has and the shell's doesn't.
const EDITOR_ICONS: &[&str] = &["calendar", "columns-2", "table", "layout-grid", "grip-vertical", "maximize", "undo", "redo", "download", "upload", "link", "paperclip",
    "user", "trash-2", "list-tree", "history", "row-add", "column-add", "type", "image", "move", "wrap-text", "replace", "caret-down", "caret-up", "triangle-down",
    "triangle-right", "square-plus", "unlink", "todo", "circle-help", "view-card", "timer", "one-to-one", "swatch-none", "fit-page"];

/// An icon from either set: the editor's `editor-icons/` for the names above and any the shell's
/// `icons/` lacks, else the shell's.
pub fn icon(name: &str, size: f32, color: Hsla) -> Svg {
    let editor = EDITOR_ICONS.contains(&name) || crate::assets::get(&format!("icons/{name}.svg")).is_none();
    if editor { crate::editor_ui::icon(name, size, color) } else { crate::ui::icon(name, size, color) }
}

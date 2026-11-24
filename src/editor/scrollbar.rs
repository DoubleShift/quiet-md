//! The page scrollbar: a thin trackless thumb on the right edge, drawn over
//! the virtualized list after it lays out, so it never lags a frame behind.

use super::view::Editor;
use crate::feishu_theme::{self as theme, Theme, craft};
use gpui::*;
use std::cell::Cell;

const W: f32 = 4.;
const W_HOT: f32 = 6.;
const INSET: f32 = 2.;
const MIN_H: f32 = 32.;
/// The grab zone is wider than the thumb.
const HIT_W: f32 = 12.;

#[derive(Default)]
pub struct Scrollbar {
    /// The pointer was over the thumb last frame.
    hot: Cell<bool>,
    /// The page thumb while dragged.
    held: Cell<Option<Grab>>,
    /// A wide table's thumb while dragged: pointer offset into the thumb, and
    /// its width (the middle is unused).
    grab: Cell<Option<(Pixels, Pixels, Pixels)>>,
}

/// A thumb being dragged. It stays under the pointer, and the page follows
/// it: unmeasured rows count at the average height, so the estimated length
/// changes as they come on screen, and the page is aimed again until it sits
/// where the thumb says. (Holding the length from the press instead, a first
/// screen of tall blocks put the end of the page a third down the track, and
/// the thumb would go no further.)
#[derive(Clone, Copy)]
struct Grab {
    /// Pointer offset into the thumb.
    dy: Pixels,
    /// Thumb height at the press.
    h: Pixels,
    /// Where the thumb is, 0 (top) to 1 (end).
    f: f32,
    /// Re-aims left before the estimate is taken as it is.
    tries: u8,
}

const TRIES: u8 = 8;

/// The thumb's top within the track and its height, for the scroll `offset`
/// in `content` seen through `view` on a `track` this tall; `None` when it
/// all fits. A held thumb is where it was dragged to.
fn thumb(offset: Pixels, content: Pixels, view: Pixels, track: Pixels, grab: Option<Grab>) -> Option<(Pixels, Pixels)> {
    if content <= view + px(1.) {
        return None;
    }
    let h = grab.map_or_else(|| (view * (view / content)).max(px(MIN_H)).min(track), |g| g.h);
    let f = grab.map_or_else(|| (offset / (content - view)).clamp(0., 1.), |g| g.f);
    Some(((track - h) * f, h))
}

/// Where a thumb at `f` puts the page in `content`: the end is the real end,
/// which the estimate may put short of it.
fn aim(f: f32, content: Pixels, view: Pixels) -> Pixels {
    if f >= 1. { px(f32::MAX) } else { (content - view) * f }
}

/// Whether the page has drifted from a held thumb (the estimate changed).
fn drifted(g: &Grab, offset: Pixels, content: Pixels, view: Pixels) -> bool {
    g.f < 1. && g.tries > 0 && (offset - aim(g.f, content, view)).abs() > px(0.5)
}

impl Editor {
    pub(super) fn page_scrollbar(&self) -> impl IntoElement {
        let list = self.list.clone();
        let sb = self.scrollbar.clone();
        let sb2 = sb.clone();
        let list2 = list.clone();
        canvas(
            move |bounds, window, _| {
                let (offset, content, view) = list.estimated_scroll_metrics();
                let grab = sb.held.get();
                if let Some(g) = grab
                    && drifted(&g, offset, content, view)
                {
                    sb.held.set(Some(Grab { tries: g.tries - 1, ..g }));
                    list.scroll_to_estimated_offset(aim(g.f, content, view));
                    window.refresh();
                }
                let (y, h) = thumb(offset, content, view, bounds.size.height, grab)?;
                let top = bounds.top() + y;
                let f = if bounds.size.height > h { y / (bounds.size.height - h) } else { 0. };
                let zone = Bounds::new(point(bounds.right() - px(HIT_W), top), size(px(HIT_W), h));
                Some((top, h, f, window.insert_hitbox(zone, HitboxBehavior::BlockMouseExceptScroll)))
            },
            move |bounds, state, window, _| {
                let sb = sb2;
                let Some((top, h, f, hitbox)) = state else {
                    sb.held.set(None);
                    return;
                };
                window.set_cursor_style(CursorStyle::Arrow, &hitbox);
                let hovered = hitbox.is_hovered(window);
                sb.hot.set(hovered);
                let dragging = sb.held.get().is_some();
                let w = if hovered || dragging { W_HOT } else { W };
                // Black at 22%, darker under the pointer and while held.
                let color = if dragging {
                    theme::ca(craft::N1000, 0.4)
                } else if hovered {
                    theme::ca(craft::N1000, 0.32)
                } else {
                    Theme::scrollbar_thumb()
                };
                let thumb = Bounds::new(point(bounds.right() - px(INSET + w), top), size(px(w), h));
                window.paint_quad(fill(thumb, color).corner_radii(px(w / 2.)));

                window.on_mouse_event({
                    let (sb, hitbox) = (sb.clone(), hitbox.clone());
                    move |ev: &MouseDownEvent, phase, window, cx| {
                        if phase == DispatchPhase::Bubble && ev.button == MouseButton::Left && hitbox.is_hovered(window) {
                            sb.held.set(Some(Grab { dy: ev.position.y - top, h, f, tries: TRIES }));
                            cx.stop_propagation();
                            window.refresh();
                        }
                    }
                });
                window.on_mouse_event({
                    let (sb, hitbox, list) = (sb.clone(), hitbox.clone(), list2.clone());
                    move |ev: &MouseMoveEvent, phase, window, _| {
                        if phase != DispatchPhase::Bubble {
                            return;
                        }
                        match sb.held.get() {
                            Some(g) if ev.pressed_button == Some(MouseButton::Left) => {
                                let travel = (bounds.size.height - g.h).max(px(1.));
                                let f = ((ev.position.y - g.dy - bounds.top()) / travel).clamp(0., 1.);
                                sb.held.set(Some(Grab { f, tries: TRIES, ..g }));
                                let (_, content, view) = list.estimated_scroll_metrics();
                                list.scroll_to_estimated_offset(aim(f, content, view));
                                window.refresh();
                            }
                            // The button went up somewhere we didn't see.
                            Some(_) => {
                                sb.held.set(None);
                                window.refresh();
                            }
                            None if hitbox.is_hovered(window) != sb.hot.get() => window.refresh(),
                            None => {}
                        }
                    }
                });
                window.on_mouse_event({
                    let sb = sb.clone();
                    move |ev: &MouseUpEvent, phase, window, _| {
                        if phase == DispatchPhase::Bubble && ev.button == MouseButton::Left && sb.held.take().is_some() {
                            window.refresh();
                        }
                    }
                });
            },
        )
        .absolute()
        .top_0()
        .right_0()
        .bottom_0()
        .w(px(HIT_W))
    }
}

/// A wide table's scrollbar: the page thumb turned sideways, along the top
/// of its box (`HIT_W` tall), for the scroller that `handle` tracks.
pub(super) fn table_scrollbar(handle: ScrollHandle, sb: std::rc::Rc<Scrollbar>) -> Canvas<Option<(Pixels, Pixels, Hitbox)>> {
    let sb2 = sb.clone();
    let list = handle.clone();
    canvas(
        move |bounds, window, _| {
            let max = list.max_offset().x;
            if max <= px(1.) {
                return None;
            }
            let view = bounds.size.width;
            let content = view + max;
            let w = match sb.grab.get() {
                Some((_, _, w)) => w,
                None => (view * (view / content)).max(px(MIN_H)).min(view),
            };
            let left = bounds.left() + (view - w) * (-list.offset().x / max).clamp(0., 1.);
            let zone = Bounds::new(point(left, bounds.top()), size(w, px(HIT_W)));
            Some((left, w, window.insert_hitbox(zone, HitboxBehavior::BlockMouseExceptScroll)))
        },
        move |bounds, state, window, _| {
            let sb = sb2;
            let Some((left, w, hitbox)) = state else {
                sb.grab.set(None);
                return;
            };
            window.set_cursor_style(CursorStyle::Arrow, &hitbox);
            let hovered = hitbox.is_hovered(window);
            sb.hot.set(hovered);
            let dragging = sb.grab.get().is_some();
            let h = if hovered || dragging { W_HOT } else { W };
            let color = if dragging {
                theme::ca(craft::N1000, 0.4)
            } else if hovered {
                theme::ca(craft::N1000, 0.32)
            } else {
                Theme::scrollbar_thumb()
            };
            let thumb = Bounds::new(point(left, bounds.top() + px(INSET)), size(w, px(h)));
            window.paint_quad(fill(thumb, color).corner_radii(px(h / 2.)));

            window.on_mouse_event({
                let (sb, hitbox) = (sb.clone(), hitbox.clone());
                move |ev: &MouseDownEvent, phase, window, cx| {
                    if phase == DispatchPhase::Bubble && ev.button == MouseButton::Left && hitbox.is_hovered(window) {
                        sb.grab.set(Some((ev.position.x - left, px(0.), w)));
                        cx.stop_propagation();
                        window.refresh();
                    }
                }
            });
            window.on_mouse_event({
                let (sb, hitbox, list) = (sb.clone(), hitbox.clone(), handle.clone());
                move |ev: &MouseMoveEvent, phase, window, _| {
                    if phase != DispatchPhase::Bubble {
                        return;
                    }
                    match sb.grab.get() {
                        Some((dx, _, w)) if ev.pressed_button == Some(MouseButton::Left) => {
                            let travel = (bounds.size.width - w).max(px(1.));
                            let f = ((ev.position.x - dx - bounds.left()) / travel).clamp(0., 1.);
                            list.set_offset(point(-list.max_offset().x * f, list.offset().y));
                            window.refresh();
                        }
                        Some(_) => {
                            sb.grab.set(None);
                            window.refresh();
                        }
                        None if hitbox.is_hovered(window) != sb.hot.get() => window.refresh(),
                        None => {}
                    }
                }
            });
            window.on_mouse_event({
                let sb = sb.clone();
                move |ev: &MouseUpEvent, phase, window, _| {
                    if phase == DispatchPhase::Bubble && ev.button == MouseButton::Left && sb.grab.take().is_some() {
                        window.refresh();
                    }
                }
            });
        },
    )
}

#[cfg(test)]
mod tests {
    use super::{Grab, TRIES, aim, drifted, thumb};
    use gpui::px;

    fn held(f: f32) -> Option<Grab> { Some(Grab { dy: px(0.), h: px(40.), f, tries: TRIES }) }

    #[test]
    fn the_thumb_reaches_the_end_with_the_page() {
        let (view, track) = (px(600.), px(600.));
        // At the end of the page the thumb is at the end of the track, whatever the length.
        for content in [px(1200.), px(24_000.), px(250_000.)] {
            let (y, h) = thumb(content - view, content, view, track, None).unwrap();
            assert_eq!(y + h, track, "{content:?}");
            assert_eq!(thumb(px(0.), content, view, track, None).unwrap().0, px(0.));
        }
        assert!(thumb(px(0.), px(600.5), view, track, None).is_none(), "it all fits");
        // Held, it is where the pointer put it, not where a changing estimate says.
        let (y, h) = thumb(px(5_000.), px(250_000.), view, track, held(1.)).unwrap();
        assert_eq!((y + h, h), (track, px(40.)));
        assert_eq!(thumb(px(0.), px(9_000.), view, track, held(0.5)).unwrap().0, px(280.));
    }

    #[test]
    fn a_held_thumb_takes_the_page_with_it() {
        let view = px(600.);
        // The end of the track is the real end, which the estimate may put short of it.
        assert_eq!(aim(1., px(10_000.), view), px(f32::MAX));
        assert_eq!(aim(0.5, px(10_600.), view), px(5_000.));
        // The estimate shrank as rows were measured: the page is aimed again …
        let g = held(0.5).unwrap();
        assert!(drifted(&g, px(5_000.), px(4_600.), view));
        assert!(!drifted(&g, px(2_000.), px(4_600.), view));
        // … but not past the end (the real end is the end), nor for ever.
        assert!(!drifted(&held(1.).unwrap(), px(0.), px(4_600.), view));
        assert!(!drifted(&Grab { tries: 0, ..g }, px(5_000.), px(4_600.), view));
    }
}

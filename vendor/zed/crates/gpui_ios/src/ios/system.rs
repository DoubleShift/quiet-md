//! quiet.md: the parts of UIKit an app needs to feel native that GPUI has no
//! notion of — the Taptic Engine, the status-bar tap that scrolls to the top,
//! and the accessibility settings an app is expected to follow (Dynamic Type,
//! Bold Text, Reduce Motion). Classes are looked up by name, so no further
//! `objc2-ui-kit` features are compiled in.

use gpui_apple::main_thread_marker;
use objc2::{
    ClassType, define_class, msg_send,
    rc::{Allocated, Retained},
    runtime::{AnyClass, AnyObject},
};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_foundation::{NSObject, NSObjectProtocol, NSString};
use std::{cell::RefCell, path::Path};

/// What the Taptic Engine plays (`UIFeedbackGenerator`'s vocabulary).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Haptic {
    /// `UIImpactFeedbackGenerator`, light: a control landing, a threshold crossed.
    Light,
    /// The same, medium: something picked up.
    Medium,
    /// The same, heavy.
    Heavy,
    /// `UISelectionFeedbackGenerator`: a value ticking over (a switch, a snap).
    Selection,
    /// `UINotificationFeedbackGenerator`: a task done.
    Success,
    /// The same: about to do something destructive.
    Warning,
    /// The same: something failed.
    Error,
}

/// Plays `kind` on the Taptic Engine. Nothing happens off the main thread or
/// on a device without one.
pub fn haptic(kind: Haptic) {
    if main_thread_marker().is_none() {
        return;
    }
    let generator = |class: &std::ffi::CStr| -> Option<Allocated<AnyObject>> {
        let class = AnyClass::get(class)?;
        // SAFETY: `alloc` on a class UIKit registered.
        Some(unsafe { msg_send![class, alloc] })
    };
    // SAFETY: the selectors and argument types are UIKit's documented ones for
    // these classes; the generators are created, used and released here.
    unsafe {
        match kind {
            Haptic::Light | Haptic::Medium | Haptic::Heavy => {
                let Some(allocated) = generator(c"UIImpactFeedbackGenerator") else { return };
                let style: isize = match kind { Haptic::Light => 0, Haptic::Medium => 1, _ => 2 };
                let generator: Retained<AnyObject> = msg_send![allocated, initWithStyle: style];
                let _: () = msg_send![&*generator, impactOccurred];
            }
            Haptic::Selection => {
                let Some(allocated) = generator(c"UISelectionFeedbackGenerator") else { return };
                let generator: Retained<AnyObject> = msg_send![allocated, init];
                let _: () = msg_send![&*generator, selectionChanged];
            }
            Haptic::Success | Haptic::Warning | Haptic::Error => {
                let Some(allocated) = generator(c"UINotificationFeedbackGenerator") else { return };
                let kind: isize = match kind { Haptic::Success => 0, Haptic::Warning => 1, _ => 2 };
                let generator: Retained<AnyObject> = msg_send![allocated, init];
                let _: () = msg_send![&*generator, notificationOccurred: kind];
            }
        }
    }
}

#[link(name = "UIKit", kind = "framework")]
unsafe extern "C" {
    fn UIAccessibilityIsBoldTextEnabled() -> bool;
    fn UIAccessibilityIsReduceMotionEnabled() -> bool;
}

/// Settings › Accessibility › Bold Text.
pub fn bold_text_enabled() -> bool {
    // SAFETY: a UIKit function with no arguments, callable from any thread.
    unsafe { UIAccessibilityIsBoldTextEnabled() }
}

/// Settings › Accessibility › Motion › Reduce Motion.
pub fn reduce_motion_enabled() -> bool {
    // SAFETY: as above.
    unsafe { UIAccessibilityIsReduceMotionEnabled() }
}

/// Dynamic Type: how much larger (or smaller) than the default the user's
/// text size makes body text. 1 at the default size; about 0.82 to 3.1 across
/// the sizes, accessibility ones included.
pub fn text_scale() -> f32 {
    let Some(class) = AnyClass::get(c"UIFontMetrics") else { return 1. };
    // SAFETY: `+[UIFontMetrics defaultMetrics]` and `-scaledValueForValue:` (CGFloat in, CGFloat out).
    let scaled: f64 = unsafe {
        let metrics: Retained<AnyObject> = msg_send![class, defaultMetrics];
        msg_send![&*metrics, scaledValueForValue: 100.0f64]
    };
    if scaled.is_finite() && scaled > 0. { (scaled / 100.) as f32 } else { 1. }
}

thread_local! {
    static STATUS_BAR_TAP: RefCell<Option<Box<dyn FnMut()>>> = const { RefCell::new(None) };
    static SCROLL_TO_TOP_DELEGATE: RefCell<Option<Retained<ScrollToTopDelegate>>> = const { RefCell::new(None) };
}

/// Calls `callback` when the status bar is tapped, the gesture that scrolls
/// an iOS app's content to its top. One callback at a time; it runs on the
/// main thread.
pub fn on_status_bar_tap(callback: impl FnMut() + 'static) {
    STATUS_BAR_TAP.with_borrow_mut(|slot| *slot = Some(Box::new(callback)));
}

define_class!(
    #[unsafe(super = NSObject)]
    #[name = "GPUIScrollToTopDelegate"]
    struct ScrollToTopDelegate;

    unsafe impl NSObjectProtocol for ScrollToTopDelegate {}

    impl ScrollToTopDelegate {
        // UIScrollViewDelegate: asked on a status-bar tap. The scroll view
        // itself stays put; GPUI's content does the scrolling.
        #[unsafe(method(scrollViewShouldScrollToTop:))]
        fn should_scroll_to_top(&self, _scroll_view: &AnyObject) -> bool {
            // Taken out while it runs: the callback may register another.
            let callback = STATUS_BAR_TAP.with_borrow_mut(Option::take);
            if let Some(mut callback) = callback {
                callback();
                STATUS_BAR_TAP.with_borrow_mut(|slot| {
                    if slot.is_none() {
                        *slot = Some(callback);
                    }
                });
            }
            false
        }
    }
);

/// UIKit delivers the status-bar tap to the window's one scroll view with
/// `scrollsToTop`, and a Metal view has none: this adds a 1pt scroll view
/// (scrolled off its top so there is somewhere to scroll to) whose delegate
/// reports the tap.
pub(super) fn install_scroll_to_top(parent: &AnyObject) {
    let Some(class) = AnyClass::get(c"UIScrollView") else { return };
    // SAFETY: UIScrollView's documented selectors, on the main thread (the
    // caller is creating the window). The delegate is kept alive for the
    // thread's lifetime because UIScrollView holds it weakly.
    unsafe {
        let scroll_view: Retained<AnyObject> = msg_send![class, new];
        let _: () = msg_send![&*scroll_view, setFrame: CGRect::new(CGPoint::ZERO, CGSize::new(1., 1.))];
        let _: () = msg_send![&*scroll_view, setContentSize: CGSize::new(1., 3.)];
        let _: () = msg_send![&*scroll_view, setContentOffset: CGPoint::new(0., 1.)];
        let _: () = msg_send![&*scroll_view, setScrollsToTop: true];
        let _: () = msg_send![&*scroll_view, setShowsVerticalScrollIndicator: false];
        let delegate: Retained<ScrollToTopDelegate> = msg_send![ScrollToTopDelegate::class(), new];
        let _: () = msg_send![&*scroll_view, setDelegate: &*delegate];
        SCROLL_TO_TOP_DELEGATE.with_borrow_mut(|slot| *slot = Some(delegate));
        let _: () = msg_send![parent, addSubview: &*scroll_view];
    }
}

/// Opens the Files app at `folder`.
pub(super) fn open_in_files(folder: &Path) {
    let Some(mtm) = main_thread_marker() else { return };
    let (Some(url_class), Some(application)) = (AnyClass::get(c"NSURL"), AnyClass::get(c"UIApplication")) else { return };
    let _ = mtm;
    // SAFETY: `+[NSURL fileURLWithPath:]`, `-absoluteString`, `+[NSURL URLWithString:]`
    // and `-[UIApplication openURL:options:completionHandler:]` with their documented types.
    unsafe {
        let path = NSString::from_str(&folder.to_string_lossy());
        let file_url: Retained<AnyObject> = msg_send![url_class, fileURLWithPath: &*path, isDirectory: true];
        // The file URL with its scheme swapped: the path stays percent-encoded.
        let absolute: Retained<NSString> = msg_send![&*file_url, absoluteString];
        let Some(rest) = absolute.to_string().strip_prefix("file://").map(str::to_string) else { return };
        let shared = NSString::from_str(&format!("shareddocuments://{rest}"));
        let url: Option<Retained<AnyObject>> = msg_send![url_class, URLWithString: &*shared];
        let Some(url) = url else { return };
        let app: Retained<AnyObject> = msg_send![application, sharedApplication];
        let options: Retained<AnyObject> = msg_send![AnyClass::get(c"NSDictionary").expect("Foundation"), new];
        let none: Option<&block2::Block<dyn Fn(objc2::runtime::Bool)>> = None;
        let _: () = msg_send![&*app, openURL: &*url, options: &*options, completionHandler: none];
    }
}

/// Presents the share sheet for the file at `path` over `presenter`.
pub(super) fn share_file(path: &Path, presenter: &AnyObject) {
    let (Some(url_class), Some(array_class), Some(sheet_class)) =
        (AnyClass::get(c"NSURL"), AnyClass::get(c"NSArray"), AnyClass::get(c"UIActivityViewController")) else { return };
    // SAFETY: UIKit's documented selectors; `presenter` is a `UIViewController`.
    unsafe {
        let path = NSString::from_str(&path.to_string_lossy());
        let url: Retained<AnyObject> = msg_send![url_class, fileURLWithPath: &*path];
        let items: Retained<AnyObject> = msg_send![array_class, arrayWithObject: &*url];
        let allocated: Allocated<AnyObject> = msg_send![sheet_class, alloc];
        let none: Option<&AnyObject> = None;
        let sheet: Retained<AnyObject> = msg_send![allocated, initWithActivityItems: &*items, applicationActivities: none];
        // An iPad shows it as a popover, which needs somewhere to point: the middle of the presenter.
        let popover: Option<Retained<AnyObject>> = msg_send![&*sheet, popoverPresentationController];
        if let Some(popover) = popover {
            let view: Option<Retained<AnyObject>> = msg_send![presenter, view];
            if let Some(view) = view {
                let bounds: CGRect = msg_send![&*view, bounds];
                let middle = CGRect::new(CGPoint::new(bounds.size.width / 2., bounds.size.height / 2.), CGSize::new(1., 1.));
                let _: () = msg_send![&*popover, setSourceView: &*view];
                let _: () = msg_send![&*popover, setSourceRect: middle];
            }
        }
        let completion: Option<&block2::Block<dyn Fn()>> = None;
        let _: () = msg_send![presenter, presentViewController: &*sheet, animated: true, completion: completion];
    }
}

thread_local! { static SHAKE: RefCell<Option<Box<dyn FnMut()>>> = const { RefCell::new(None) }; }

/// Calls `callback` when the device is shaken (iOS's gesture for undo) while
/// the window's view or its text input is first responder.
pub fn on_shake(callback: impl FnMut() + 'static) {
    SHAKE.with_borrow_mut(|slot| *slot = Some(Box::new(callback)));
}

/// `motionEnded:withEvent:` on the Metal view, for `UIEventSubtypeMotionShake`.
pub(super) fn shaken() {
    let callback = SHAKE.with_borrow_mut(Option::take);
    if let Some(mut callback) = callback {
        callback();
        SHAKE.with_borrow_mut(|slot| {
            if slot.is_none() {
                *slot = Some(callback);
            }
        });
    }
}

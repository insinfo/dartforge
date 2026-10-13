//! The dialect of the official compiler that the generated file imitates.
//!
//! `ngdart` 8.0.0-dev.4 (`ngcompiler` 3.0.0-dev.3) and the `ngx_dart` fork
//! 9.0.0-dev.2 (`ngx_compiler` 9.0.0-dev.2), which replaces `dart:html` with
//! `package:web`, have the same compiler except for a short list of differences
//! (`docs/NGDART-COMPILADOR-DE-VISOES.md` part B): the package names, the DOM
//! identifiers, the built-in `JSObject` provider and the typed events.
//!
//! Internally the port always speaks `ngdart` 8: every URI read from the
//! program goes through [`canonical`] (`package:ngx_dart/…` becomes
//! `package:ngdart/…`), and the output goes back to the generation's dialect
//! through [`writing`] (in the import table). The dialect of a generation is
//! that of the loaded program
//! ([`Resolver::new`](crate::resolution::Resolver::new) sets it on the
//! thread): if it contains `package:ngx_dart/…`, it is 9.
use std::borrow::Cow;
use std::cell::Cell;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Dialect {
    /// `ngdart` 8.0.0-dev.4 with `dart:html`.
    #[default]
    Ngdart,
    /// `ngx_dart` 9.0.0-dev.2 with `package:web`.
    Ngx,
}

thread_local! {
    static CURRENT: Cell<Dialect> = const { Cell::new(Dialect::Ngdart) };
}

/// The dialect of the generation in progress on this thread.
pub fn current() -> Dialect {
    CURRENT.with(Cell::get)
}

/// Sets the dialect of this thread (the resolver does it when created).
pub fn set_dialect(d: Dialect) {
    CURRENT.with(|a| a.set(d));
}

/// The dialect of a program: that of `ngx_dart` if it loads any library of that
/// package.
pub fn from_program(p: &dartforge_elements::model::Program) -> Dialect {
    if p.libraries
        .iter()
        .any(|l| l.uri.starts_with("package:ngx_dart/"))
    {
        Dialect::Ngx
    } else {
        Dialect::Ngdart
    }
}

/// The packages renamed by the fork: (ngdart 8, ngx 9).
const PARES: &[(&str, &str)] = &[
    ("package:ngdart/", "package:ngx_dart/"),
    ("package:ngforms/", "package:ngx_forms/"),
    ("package:ngrouter/", "package:ngx_router/"),
    ("package:ngtest/", "package:ngx_test/"),
];

/// The library URI in the `ngdart` 8 name (`package:ngx_dart/x.dart` →
/// `package:ngdart/x.dart`); the others stay.
pub fn canonical(uri: &str) -> Cow<'_, str> {
    for (ng, ngx) in PARES {
        if let Some(rest) = uri.strip_prefix(ngx) {
            return Cow::Owned(format!("{ng}{rest}"));
        }
    }
    Cow::Borrowed(uri)
}

/// [`canonical`] taking ownership.
pub fn canonical_string(uri: String) -> String {
    match canonical(&uri) {
        Cow::Borrowed(_) => uri,
        Cow::Owned(u) => u,
    }
}

/// The canonical URI as it is in the program and goes out in the generated
/// file: in dialect 9, `package:ngdart/x.dart` → `package:ngx_dart/x.dart`.
pub fn writing(uri: &str) -> Cow<'_, str> {
    if current() == Dialect::Ngx {
        for (ng, ngx) in PARES {
            if let Some(rest) = uri.strip_prefix(ng) {
                return Cow::Owned(format!("{ngx}{rest}"));
            }
        }
    }
    Cow::Borrowed(uri)
}

/// The two names of the same package (`ngx_forms` and `ngforms`) count as the
/// same package when computing the relative import (`getImportModulePath`).
pub fn canonical_package(package: &str) -> &str {
    match package {
        "ngx_dart" => "ngdart",
        "ngx_forms" => "ngforms",
        "ngx_router" => "ngrouter",
        "ngx_test" => "ngtest",
        p => p,
    }
}

/// Built-in element providers in addition to those of `ngdart` 8: the
/// `JSObject` that `ngx_compiler` puts right after `ElementRef`
/// (`NX:compiler/view_compiler/compile_element.dart:116`). Every element
/// provider `uniqueId` goes up by this much.
pub fn extra_builtins() -> u32 {
    match current() {
        Dialect::Ngdart => 0,
        Dialect::Ngx => 1,
    }
}

/// The compiler's DOM identifiers (`Identifiers` in
/// `compiler/identifiers.dart`), by their `dart:html` name: the module and the
/// name in the generation's dialect. In 9 each one has its own `package:web`
/// module (`NX:compiler/identifiers.dart:295-389`).
pub fn dom(name: &str) -> (&'static str, Cow<'static, str>) {
    match current() {
        Dialect::Ngdart => {
            let uri = if matches!(name, "SvgSvgElement" | "SvgElement") {
                "dart:svg"
            } else {
                "dart:html"
            };
            (uri, Cow::Owned(name.to_string()))
        }
        Dialect::Ngx => dom_web(name),
    }
}

const WEB_DOM: &str = "package:web/src/dom/dom.dart";
const WEB_HTML: &str = "package:web/src/dom/html.dart";
const WEB_SVG: &str = "package:web/src/dom/svg.dart";
const WEB_UIEVENTS: &str = "package:web/src/dom/uievents.dart";

fn dom_web(name: &str) -> (&'static str, Cow<'static, str>) {
    let entry = |uri: &'static str, n: &'static str| (uri, Cow::Borrowed(n));
    match name {
        "document" => entry(WEB_DOM, "document"),
        "Element" => entry(WEB_DOM, "Element"),
        "Node" => entry(WEB_DOM, "Node"),
        "Text" => entry(WEB_DOM, "Text"),
        "Comment" => entry(WEB_DOM, "Comment"),
        "DocumentFragment" => entry(WEB_DOM, "DocumentFragment"),
        "Event" => entry(WEB_DOM, "Event"),
        "HtmlElement" => entry(WEB_HTML, "HTMLElement"),
        "SvgSvgElement" => entry(WEB_SVG, "SVGSVGElement"),
        "SvgElement" => entry(WEB_SVG, "SVGElement"),
        // The typed elements of `_tagNameToIdentifier`: `HTML` + the name.
        "AnchorElement" | "AreaElement" | "AudioElement" | "ButtonElement" | "CanvasElement"
        | "DivElement" | "FormElement" | "IFrameElement" | "ImageElement" | "InputElement"
        | "TextAreaElement" | "MediaElement" | "MenuElement" | "OptionElement" | "OListElement"
        | "SelectElement" | "TableElement" | "TableRowElement" | "TableColElement"
        | "UListElement" => (WEB_HTML, Cow::Owned(format!("HTML{name}"))),
        // The new events (`html_events.dart`), each in its module.
        "MouseEvent" | "KeyboardEvent" | "FocusEvent" | "InputEvent" | "CompositionEvent"
        | "WheelEvent" => (WEB_UIEVENTS, Cow::Owned(name.to_string())),
        "DragEvent" => (WEB_HTML, Cow::Borrowed("DragEvent")),
        "PointerEvent" => entry("package:web/src/dom/pointerevents.dart", "PointerEvent"),
        "TouchEvent" => entry("package:web/src/dom/touch_events.dart", "TouchEvent"),
        "ClipboardEvent" => entry("package:web/src/dom/clipboard_apis.dart", "ClipboardEvent"),
        "AnimationEvent" => entry("package:web/src/dom/css_animations.dart", "AnimationEvent"),
        "TransitionEvent" => entry(
            "package:web/src/dom/css_transitions.dart",
            "TransitionEvent",
        ),
        other => (WEB_DOM, Cow::Owned(other.to_string())),
    }
}

/// `nativeHtmlEventType` (`NX:compiler/html_events.dart:116-177`): the type (by
/// the [`dom`] name) of the native event `name`; outside the table, `Event`.
pub fn native_event_type(name: &str) -> &'static str {
    match name {
        "animationend" | "animationiteration" | "animationstart" => "AnimationEvent",
        "blur" | "focus" | "focusin" | "focusout" => "FocusEvent",
        "click" | "contextmenu" | "dblclick" | "mousedown" | "mouseenter" | "mouseleave"
        | "mousemove" | "mouseout" | "mouseover" | "mouseup" => "MouseEvent",
        "compositionend" | "compositionstart" | "compositionupdate" => "CompositionEvent",
        "copy" | "cut" | "paste" => "ClipboardEvent",
        "drag" | "dragend" | "dragenter" | "dragleave" | "dragover" | "dragstart" | "drop" => {
            "DragEvent"
        }
        "gotpointercapture" | "lostpointercapture" | "pointercancel" | "pointerdown"
        | "pointerenter" | "pointerleave" | "pointermove" | "pointerout" | "pointerover"
        | "pointerup" => "PointerEvent",
        "input" => "InputEvent",
        "keydown" | "keypress" | "keyup" => "KeyboardEvent",
        "touchcancel" | "touchend" | "touchmove" | "touchstart" => "TouchEvent",
        "transitionend" => "TransitionEvent",
        "wheel" => "WheelEvent",
        _ => "Event",
    }
}

/// Whether the class `(uri, name)` of the program is the element's built-in
/// node — `Element` or `HtmlElement` of `dart:html` in 8, `Element`
/// (`dom.dart`) or `HTMLElement` (`html.dart`) of `package:web` in 9 —, and
/// which of the two (by the name in 8).
pub fn embedded_node(uri: &str, name: &str) -> Option<&'static str> {
    match (uri, name) {
        ("dart:html", "Element") | (WEB_DOM, "Element") => Some("Element"),
        ("dart:html", "HtmlElement") | (WEB_HTML, "HTMLElement") => Some("HtmlElement"),
        _ => None,
    }
}

/// Whether the type `(uri, name)` is the DOM `Element` or a subtype of it (the
/// `isElementType` of queries). In 8, every `…Element` of `dart:html` (except
/// `NoncedElement`) and of `dart:svg`; in 9, every `…Element` declared in
/// `package:web/src/dom/` (the *extension types* that `implements Element`).
pub fn is_element(uri: &str, name: &str) -> bool {
    match uri {
        "dart:html" => name.ends_with("Element") && name != "NoncedElement",
        "dart:svg" => name.ends_with("Element"),
        u if u.starts_with("package:web/src/dom/") => name.ends_with("Element"),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packages_in_both_directions() {
        assert_eq!(
            canonical("package:ngx_dart/src/utilities.dart"),
            "package:ngdart/src/utilities.dart"
        );
        assert_eq!(
            canonical("package:ngx_dartx/a.dart"),
            "package:ngx_dartx/a.dart"
        );
        set_dialect(Dialect::Ngx);
        assert_eq!(
            writing("package:ngforms/ngforms.dart"),
            "package:ngx_forms/ngforms.dart"
        );
        assert_eq!(dom("DivElement").1, "HTMLDivElement");
        assert_eq!(extra_builtins(), 1);
        set_dialect(Dialect::Ngdart);
        assert_eq!(
            writing("package:ngforms/ngforms.dart"),
            "package:ngforms/ngforms.dart"
        );
        assert_eq!(
            dom("DivElement"),
            ("dart:html", Cow::Owned("DivElement".into()))
        );
    }
}

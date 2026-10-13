//! The dialect of the official compiler that the generated file imitates.
//!
//! `ngdart` 8.0.0-dev.4 (`ngcompiler` 3.0.0-dev.3) and the `ngx_dart` fork
//! 9.0.0-dev.2 (`ngx_compiler` 9.0.0-dev.2), which replaces `dart:html` with
//! `package:web`, have the same compiler except for a short list of differences
//! (`docs/NGDART-COMPILADOR-DE-VISOES.md` part B): the package names, the DOM
//! identifiers, the built-in `JSObject` provider and the typed events.
//!
//! Internally the port always speaks `ngdart` 8: every URI read from the
//! program goes through [`canonica`] (`package:ngx_dart/…` becomes
//! `package:ngdart/…`), and the output goes back to the generation's dialect
//! through [`escrita`] (in the import table). The dialect of a generation is
//! that of the loaded program
//! ([`Resolvedor::novo`](crate::resolucao::Resolvedor::novo) sets it on the
//! thread): if it contains `package:ngx_dart/…`, it is 9.
use std::borrow::Cow;
use std::cell::Cell;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Dialeto {
    /// `ngdart` 8.0.0-dev.4 com `dart:html`.
    #[default]
    Ngdart,
    /// `ngx_dart` 9.0.0-dev.2 com `package:web`.
    Ngx,
}

thread_local! {
    static ATUAL: Cell<Dialeto> = const { Cell::new(Dialeto::Ngdart) };
}

/// The dialect of the generation in progress on this thread.
pub fn atual() -> Dialeto {
    ATUAL.with(Cell::get)
}

/// Fixa o dialeto desta thread (o resolvedor o faz ao ser criado).
pub fn fixar(d: Dialeto) {
    ATUAL.with(|a| a.set(d));
}

/// The dialect of a program: that of `ngx_dart` if it loads any library of that
/// package.
pub fn do_programa(p: &dartforge_elements::model::Program) -> Dialeto {
    if p
        .libraries
        .iter()
        .any(|l| l.uri.starts_with("package:ngx_dart/"))
    {
        Dialeto::Ngx
    } else {
        Dialeto::Ngdart
    }
}

/// The packages renamed by the fork: (ngdart 8, ngx 9).
const PARES: &[(&str, &str)] = &[
    ("package:ngdart/", "package:ngx_dart/"),
    ("package:ngforms/", "package:ngx_forms/"),
    ("package:ngrouter/", "package:ngx_router/"),
    ("package:ngtest/", "package:ngx_test/"),
];

/// A URI de biblioteca no nome do `ngdart` 8 (`package:ngx_dart/x.dart` →
/// `package:ngdart/x.dart`); as outras ficam.
pub fn canonica(uri: &str) -> Cow<'_, str> {
    for (ng, ngx) in PARES {
        if let Some(resto) = uri.strip_prefix(ngx) {
            return Cow::Owned(format!("{ng}{resto}"));
        }
    }
    Cow::Borrowed(uri)
}

/// [`canonica`] taking ownership.
pub fn canonica_string(uri: String) -> String {
    match canonica(&uri) {
        Cow::Borrowed(_) => uri,
        Cow::Owned(u) => u,
    }
}

/// The canonical URI as it is in the program and goes out in the generated
/// file: in dialect 9, `package:ngdart/x.dart` → `package:ngx_dart/x.dart`.
pub fn escrita(uri: &str) -> Cow<'_, str> {
    if atual() == Dialeto::Ngx {
        for (ng, ngx) in PARES {
            if let Some(resto) = uri.strip_prefix(ng) {
                return Cow::Owned(format!("{ngx}{resto}"));
            }
        }
    }
    Cow::Borrowed(uri)
}

/// The two names of the same package (`ngx_forms` and `ngforms`) count as the
/// same package when computing the relative import (`getImportModulePath`).
pub fn pacote_canonico(pacote: &str) -> &str {
    match pacote {
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
pub fn embutidos_extra() -> u32 {
    match atual() {
        Dialeto::Ngdart => 0,
        Dialeto::Ngx => 1,
    }
}

/// The compiler's DOM identifiers (`Identifiers` in
/// `compiler/identifiers.dart`), by their `dart:html` name: the module and the
/// name in the generation's dialect. In 9 each one has its own `package:web`
/// module (`NX:compiler/identifiers.dart:295-389`).
pub fn dom(nome: &str) -> (&'static str, Cow<'static, str>) {
    match atual() {
        Dialeto::Ngdart => {
            let uri = if matches!(nome, "SvgSvgElement" | "SvgElement") {
                "dart:svg"
            } else {
                "dart:html"
            };
            (uri, Cow::Owned(nome.to_string()))
        }
        Dialeto::Ngx => dom_web(nome),
    }
}

const WEB_DOM: &str = "package:web/src/dom/dom.dart";
const WEB_HTML: &str = "package:web/src/dom/html.dart";
const WEB_SVG: &str = "package:web/src/dom/svg.dart";
const WEB_UIEVENTS: &str = "package:web/src/dom/uievents.dart";

fn dom_web(nome: &str) -> (&'static str, Cow<'static, str>) {
    let de = |uri: &'static str, n: &'static str| (uri, Cow::Borrowed(n));
    match nome {
        "document" => de(WEB_DOM, "document"),
        "Element" => de(WEB_DOM, "Element"),
        "Node" => de(WEB_DOM, "Node"),
        "Text" => de(WEB_DOM, "Text"),
        "Comment" => de(WEB_DOM, "Comment"),
        "DocumentFragment" => de(WEB_DOM, "DocumentFragment"),
        "Event" => de(WEB_DOM, "Event"),
        "HtmlElement" => de(WEB_HTML, "HTMLElement"),
        "SvgSvgElement" => de(WEB_SVG, "SVGSVGElement"),
        "SvgElement" => de(WEB_SVG, "SVGElement"),
        // The typed elements of `_tagNameToIdentifier`: `HTML` + the name.
        "AnchorElement" | "AreaElement" | "AudioElement" | "ButtonElement" | "CanvasElement"
        | "DivElement" | "FormElement" | "IFrameElement" | "ImageElement" | "InputElement"
        | "TextAreaElement" | "MediaElement" | "MenuElement" | "OptionElement"
        | "OListElement" | "SelectElement" | "TableElement" | "TableRowElement"
        | "TableColElement" | "UListElement" => (WEB_HTML, Cow::Owned(format!("HTML{nome}"))),
        // The new events (`html_events.dart`), each in its module.
        "MouseEvent" | "KeyboardEvent" | "FocusEvent" | "InputEvent" | "CompositionEvent"
        | "WheelEvent" => (WEB_UIEVENTS, Cow::Owned(nome.to_string())),
        "DragEvent" => (WEB_HTML, Cow::Borrowed("DragEvent")),
        "PointerEvent" => de("package:web/src/dom/pointerevents.dart", "PointerEvent"),
        "TouchEvent" => de("package:web/src/dom/touch_events.dart", "TouchEvent"),
        "ClipboardEvent" => de("package:web/src/dom/clipboard_apis.dart", "ClipboardEvent"),
        "AnimationEvent" => de("package:web/src/dom/css_animations.dart", "AnimationEvent"),
        "TransitionEvent" => de("package:web/src/dom/css_transitions.dart", "TransitionEvent"),
        outro => (WEB_DOM, Cow::Owned(outro.to_string())),
    }
}

/// `nativeHtmlEventType` (`NX:compiler/html_events.dart:116-177`): the type (by
/// the [`dom`] name) of the native event `nome`; outside the table, `Event`.
pub fn tipo_do_evento_nativo(nome: &str) -> &'static str {
    match nome {
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

/// Whether the class `(uri, nome)` of the program is the element's built-in
/// node — `Element` or `HtmlElement` of `dart:html` in 8, `Element`
/// (`dom.dart`) or `HTMLElement` (`html.dart`) of `package:web` in 9 —, and
/// which of the two (by the name in 8).
pub fn no_embutido(uri: &str, nome: &str) -> Option<&'static str> {
    match (uri, nome) {
        ("dart:html", "Element") | (WEB_DOM, "Element") => Some("Element"),
        ("dart:html", "HtmlElement") | (WEB_HTML, "HTMLElement") => Some("HtmlElement"),
        _ => None,
    }
}

/// Whether the type `(uri, nome)` is the DOM `Element` or a subtype of it (the
/// `isElementType` of queries). In 8, every `…Element` of `dart:html` (except
/// `NoncedElement`) and of `dart:svg`; in 9, every `…Element` declared in
/// `package:web/src/dom/` (the *extension types* that `implements Element`).
pub fn e_elemento(uri: &str, nome: &str) -> bool {
    match uri {
        "dart:html" => nome.ends_with("Element") && nome != "NoncedElement",
        "dart:svg" => nome.ends_with("Element"),
        u if u.starts_with("package:web/src/dom/") => nome.ends_with("Element"),
        _ => false,
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn pacotes_nos_dois_sentidos() {
        assert_eq!(
            canonica("package:ngx_dart/src/utilities.dart"),
            "package:ngdart/src/utilities.dart"
        );
        assert_eq!(canonica("package:ngx_dartx/a.dart"), "package:ngx_dartx/a.dart");
        fixar(Dialeto::Ngx);
        assert_eq!(
            escrita("package:ngforms/ngforms.dart"),
            "package:ngx_forms/ngforms.dart"
        );
        assert_eq!(dom("DivElement").1, "HTMLDivElement");
        assert_eq!(embutidos_extra(), 1);
        fixar(Dialeto::Ngdart);
        assert_eq!(escrita("package:ngforms/ngforms.dart"), "package:ngforms/ngforms.dart");
        assert_eq!(dom("DivElement"), ("dart:html", Cow::Owned("DivElement".into())));
    }
}

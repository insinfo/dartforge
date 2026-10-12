//! O dialeto do compilador oficial que o arquivo gerado imita.
//!
//! O `ngdart` 8.0.0-dev.4 (`ngcompiler` 3.0.0-dev.3) e o fork `ngx_dart`
//! 9.0.0-dev.2 (`ngx_compiler` 9.0.0-dev.2), que troca `dart:html` por
//! `package:web`, têm o mesmo compilador a menos de uma lista curta de
//! diferenças (`docs/NGDART-COMPILADOR-DE-VISOES.md` parte B): os nomes dos
//! pacotes, os identificadores de DOM, o provedor embutido `JSObject` e os
//! eventos tipados.
//!
//! Por dentro o porte fala sempre o `ngdart` 8: toda URI lida do programa
//! passa por [`canonica`] (`package:ngx_dart/…` vira `package:ngdart/…`), e a
//! saída volta ao dialeto da geração por [`escrita`] (na tabela de imports).
//! O dialeto de uma geração é o do programa carregado
//! ([`Resolvedor::novo`](crate::resolucao::Resolvedor::novo) o fixa na
//! thread): há `package:ngx_dart/…` nele, é o 9.
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

/// O dialeto da geração em curso nesta thread.
pub fn atual() -> Dialeto {
    ATUAL.with(Cell::get)
}

/// Fixa o dialeto desta thread (o resolvedor o faz ao ser criado).
pub fn fixar(d: Dialeto) {
    ATUAL.with(|a| a.set(d));
}

/// O dialeto de um programa: o do `ngx_dart` se ele carrega alguma
/// biblioteca desse pacote.
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

/// Os pacotes renomeados pelo fork: (ngdart 8, ngx 9).
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

/// [`canonica`] já com a posse.
pub fn canonica_string(uri: String) -> String {
    match canonica(&uri) {
        Cow::Borrowed(_) => uri,
        Cow::Owned(u) => u,
    }
}

/// A URI canônica como ela está no programa e sai no arquivo gerado: no
/// dialeto 9, `package:ngdart/x.dart` → `package:ngx_dart/x.dart`.
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

/// Os dois nomes do mesmo pacote (`ngx_forms` e `ngforms`) contam como o
/// mesmo pacote no cálculo do import relativo (`getImportModulePath`).
pub fn pacote_canonico(pacote: &str) -> &str {
    match pacote {
        "ngx_dart" => "ngdart",
        "ngx_forms" => "ngforms",
        "ngx_router" => "ngrouter",
        "ngx_test" => "ngtest",
        p => p,
    }
}

/// Provedores embutidos do elemento a mais que no `ngdart` 8: o `JSObject`
/// que o `ngx_compiler` põe logo depois do `ElementRef`
/// (`NX:compiler/view_compiler/compile_element.dart:116`). Todo `uniqueId`
/// de provedor de elemento sobe esse tanto.
pub fn embutidos_extra() -> u32 {
    match atual() {
        Dialeto::Ngdart => 0,
        Dialeto::Ngx => 1,
    }
}

/// Os identificadores de DOM do compilador (`Identifiers` em
/// `compiler/identifiers.dart`), pelo nome do `dart:html`: o módulo e o nome
/// no dialeto da geração. No 9 cada um tem o seu módulo do `package:web`
/// (`NX:compiler/identifiers.dart:295-389`).
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
        // Os elementos tipados de `_tagNameToIdentifier`: `HTML` + o nome.
        "AnchorElement" | "AreaElement" | "AudioElement" | "ButtonElement" | "CanvasElement"
        | "DivElement" | "FormElement" | "IFrameElement" | "ImageElement" | "InputElement"
        | "TextAreaElement" | "MediaElement" | "MenuElement" | "OptionElement"
        | "OListElement" | "SelectElement" | "TableElement" | "TableRowElement"
        | "TableColElement" | "UListElement" => (WEB_HTML, Cow::Owned(format!("HTML{nome}"))),
        // Os eventos novos (`html_events.dart`), no módulo de cada um.
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

/// `nativeHtmlEventType` (`NX:compiler/html_events.dart:116-177`): o tipo
/// (pelo nome de [`dom`]) do evento nativo `nome`; fora da tabela, `Event`.
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

/// A classe `(uri, nome)` do programa é o nó embutido do elemento — o
/// `Element` ou o `HtmlElement` do `dart:html` no 8, o `Element`
/// (`dom.dart`) ou o `HTMLElement` (`html.dart`) do `package:web` no 9 —, e
/// qual dos dois (pelo nome do 8).
pub fn no_embutido(uri: &str, nome: &str) -> Option<&'static str> {
    match (uri, nome) {
        ("dart:html", "Element") | (WEB_DOM, "Element") => Some("Element"),
        ("dart:html", "HtmlElement") | (WEB_HTML, "HTMLElement") => Some("HtmlElement"),
        _ => None,
    }
}

/// O tipo `(uri, nome)` é `Element` do DOM ou um subtipo dele (o
/// `isElementType` das consultas). No 8, todo `…Element` do `dart:html` (menos
/// o `NoncedElement`) e do `dart:svg`; no 9, todo `…Element` declarado em
/// `package:web/src/dom/` (os *extension types* que `implements Element`).
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

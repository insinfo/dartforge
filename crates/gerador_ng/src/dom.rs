//! Tabelas do DOM que o emissor oficial usa, copiadas do `ngcompiler`.
//!
//! `_htmlTagNames` e `_tagNameToIdentifier` de
//! `view_compiler/view_compiler_utils.dart`: decidem o tipo com que o
//! elemento é criado e, por tabela batida, se um nome de tag é HTML ou é um
//! componente/diretiva da aplicação.

/// Nomes de tag que o ngcompiler reconhece como HTML (`_htmlTagNames`).
pub const TAGS_HTML: &[&str] = &[
    "a", "abbr", "acronym", "address", "applet", "area", "article", "aside", "audio", "b", "base",
    "basefont", "bdi", "bdo", "bgsound", "big", "blockquote", "body", "br", "button", "canvas",
    "caption", "center", "cite", "code", "col", "colgroup", "command", "data", "datalist", "dd",
    "del", "details", "dfn", "dialog", "dir", "div", "dl", "dt", "element", "em", "embed",
    "fieldset", "figcaption", "figure", "font", "footer", "form", "h1", "h2", "h3", "h4", "h5",
    "h6", "head", "header", "hr", "i", "iframe", "img", "input", "ins", "kbd", "keygen", "label",
    "legend", "li", "link", "listing", "main", "map", "mark", "menu", "menuitem", "meta", "meter",
    "nav", "object", "ol", "optgroup", "option", "output", "p", "param", "picture", "pre",
    "progress", "q", "rp", "rt", "rtc", "ruby", "s", "samp", "script", "section", "select",
    "shadow", "small", "source", "span", "strong", "style", "sub", "summary", "sup", "table",
    "tbody", "td", "template", "textarea", "tfoot", "th", "thead", "time", "title", "tr", "track",
    "tt", "u", "ul", "var", "video", "wbr",
];

/// `_tagNameToIdentifier`: a classe de `dart:html` com que o elemento é
/// tipado. Fora desta tabela, tag HTML vira `HtmlElement` e o resto,
/// `Element`.
const TIPADOS: &[(&str, &str)] = &[
    ("a", "AnchorElement"),
    ("area", "AreaElement"),
    ("audio", "AudioElement"),
    ("button", "ButtonElement"),
    ("canvas", "CanvasElement"),
    ("div", "DivElement"),
    ("form", "FormElement"),
    ("iframe", "IFrameElement"),
    ("input", "InputElement"),
    ("image", "ImageElement"),
    ("media", "MediaElement"),
    ("menu", "MenuElement"),
    ("ol", "OListElement"),
    ("option", "OptionElement"),
    ("col", "TableColElement"),
    ("row", "TableRowElement"),
    ("select", "SelectElement"),
    ("table", "TableElement"),
    ("textarea", "TextAreaElement"),
    ("ul", "UListElement"),
    ("svg", "SvgSvgElement"),
];

/// É um nome de tag do HTML (e não de um componente da aplicação)?
pub fn tag_html(nome: &str) -> bool {
    TAGS_HTML.contains(&nome.to_ascii_lowercase().as_str())
}

/// Tipo com que o elemento é criado (`identifierFromTagName`).
pub fn tipo_da_tag(nome: &str) -> &'static str {
    let n = nome.to_ascii_lowercase();
    if let Some((_, t)) = TIPADOS.iter().find(|(tag, _)| *tag == n) {
        return t;
    }
    if tag_html(&n) { "HtmlElement" } else { "Element" }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn tipos_como_no_oficial() {
        // `appendElement<import8.HtmlElement>(doc, _el_0, 'img')` no
        // no_data_component gerado pelo compilador oficial: `img` não está na
        // tabela de tipados, mas é HTML.
        assert_eq!(tipo_da_tag("img"), "HtmlElement");
        assert_eq!(tipo_da_tag("div"), "DivElement");
        assert_eq!(tipo_da_tag("INPUT"), "InputElement");
        assert_eq!(tipo_da_tag("meu-componente"), "Element");
        assert!(!tag_html("meu-componente"));
        assert!(tag_html("section"));
    }
}

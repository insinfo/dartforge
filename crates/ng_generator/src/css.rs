//! The ngdart style shim: ngcompiler's `shimShadowCss`, with emulated
//! encapsulation.
//!
//! Without Shadow DOM, style isolation is done by attribute: each element of
//! the view gets `_ngcontent-<id>` and each selector of the stylesheet gets
//! `._ngcontent-%ID%` — `%ID%` is replaced by the component id at run time. The
//! `:host` selector becomes `._nghost-%ID%`.
//!
//! The work is done by the faithful port of the official code in
//! [`crate::shadow_css`] (which reads and prints with the csslib port,
//! [`crate::csslib`]): the `StyleCompiler` calls `shimShadowCss(style,
//! '_ngcontent-%ID%', '_nghost-%ID%')` (`style_compiler.dart`).
use crate::view::Reason;

/// `_viewClass` do `style_compiler.dart`.
const CONTENT: &str = "_ngcontent-%ID%";
/// `_hostClass` do `style_compiler.dart`.
const HOST: &str = "_nghost-%ID%";

/// Turns the stylesheet into the text that goes inside `styles` in
/// `<name>.css.shim.dart`. `Err` where the official compiler throws (and the
/// build fails); a parse error is not a failure — the official one warns and
/// generates anyway.
pub fn shim(css: &str) -> Result<String, Reason> {
    crate::shadow_css::shim_shadow_css(css, CONTENT, HOST).map_err(|_| Reason::Styles)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The exact output of the official compiler for the CSS of case b15.
    #[test]
    fn rich_sheet_equal_to_official() {
        let css = "/* um comentário */\n:host {\n  display: block;\n}\n\n.a .b {\n  color: red;\n}\n\n.c, .d {\n  margin: 0 auto;\n}\n\na:hover {\n  text-decoration: underline;\n}\n\n@media (max-width: 600px) {\n  .a {\n    display: none;\n  }\n}\n";
        let expected = "._nghost-%ID%{display:block}.a._ngcontent-%ID% .b._ngcontent-%ID%{color:red}.c._ngcontent-%ID%,.d._ngcontent-%ID%{margin:0 auto}a:hover._ngcontent-%ID%{text-decoration:underline}@media (max-width:600px){.a._ngcontent-%ID%{display:none}}";
        assert_eq!(shim(css).unwrap(), expected);
    }

    /// A do caso b07.
    #[test]
    fn simple_sheet_equal_to_official() {
        let css = ".c { color: red; }\nspan { font-weight: bold; }\n";
        assert_eq!(
            shim(css).unwrap(),
            ".c._ngcontent-%ID%{color:red}span._ngcontent-%ID%{font-weight:bold}"
        );
    }

    /// The shapes of case b16, one by one, against the official output.
    #[test]
    fn b16_shapes_equal_to_official() {
        let css = ".a::before {
  content: \"x\";
}

:host(.tema-escuro) .b {
  color: white;
}

:host-context(.pai) .c {
  color: red;
}

::ng-deep .d {
  color: blue;
}

.e > .f {
  margin: 0;
}

input[type=\"text\"] {
  border: 0;
}

@keyframes girar {
  from { opacity: 0; }
  to { opacity: 1; }
}

.g {
  animation: girar 1s;
}
";
        let expected = ".a._ngcontent-%ID%::before{content:\"x\"}._nghost-%ID%.tema-escuro .b._ngcontent-%ID%{color:white}._nghost-%ID%.pai .c._ngcontent-%ID%,.pai ._nghost-%ID% .c._ngcontent-%ID%{color:red} .d{color:blue}.e._ngcontent-%ID% > .f._ngcontent-%ID%{margin:0}input[type=\"text\"]._ngcontent-%ID%{border:0}@keyframes girar{from{opacity:0}to{opacity:1}}.g._ngcontent-%ID%{animation:girar 1s}";
        assert_eq!(shim(css).unwrap(), expected);
    }

    /// Shapes that csslib accepts and the official shim generates (outputs of
    /// the `shimShadowCss` of ngcompiler 3.0.0-dev.3): a nested rule stays
    /// nested, `@font-face` comes out as is, `#ffffff` becomes `white`, `10 px`
    /// becomes `10px`, the media operators come out in uppercase.
    #[test]
    fn csslib_shapes_equal_to_official() {
        let cases = [
            (
                ".a { .b { color: red; } }",
                ".a._ngcontent-%ID%{.b._ngcontent-%ID%{color:red}}",
            ),
            (
                "@font-face { font-family: x; }",
                "@font-face{font-family:x}",
            ),
            (
                ":host-context(.a) .b, ::ng-deep .c, :host(span.x) > .d::before{color:#ffffff;margin:10 px}",
                "._nghost-%ID%.a .b._ngcontent-%ID%,.a ._nghost-%ID% .b._ngcontent-%ID%, .c,span._nghost-%ID%.x > .d._ngcontent-%ID%::before{color:white;margin:10px}",
            ),
            (
                "@media only screen and (max-width:600px){.x{filter:alpha(opacity=50)}}\n@supports (display:grid){.y{--z:var(--w, #000)}}",
                "@media ONLY screen AND (max-width:600px){.x._ngcontent-%ID%{filter:alpha(opacity=50)}}@supports (display:grid){.y._ngcontent-%ID%{--z:var(--w, black)}}",
            ),
        ];
        for (css, expected) in cases {
            assert_eq!(shim(css).unwrap(), expected, "{css}");
        }
    }

    /// Where the official one throws (and the build fails), the shim refuses:
    /// `@keyframes` without a block (`processTerm() as Expression` on null) and
    /// the endless loop of `processMarginsDeclarations` in a `@page` with a
    /// selector.
    #[test]
    fn refuses_where_the_official_fails() {
        assert!(shim("@keyframes k{}").is_err());
        assert!(shim("@page{.x}").is_err());
        assert!(shim("@supports").is_err());
    }
}

//! O shim de estilo do ngdart: o `shimShadowCss` do ngcompiler, com o
//! encapsulamento emulado.
//!
//! Sem Shadow DOM, o isolamento de estilo é por atributo: cada elemento da
//! visão recebe `_ngcontent-<id>` e cada seletor da folha ganha
//! `._ngcontent-%ID%` — o `%ID%` é trocado pelo id do componente em tempo de
//! execução. O seletor `:host` vira `._nghost-%ID%`.
//!
//! O trabalho é do porte fiel do oficial em [`crate::shadow_css`] (que lê e
//! imprime com o porte do csslib, [`crate::csslib`]): o `StyleCompiler`
//! chama `shimShadowCss(style, '_ngcontent-%ID%', '_nghost-%ID%')`
//! (`style_compiler.dart`).
use crate::visao::Motivo;

/// `_viewClass` do `style_compiler.dart`.
const CONTEUDO: &str = "_ngcontent-%ID%";
/// `_hostClass` do `style_compiler.dart`.
const HOSPEDEIRO: &str = "_nghost-%ID%";

/// Transforma a folha no texto que vai dentro de `styles` no
/// `<nome>.css.shim.dart`. `Err` onde o compilador oficial lança (e a build
/// falha); erro de parse não é falha — o oficial avisa e gera assim mesmo.
pub fn shim(css: &str) -> Result<String, Motivo> {
    crate::shadow_css::shim_shadow_css(css, CONTEUDO, HOSPEDEIRO).map_err(|_| Motivo::Estilos)
}

#[cfg(test)]
mod testes {
    use super::*;

    /// A saída exata do compilador oficial para o CSS do caso b15.
    #[test]
    fn folha_rica_igual_ao_oficial() {
        let css = "/* um comentário */\n:host {\n  display: block;\n}\n\n.a .b {\n  color: red;\n}\n\n.c, .d {\n  margin: 0 auto;\n}\n\na:hover {\n  text-decoration: underline;\n}\n\n@media (max-width: 600px) {\n  .a {\n    display: none;\n  }\n}\n";
        let esperado = "._nghost-%ID%{display:block}.a._ngcontent-%ID% .b._ngcontent-%ID%{color:red}.c._ngcontent-%ID%,.d._ngcontent-%ID%{margin:0 auto}a:hover._ngcontent-%ID%{text-decoration:underline}@media (max-width:600px){.a._ngcontent-%ID%{display:none}}";
        assert_eq!(shim(css).unwrap(), esperado);
    }

    /// A do caso b07.
    #[test]
    fn folha_simples_igual_ao_oficial() {
        let css = ".c { color: red; }\nspan { font-weight: bold; }\n";
        assert_eq!(
            shim(css).unwrap(),
            ".c._ngcontent-%ID%{color:red}span._ngcontent-%ID%{font-weight:bold}"
        );
    }

    /// As formas do caso b16, uma a uma, contra a saída do oficial.
    #[test]
    fn formas_do_b16_iguais_ao_oficial() {
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
        let esperado = ".a._ngcontent-%ID%::before{content:\"x\"}._nghost-%ID%.tema-escuro .b._ngcontent-%ID%{color:white}._nghost-%ID%.pai .c._ngcontent-%ID%,.pai ._nghost-%ID% .c._ngcontent-%ID%{color:red} .d{color:blue}.e._ngcontent-%ID% > .f._ngcontent-%ID%{margin:0}input[type=\"text\"]._ngcontent-%ID%{border:0}@keyframes girar{from{opacity:0}to{opacity:1}}.g._ngcontent-%ID%{animation:girar 1s}";
        assert_eq!(shim(css).unwrap(), esperado);
    }

    /// Formas que o csslib aceita e o shim oficial gera (saídas do
    /// `shimShadowCss` do ngcompiler 3.0.0-dev.3): aninhamento de regra fica
    /// aninhado, `@font-face` sai como está, `#ffffff` vira `white`, `10 px`
    /// vira `10px`, os operadores de mídia saem em maiúsculas.
    #[test]
    fn formas_do_csslib_iguais_ao_oficial() {
        let casos = [
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
        for (css, esperado) in casos {
            assert_eq!(shim(css).unwrap(), esperado, "{css}");
        }
    }

    /// Onde o oficial lança (e a build falha), o shim recusa: `@keyframes`
    /// sem bloco (`processTerm() as Expression` em nulo) e o laço sem fim do
    /// `processMarginsDeclarations` num `@page` com seletor.
    #[test]
    fn recusa_onde_o_oficial_falha() {
        assert!(shim("@keyframes k{}").is_err());
        assert!(shim("@page{.x}").is_err());
        assert!(shim("@supports").is_err());
    }
}

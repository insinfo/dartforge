//! `this` repetido vira um local (`docs/JS-PRODUCAO-TAMANHO.md` §8.4).
//!
//! Os métodos no contrato do DDC citam `this` muitas vezes (`this.x`,
//! `this[s]`, `S.new.call(this, …)`), e um minificador não pode encurtar
//! `this`. O `dart2js` guarda o receptor num local de uma letra (`var s=this`)
//! nos métodos que o usam muito (`ssa/codegen.dart`, a variável `s` do
//! receptor). Aqui: numa função (não flecha) cujo corpo cita `this` `k` vezes
//! com `3k > 11` (cada citação passa a custar 1 byte em vez de 4, contra os
//! ~11 da declaração), o corpo ganha `let t$this = this;` no início e cada
//! `this` dele (inclusive dentro de flechas, que herdam o mesmo `this`) vira
//! `t$this`, que a troca de nomes encurta.
//!
//! Ficam como estão: o `constructor` de classe (antes do `super()` o `this`
//! não existe), corpos com diretivas, e o `this` de inicializadores de
//! campo de classe, que não é o da função envolvente.

use oxc_ast::ast::{Class, Function, MethodDefinition, MethodDefinitionKind, ThisExpression};
use oxc_ast_visit::{Visit, walk};
use oxc_syntax::scope::ScopeFlags;

struct Quadro {
    /// O corpo (`{` … `}`); `None` quando a função não pode receber o local.
    corpo: Option<(u32, u32)>,
    usos: Vec<(u32, u32)>,
}

struct Coleta {
    pilha: Vec<Quadro>,
    proximo_e_construtor: bool,
    trocas: Vec<(u32, u32, &'static str)>,
}

impl<'a> Visit<'a> for Coleta {
    fn visit_method_definition(&mut self, it: &MethodDefinition<'a>) {
        if it.kind == MethodDefinitionKind::Constructor {
            self.proximo_e_construtor = true;
        }
        walk::walk_method_definition(self, it);
        self.proximo_e_construtor = false;
    }
    fn visit_class(&mut self, it: &Class<'a>) {
        // Barreira: o `this` de um campo de classe não é o da função de fora.
        self.pilha.push(Quadro { corpo: None, usos: Vec::new() });
        walk::walk_class(self, it);
        self.pilha.pop();
    }
    fn visit_function(&mut self, it: &Function<'a>, flags: ScopeFlags) {
        let construtor = std::mem::take(&mut self.proximo_e_construtor);
        let corpo = it.body.as_ref().filter(|b| b.directives.is_empty() && !construtor).map(|b| (b.span.start, b.span.end));
        self.pilha.push(Quadro { corpo, usos: Vec::new() });
        walk::walk_function(self, it, flags);
        let q = self.pilha.pop().unwrap_or(Quadro { corpo: None, usos: Vec::new() });
        if let Some((ini, fim)) = q.corpo {
            // Só o `this` do corpo: o dos parâmetros (valor padrão) roda
            // antes da declaração.
            let usos: Vec<(u32, u32)> = q.usos.into_iter().filter(|(a, _)| *a > ini && *a < fim).collect();
            if 3 * usos.len() > 11 {
                self.trocas.push((ini + 1, ini + 1, "let t$this = this;"));
                for (a, b) in usos {
                    self.trocas.push((a, b, "t$this"));
                }
            }
        }
    }
    fn visit_this_expression(&mut self, it: &ThisExpression) {
        if let Some(q) = self.pilha.last_mut() {
            q.usos.push((it.span.start, it.span.end));
        }
    }
}

/// Reescreve o texto (`fonte`, já embrulhado na IIFE). Devolve o texto como
/// veio se ele não for lido.
pub fn aplicar(fonte: &str) -> String {
    let alocador = oxc_allocator::Allocator::default();
    let lido = oxc_parser::Parser::new(&alocador, fonte, oxc_span::SourceType::cjs()).parse();
    if !lido.diagnostics.is_empty() {
        return fonte.to_string();
    }
    let mut c = Coleta { pilha: Vec::new(), proximo_e_construtor: false, trocas: Vec::new() };
    c.visit_program(&lido.program);
    if c.trocas.is_empty() {
        return fonte.to_string();
    }
    c.trocas.sort_by_key(|t| (t.0, t.1));
    let mut out = String::with_capacity(fonte.len());
    let mut pos = 0usize;
    for (a, b, novo) in c.trocas {
        let (a, b) = (a as usize, b as usize);
        if a < pos {
            continue;
        }
        out.push_str(&fonte[pos..a]);
        out.push_str(novo);
        pos = b;
    }
    out.push_str(&fonte[pos..]);
    out
}

#[cfg(test)]
mod testes {
    use super::aplicar;

    #[test]
    fn quatro_citacoes_viram_local() {
        let s = aplicar("(function () { function f() { return this.a + this.b + this.c + (() => this.d)(); } })();");
        assert!(s.contains("let t$this = this;"), "{s}");
        assert_eq!(s.matches("this").count(), 1 + 5, "{s}");
        assert!(s.contains("t$this.d"), "{s}");
    }

    #[test]
    fn poucas_citacoes_construtor_e_campo_ficam() {
        let poucas = "(function () { function f() { return this.a + this.b + this.c; } })();";
        assert_eq!(aplicar(poucas), poucas);
        let construtor = "(function () { class A extends B { constructor() { super(); this.a = this.b = this.c = this.d = 1; } } })();";
        assert_eq!(aplicar(construtor), construtor);
        let campo = "(function () { class A { x = this.a + this.b + this.c + this.d; } })();";
        assert_eq!(aplicar(campo), campo);
    }
}

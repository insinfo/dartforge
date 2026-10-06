//! O alvo escrito de uma fábrica redirecionadora (`factory C() = D.nome;`).
//!
//! O parser lê o alvo como tipo e só separa o `.nome` final quando sobra um
//! ponto: `D.nome` chega como o tipo `[D, nome]`, que tanto pode ser o tipo
//! `nome` do prefixo de import `D` quanto a classe `D` e o construtor `nome`
//! (o `AstRewriter` do analyzer decide pelo escopo). Os dois não convivem no
//! mesmo escopo (`PREFIX_COLLIDES_WITH_TOP_LEVEL_MEMBER`), então a ordem das
//! tentativas não muda o resultado.

use dartforge_elements::model::{ClassId, Element, Program, UnitId};
use dartforge_frontend::ast;

/// A classe do alvo e o nome do construtor escrito (`None`: o sem nome).
pub fn classe_e_construtor(program: &Program, unit: UnitId, r: &ast::RedirectTarget) -> Option<(ClassId, Option<ast::Name>)> {
    let ast::TypeKind::Named { name, .. } = &program.unit(unit).ast.ty(r.ty).kind else { return None };
    let (b, construtor) = match (&name[..], r.constructor) {
        ([n], ctor) => (program.lookup_na_unidade(unit, n.sym), ctor),
        ([p, n], None) => match program.lookup_prefixed_na_unidade(unit, p.sym, n.sym) {
            Some(b) => (Some(b), None),
            None => (program.lookup_na_unidade(unit, p.sym), Some(*n)),
        },
        ([p, n], ctor) => (program.lookup_prefixed_na_unidade(unit, p.sym, n.sym), ctor),
        ([p, n, c], None) => (program.lookup_prefixed_na_unidade(unit, p.sym, n.sym), Some(*c)),
        _ => (None, None),
    };
    match b?.getter? {
        Element::Class(c) => Some((c, construtor)),
        _ => None,
    }
}

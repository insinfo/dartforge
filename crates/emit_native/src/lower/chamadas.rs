//! Chamadas: funções de topo, locais e estáticas do usuário, construtores
//! (`C(…)`, `C.nome(…)`) e métodos de instância pelo elemento resolvido.
//! O que é do SDK passa por `sdk_por_nome.rs` (congelado).

use super::fn_builder::FnBuilder;
use crate::hir::*;
use dartforge_frontend::ast::{self, ExprId, ExprKind};
use dartforge_types::resolved::{MemberRef, Resolved};

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// `f(…)`, `o.m(…)`, `o?.m(…)`, `C(…)`, `C.m(…)`.
    pub(super) fn lower_chamada(
        &mut self,
        ast: &ast::Ast,
        expr_id: ExprId,
        expr: &ast::Expr,
        target: &ExprId,
        arguments: &ast::Arguments,
    ) -> Operand {
        if let Some(op) = self.funcao_sdk_por_nome(ast, target, arguments) {
            return op;
        }

        if let ExprKind::Identifier(id) = &ast.expr(*target).kind {
            let resolvido = self.ctx.get_resolved(self.unit_id, *target).cloned();
            // Local ou parâmetro que guarda um valor função (função local,
            // closure, tear-off): chamada pela convenção uniforme (P1).
            if matches!(
                resolvido,
                None | Some(Resolved::Local(_)) | Some(Resolved::Parameter { .. })
            ) && let Some(local) = self.buscar_local(id.sym)
            {
                let f = self.ler_local(&local);
                let avaliados = self.avaliar_args(ast, &arguments.args);
                return self.chamar_valor_funcao(f, &avaliados);
            }
            // Sem resolução (corpo de closure): pelo escopo léxico.
            let resolvido = match resolvido {
                None => self.resolver_por_nome(id.sym),
                r => r,
            };
            // Função de topo, membro implícito (`m()` = `this.m()`) ou
            // estático: pelo elemento resolvido do alvo.
            if let Some(Resolved::ExtensionMember { member, .. }) = resolvido {
                let avaliados = self.avaliar_args(ast, &arguments.args);
                let this = self.this_param.clone().unwrap_or(Operand::Constant(Constant::Null));
                return self.chamar_extensao(this, member.0 as usize, &avaliados, expr.span);
            }
            // Método do SDK chamado sem receptor (no corpo de uma extensão
            // sobre um tipo do SDK): pelo nome, com `this`.
            if let Some(Resolved::Member { member: MemberRef::Function(f), .. }) = resolvido
                && !crate::lower::funcao_do_usuario(self.ctx, f.0 as usize)
                && let Some(this) = self.this_param.clone()
            {
                let nome = self.ctx.symbol_name(id.sym).to_string();
                return self.metodo_sdk_por_nome(ast, expr, target, target, &nome, this, arguments);
            }
            match resolvido {
                Some(Resolved::Element(dartforge_elements::model::Element::Function(f)))
                    if crate::lower::funcao_do_usuario(self.ctx, f.0 as usize)
                        && self.ctx.program.functions[f.0 as usize].variable.is_none() =>
                {
                    let fid = f.0 as usize;
                    let avaliados = self.avaliar_args(ast, &arguments.args);
                    if self.ctx.program.functions[fid].kind
                        == dartforge_elements::model::FunctionKind::Getter
                    {
                        let v = self.chamar_direto(fid, None, Vec::new());
                        return self.chamar_valor_funcao(v, &avaliados);
                    }
                    let args = self.casar_args(fid, &avaliados);
                    return self.chamar_direto(fid, None, args);
                }
                Some(Resolved::Element(dartforge_elements::model::Element::Function(f)))
                    if crate::lower::funcao_do_usuario(self.ctx, f.0 as usize) =>
                {
                    // Variável de topo de tipo função.
                    let vid = self.ctx.program.functions[f.0 as usize]
                        .variable
                        .expect("acessor");
                    let v = self.ler_global(vid, expr.span);
                    let avaliados = self.avaliar_args(ast, &arguments.args);
                    return self.chamar_valor_funcao(v, &avaliados);
                }
                Some(Resolved::Element(dartforge_elements::model::Element::Variable(vid)))
                    if self.ctx.biblioteca_compilada(self.ctx.program.variables[vid.0 as usize].library) =>
                {
                    let v = self.ler_global(vid, expr.span);
                    let avaliados = self.avaliar_args(ast, &arguments.args);
                    return self.chamar_valor_funcao(v, &avaliados);
                }
                Some(Resolved::Member {
                    member: MemberRef::Function(f),
                    ..
                }) if crate::lower::funcao_do_usuario(self.ctx, f.0 as usize)
                    && self.ctx.program.functions[f.0 as usize].variable.is_none()
                    && self.ctx.program.functions[f.0 as usize].kind
                        != dartforge_elements::model::FunctionKind::Getter =>
                {
                    let fid = f.0 as usize;
                    let avaliados = self.avaliar_args(ast, &arguments.args);
                    if self.ctx.program.functions[fid].static_ {
                        let args = self.casar_args(fid, &avaliados);
                        return self.chamar_direto(fid, None, args);
                    }
                    let Some(this) = self.this_param.clone() else {
                        return self.nao_suportado(
                            "método de instância fora de membro de instância",
                            expr.span,
                        );
                    };
                    return self.chamar_membro(this, fid, &avaliados, expr.span);
                }
                Some(Resolved::Element(dartforge_elements::model::Element::Class(c)))
                    if self.ctx.get_resolved(self.unit_id, expr_id).is_none() =>
                {
                    // `C(…)` sem resolução (operando de `throw`, corpo de
                    // closure): o construtor sem nome da classe.
                    let vazio = self.ctx.interner.lookup("");
                    let ctor = vazio.and_then(|v| self.ctx.program.classes[c.0 as usize].constructors.get(&v).copied());
                    if let Some(f) = ctor {
                        return self.instanciar(ast, f, &arguments.args, expr.span);
                    }
                }
                Some(Resolved::Member { member, class, .. })
                    if self.ctx.biblioteca_compilada(self.ctx.program.classes[class.0 as usize].library) =>
                {
                    // Campo ou getter de tipo função: lê e chama o valor.
                    let v = self.ler_membro_implicito(member, expr.span);
                    let avaliados = self.avaliar_args(ast, &arguments.args);
                    return self.chamar_valor_funcao(v, &avaliados);
                }
                _ => {}
            }
        }
        if let Some(op) = self.estatica_sdk_por_nome(ast, target, arguments) {
            return op;
        }

        // Instanciação sem `new`: `C(…)`, `C.nome(…)`.
        if let Some(Resolved::Constructor(fid)) =
            self.ctx.get_resolved(self.unit_id, expr_id).cloned()
        {
            return self.instanciar(ast, fid, &arguments.args, expr.span);
        }

        // Chamadas de método sobre objeto / coleção
        if let ExprKind::Property {
            target: inner_target,
            name: method_name,
            null_aware,
        } = &ast.expr(*target).kind
        {
            let m_name = self.ctx.symbol_name(method_name.sym);
            let resolved_alvo = self.ctx.get_resolved(self.unit_id, *target).cloned();

            // `prefixo.f(…)`: o elemento importado com prefixo.
            if let Some(el) = self.elemento_prefixado(ast, *inner_target, method_name.sym) {
                use dartforge_elements::model::Element;
                match el {
                    Element::Function(f)
                        if crate::lower::funcao_do_usuario(self.ctx, f.0 as usize)
                            && self.ctx.program.functions[f.0 as usize].variable.is_none()
                            && self.ctx.program.functions[f.0 as usize].kind
                                != dartforge_elements::model::FunctionKind::Getter =>
                    {
                        let fid = f.0 as usize;
                        let avaliados = self.avaliar_args(ast, &arguments.args);
                        let args = self.casar_args(fid, &avaliados);
                        return self.chamar_direto(fid, None, args);
                    }
                    Element::Class(c) if self.ctx.biblioteca_compilada(self.ctx.program.classes[c.0 as usize].library) => {
                        let vazio = self.ctx.interner.lookup("");
                        if let Some(f) = vazio.and_then(|v| self.ctx.program.classes[c.0 as usize].constructors.get(&v).copied()) {
                            return self.instanciar(ast, f, &arguments.args, expr.span);
                        }
                    }
                    outro => {
                        let v = self.ler_elemento(outro, expr.span);
                        let avaliados = self.avaliar_args(ast, &arguments.args);
                        return self.chamar_valor_funcao(v, &avaliados);
                    }
                }
            }

            // `C.m(…)`: método estático do usuário.
            // `C.m(…)` (e `Alias.m(…)`, com `typedef Alias = C`).
            let alvo_e_classe = matches!(
                self.ctx.get_resolved(self.unit_id, *inner_target),
                Some(Resolved::Element(
                    dartforge_elements::model::Element::Class(_) | dartforge_elements::model::Element::Typedef(_)
                ))
            );
            if alvo_e_classe {
                if let Some(Resolved::Member {
                    member: MemberRef::Function(f),
                    ..
                }) = resolved_alvo
                {
                    let fid = f.0 as usize;
                    if crate::lower::funcao_do_usuario(self.ctx, fid)
                        && self.ctx.program.functions[fid].static_
                    {
                        let avaliados = self.avaliar_args(ast, &arguments.args);
                        let args = self.casar_args(fid, &avaliados);
                        return self.chamar_direto(fid, None, args);
                    }
                }
                return self.nao_suportado(&format!("chamada estática `{m_name}`"), expr.span);
            }
            // `C.nome(…)` sem resolução (corpo de closure, operando de
            // `throw`): construtor nomeado ou método estático pelo nome.
            if let ExprKind::Identifier(cn) = &ast.expr(*inner_target).kind
                && self.ctx.get_resolved(self.unit_id, *inner_target).is_none()
                && self.buscar_local(cn.sym).is_none()
                && let Some(Resolved::Element(dartforge_elements::model::Element::Class(c))) =
                    self.resolver_por_nome(cn.sym)
                && self.ctx.biblioteca_compilada(self.ctx.program.classes[c.0 as usize].library)
            {
                let classe = &self.ctx.program.classes[c.0 as usize];
                if let Some(&f) = classe.constructors.get(&method_name.sym) {
                    return self.instanciar(ast, f, &arguments.args, expr.span);
                }
                if let Some(&f) = classe.static_members.get(&method_name.sym) {
                    let fid = f.0 as usize;
                    let avaliados = self.avaliar_args(ast, &arguments.args);
                    let args = self.casar_args(fid, &avaliados);
                    return self.chamar_direto(fid, None, args);
                }
            }

            // `super.m(…)`: chamada direta ao membro da superclasse (P4).
            if matches!(ast.expr(*inner_target).kind, ExprKind::Super) {
                let avaliados = self.avaliar_args(ast, &arguments.args);
                return self.chamar_super_metodo(method_name.sym, &avaliados, expr.span);
            }

            let recv_op = self.lower_alvo(ast, *inner_target);
            if *null_aware {
                self.desviar_se_nulo(&recv_op);
            }

            // Membro de extensão (P4): chamada direta com o receptor.
            if let Some(Resolved::ExtensionMember { member, .. }) = resolved_alvo
                && crate::lower::funcao_do_usuario(self.ctx, member.0 as usize)
            {
                let avaliados = self.avaliar_args(ast, &arguments.args);
                return self.chamar_extensao(recv_op, member.0 as usize, &avaliados, expr.span);
            }

            // Método de classe do usuário: pelo elemento resolvido, com
            // despacho pela classe dinâmica (R7).
            if let Some((_, member)) =
                self.membro_do_usuario(*target, *inner_target, method_name.sym, false)
            {
                {
                    let campo = match member {
                        MemberRef::Variable(v) => Some(v),
                        MemberRef::Function(f) => self.ctx.program.functions[f.0 as usize].variable,
                    };
                    let getter = matches!(member, MemberRef::Function(f)
                        if self.ctx.program.functions[f.0 as usize].kind
                            == dartforge_elements::model::FunctionKind::Getter);
                    if let Some(vid) = campo {
                        // Campo de tipo função: lê e chama o valor (P1).
                        let v = if crate::lower::e_global(self.ctx, vid) {
                            self.ler_global(vid, expr.span)
                        } else {
                            self.ler_campo_com_late(recv_op, vid, expr.span)
                        };
                        let avaliados = self.avaliar_args(ast, &arguments.args);
                        return self.chamar_valor_funcao(v, &avaliados);
                    }
                    let MemberRef::Function(f) = member else {
                        unreachable!("campo tratado acima")
                    };
                    if getter {
                        let v = self.chamar_membro(recv_op, f.0 as usize, &[], expr.span);
                        let avaliados = self.avaliar_args(ast, &arguments.args);
                        return self.chamar_valor_funcao(v, &avaliados);
                    }
                    let avaliados = self.avaliar_args(ast, &arguments.args);
                    return self.chamar_membro(recv_op, f.0 as usize, &avaliados, expr.span);
                }
            }

            // Campo de record (com forma) de tipo função: `r.f(args)`.
            if !self.formas_com_campo(m_name).is_empty() {
                let nome = m_name.to_string();
                let r2 = recv_op.clone();
                return self.chamar_campo_de_registro(
                    recv_op,
                    m_name,
                    &mut |s: &mut Self| s.avaliar_args(ast, &arguments.args),
                    &mut |s: &mut Self| {
                        s.chamada_sem_membro(ast, expr, target, inner_target, &nome, r2.clone(), arguments)
                    },
                );
            }
            return self.chamada_sem_membro(ast, expr, target, inner_target, m_name, recv_op, arguments);
        }

        // Qualquer outra expressão como alvo (`f()()`, m['k']!(x),
        // (closure)(x)): o valor dela é chamado (P1).
        if !matches!(ast.expr(*target).kind, ExprKind::Identifier(_)) {
            let f = self.lower_expr(ast, *target);
            let avaliados = self.avaliar_args(ast, &arguments.args);
            return self.chamar_valor_funcao(f, &avaliados);
        }
        self.chamada_nao_suportada(ast, target, expr)
    }

    /// A expressão tem tipo função (ou Function), ou é um tear-off.
    fn e_valor_funcao(&self, e: ExprId) -> bool {
        let Some(t) = self.ctx.get_type(self.unit_id, e) else {
            return false;
        };
        match self.ctx.table.get(t) {
            dartforge_types::table::Type::Function { .. } => true,
            dartforge_types::table::Type::Interface { class, .. } => {
                self.ctx.symbol_name(self.ctx.program.classes[class.0 as usize].name) == "Function"
            }
            _ => t == self.ctx.core.dynamic_,
        }
    }

    /// Diagnóstico de chamada que nenhum caminho baixou.
    pub(super) fn chamada_nao_suportada(
        &mut self,
        ast: &ast::Ast,
        target: &ExprId,
        expr: &ast::Expr,
    ) -> Operand {
        let oque = match &ast.expr(*target).kind {
            ExprKind::Property { name, .. } | ExprKind::Identifier(name) => {
                format!("chamada `{}`", self.ctx.symbol_name(name.sym))
            }
            _ => "chamada de valor de função".to_string(),
        };
        self.nao_suportado(&oque, expr.span)
    }

    /// `alvo.m(args)` quando `m` não é membro estático do tipo do alvo: o
    /// membro pela classe dinâmica (receptor sem tipo útil), `f.call(…)`, ou
    /// o membro do SDK casado pelo nome.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn chamada_sem_membro(
        &mut self,
        ast: &ast::Ast,
        expr: &ast::Expr,
        target: &ExprId,
        inner_target: &ExprId,
        m_name: &str,
        recv_op: Operand,
        arguments: &ast::Arguments,
    ) -> Operand {        // Receptor sem tipo útil: o membro pela classe dinâmica.
        if self.receptor_dinamico(*inner_target) {
            let alvos = self.alvos_por_nome(m_name);
            if !alvos.is_empty() {
                let nome = m_name.to_string();
                let r2 = recv_op.clone();
                return self.despachar(
                    recv_op,
                    &alvos,
                    super::despacho::Uso::Chamar,
                    &mut |s: &mut Self| s.avaliar_args(ast, &arguments.args),
                    &mut |s: &mut Self| {
                        let n = s.erros.len();
                        let r = s.metodo_sdk_por_nome(
                            ast,
                            expr,
                            target,
                            inner_target,
                            &nome,
                            r2.clone(),
                            arguments,
                        );
                        if s.erros.len() > n {
                            s.erros.truncate(n);
                            return s.lancar_nsm(&nome);
                        }
                        r
                    },
                    expr.span,
                );
            }
        }

        // `f.call(…)` sobre um valor função.
        if m_name == "call" && self.e_valor_funcao(*inner_target) {
            let avaliados = self.avaliar_args(ast, &arguments.args);
            return self.chamar_valor_funcao(recv_op, &avaliados);
        }

        self.metodo_sdk_por_nome(
            ast,
            expr,
            target,
            inner_target,
            m_name,
            recv_op,
            arguments,
        )
    }

    /// `prefixo.nome` com `prefixo` de um `import … as prefixo`: o elemento.
    pub fn elemento_prefixado(
        &self,
        ast: &ast::Ast,
        alvo: ExprId,
        nome: dartforge_intern::SymbolId,
    ) -> Option<dartforge_elements::model::Element> {
        let ExprKind::Identifier(p) = &ast.expr(alvo).kind else {
            return None;
        };
        match self.ctx.get_resolved(self.unit_id, alvo) {
            Some(Resolved::Prefix(_)) | None => {}
            _ => return None,
        }
        if self.buscar_local(p.sym).is_some() {
            return None;
        }
        let lib = self.ctx.program.unit(self.unit_id).library;
        let b = self.ctx.program.lookup_prefixed(lib, p.sym, nome)?;
        b.getter.or(b.setter)
    }
}

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
            if let Some((sym, ret_ty, _)) = self.local_functions.get(&id.sym).cloned() {
                // Função local: parâmetros e retorno são `Ref` (R4).
                let mut args_ops = Vec::new();
                for a in &arguments.args {
                    let v = self.lower_expr(ast, a.value);
                    args_ops.push(self.coagir(v, Type::Ref));
                }
                return self.emit_call_with_check(
                    Instruction::CallStatic {
                        symbol: sym,
                        args: args_ops,
                        ret_ty,
                    },
                    ret_ty,
                );
            }
            // Função de topo, membro implícito (`m()` = `this.m()`) ou
            // estático: pelo elemento resolvido do alvo.
            match self.ctx.get_resolved(self.unit_id, *target).cloned() {
                Some(Resolved::Element(dartforge_elements::model::Element::Function(f)))
                    if crate::lower::funcao_do_usuario(self.ctx, f.0 as usize)
                        && self.ctx.program.functions[f.0 as usize].variable.is_none() =>
                {
                    let fid = f.0 as usize;
                    let avaliados = self.avaliar_args(ast, &arguments.args);
                    let args = self.casar_args(fid, &avaliados);
                    return self.chamar_direto(fid, None, args);
                }
                Some(Resolved::Member {
                    member: MemberRef::Function(f),
                    ..
                }) if crate::lower::funcao_do_usuario(self.ctx, f.0 as usize)
                    && self.ctx.program.functions[f.0 as usize].variable.is_none() =>
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

            // `C.m(…)`: método estático do usuário.
            let alvo_e_classe = matches!(
                self.ctx.get_resolved(self.unit_id, *inner_target),
                Some(Resolved::Element(
                    dartforge_elements::model::Element::Class(_)
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

            let recv_op = self.lower_alvo(ast, *inner_target);
            if *null_aware {
                self.desviar_se_nulo(&recv_op);
            }

            // Método de classe do usuário: pelo elemento resolvido, com
            // despacho pela classe dinâmica (R7).
            if let Some((_, member)) =
                self.membro_do_usuario(*target, *inner_target, method_name.sym, false)
            {
                {
                    let MemberRef::Function(f) = member else {
                        return self.nao_suportado("chamada de campo de tipo função", expr.span);
                    };
                    if self.ctx.program.functions[f.0 as usize].variable.is_some() {
                        return self.nao_suportado("chamada de campo de tipo função", expr.span);
                    }
                    let avaliados = self.avaliar_args(ast, &arguments.args);
                    return self.chamar_membro(recv_op, f.0 as usize, &avaliados, expr.span);
                }
            }

            return self.metodo_sdk_por_nome(
                ast,
                expr,
                target,
                inner_target,
                m_name,
                recv_op,
                arguments,
            );
        }

        self.chamada_nao_suportada(ast, target, expr)
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
}

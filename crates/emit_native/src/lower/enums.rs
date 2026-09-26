//! Enums (P3): cada valor é um objeto canônico do programa, guardado num
//! global preguiçoso (o mesmo mecanismo das variáveis de topo, N6).
//!
//! O objeto de um enum tem, antes dos campos declarados, os dois campos
//! implícitos da especificação (§13 "Enums"): `index` (posição 0, `int`) e
//! `_name` (posição 1, a `String` do nome). O valor é criado como o `const`
//! de um enum aprimorado: aloca, grava `index` e `_name`, e roda o
//! construtor escolhido (`valor(args)`/`valor.nome(args)`) com os campos
//! declarados. `values` é a lista dos valores na ordem de declaração;
//! `toString()` é `Enum.valor` (o do `_Enum` da VM), a menos que o enum
//! declare o seu. A igualdade é identidade (o valor é canônico), e a chave
//! de recarga é `(classe, nome)` (PESQUISA-HOT-RELOAD §4.7): o símbolo do
//! global é o caminho `dfg.<biblioteca>.<Enum>.<valor>`.

use super::fn_builder::FnBuilder;
use crate::context::Context;
use crate::hir::*;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, ClassKind, VariableId, VariableRef};
use dartforge_frontend::ast::DeclKind;

/// A classe é um `enum` do programa.
pub fn e_enum(ctx: &Context, cid: ClassId) -> bool {
    let c = &ctx.program.classes[cid.0 as usize];
    c.kind == ClassKind::Enum && ctx.biblioteca_compilada(c.library)
}

/// Quantos campos implícitos vêm antes do layout declarado (2 num enum).
pub fn base_do_layout(ctx: &Context, cid: ClassId) -> usize {
    if e_enum(ctx, cid) { 2 } else { 0 }
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// `index`/`name` de um valor de enum da classe `cid`.
    pub fn membro_de_enum(&mut self, cid: ClassId, nome: &str, valor: Operand) -> Option<Operand> {
        if !e_enum(self.ctx, cid) {
            return None;
        }
        let (idx, ty) = match nome {
            "index" => (0, Type::I64),
            "name" => (1, Type::Ref),
            _ => return None,
        };
        let valor = self.coagir(valor, Type::Ref);
        Some(self.emit(
            Instruction::CallRuntime {
                name: "dartforge_object_get".to_string(),
                args: vec![
                    (valor, Type::Ref),
                    (Operand::Constant(Constant::Int(idx)), Type::I64),
                ],
                ret_ty: ty,
            },
            ty,
        ))
    }

    /// `E.values`: a lista dos valores, na ordem de declaração.
    pub fn valores_do_enum(&mut self, cid: ClassId, span: Span) -> Operand {
        // `E.values` é uma constante: a mesma lista (`List<E>`, imutável)
        // em toda leitura.
        let classe = &self.ctx.program.classes[cid.0 as usize];
        let simbolo = format!(
            "dfc.{}.{}.values",
            crate::context::escapar(&self.ctx.nome_da_biblioteca(classe.library)),
            crate::context::escapar(self.ctx.symbol_name(classe.name))
        );
        let consts: Vec<VariableId> = classe.enum_constants.clone();
        self.constante_gerada(&simbolo, true, |b| {
            let mut elementos = Vec::with_capacity(consts.len());
            for v in consts {
                let x = b.ler_global(v, span);
                elementos.push((x, 3));
            }
            let l = b.emit(Instruction::AllocList { elements: elementos }, Type::Ref);
            if let Some(lista) = b.ctx.core.list_class {
                let tipo = b.rti_da_receita(&super::rti::Receita {
                    texto: format!("C{}<C{}>", b.ctx.id_rti(lista), b.ctx.id_rti(cid)),
                    variaveis: false,
                });
                b.definir_rti(l.clone(), tipo);
            }
            l
        })
    }

    /// Corpo do getter preguiçoso de um valor de enum: cria o objeto
    /// canônico na primeira leitura.
    pub fn lower_valor_de_enum(&mut self, vid: VariableId) {
        let var = &self.ctx.program.variables[vid.0 as usize];
        let VariableRef::EnumConstant { unit, decl, index } = var.node else {
            return;
        };
        let Some(cid) = var.class else { return };
        let valor = super::simbolo_valor_global(self.ctx, vid);
        let bandeira = format!("{valor}$ok");
        let ok = self.emit(
            Instruction::LoadGlobal {
                simbolo: bandeira.clone(),
                ty: Type::I8,
            },
            Type::I8,
        );
        let pronto = self.emit(
            Instruction::ICmp(ICmpOp::Ne, ok, Operand::Constant(Constant::Int(0))),
            Type::I1,
        );
        let b_ler = self.new_block();
        let b_init = self.new_block();
        self.terminate(Terminator::CondBranch {
            cond: pronto,
            then_block: b_ler,
            else_block: b_init,
        });
        self.set_block(b_ler);
        let v = self.emit(
            Instruction::LoadGlobal {
                simbolo: valor.clone(),
                ty: Type::Ref,
            },
            Type::Ref,
        );
        self.terminate(Terminator::Return(Some(v)));
        self.set_block(b_init);
        self.emit(
            Instruction::StoreGlobal {
                simbolo: bandeira,
                val: Operand::Constant(Constant::Int(1)),
                ty: Type::I8,
                raiz: None,
            },
            Type::Void,
        );
        let n_campos = super::membros::layout(self.ctx, cid).len() + base_do_layout(self.ctx, cid);
        let id = self.ctx.id_de_classe(cid).unwrap_or(0);
        let obj = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_object_new".to_string(),
                args: vec![
                    (Operand::Constant(Constant::Int(i64::from(id))), Type::I64),
                    (Operand::Constant(Constant::Int(n_campos as i64)), Type::I64),
                ],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        );
        // O objeto já é o valor do global (e raiz) antes do construtor: um
        // valor que se refere a si mesmo no construtor lê o objeto.
        self.emit(
            Instruction::StoreGlobal {
                simbolo: valor.clone(),
                val: obj.clone(),
                ty: Type::Ref,
                raiz: Some(vid.0),
            },
            Type::Void,
        );
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_marcar_constante".to_string(),
                args: vec![(obj.clone(), Type::Ref), (Operand::Constant(Constant::Funcao(self.func.symbol.clone())), Type::I64)],
                ret_ty: Type::Void,
            },
            Type::Void,
        );
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_object_set".to_string(),
                args: vec![
                    (obj.clone(), Type::Ref),
                    (Operand::Constant(Constant::Int(0)), Type::I64),
                    (Operand::Constant(Constant::Int(index as i64)), Type::I64),
                    (Operand::Constant(Constant::Int(0)), Type::I8),
                ],
                ret_ty: Type::Void,
            },
            Type::Void,
        );
        let nome = self.ctx.symbol_name(var.name).to_string();
        let texto = self.emit(Instruction::Const(Constant::String(nome)), Type::Ref);
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_object_set".to_string(),
                args: vec![
                    (obj.clone(), Type::Ref),
                    (Operand::Constant(Constant::Int(1)), Type::I64),
                    (texto, Type::I64),
                    (Operand::Constant(Constant::Int(1)), Type::I8),
                ],
                ret_ty: Type::Void,
            },
            Type::Void,
        );
        // O construtor do valor (o sem nome quando o valor não escolhe).
        let ast = &self.ctx.program.unit(unit).ast;
        let DeclKind::Enum(e) = &ast.decl(decl).kind else {
            self.terminate(Terminator::Return(Some(obj)));
            return;
        };
        let constante = &e.constants[index];
        let nome_ctor = constante
            .constructor
            .map(|n| n.sym)
            .or_else(|| self.ctx.interner.lookup(""));
        let ctor = nome_ctor.and_then(|s| {
            self.ctx.program.classes[cid.0 as usize]
                .constructors
                .get(&s)
                .copied()
        });
        if let Some(f) = ctor {
            let fid = f.0 as usize;
            if super::membros::tem_corpo(self.ctx, fid) {
                let salvo = self.unit_id;
                self.unit_id = unit;
                let avaliados = match &constante.arguments {
                    Some(a) => self.avaliar_args(ast, &a.args),
                    None => Vec::new(),
                };
                let args = self.casar_args(fid, &avaliados);
                self.unit_id = salvo;
                self.chamar_direto(fid, Some(obj.clone()), args);
            }
        }
        self.terminate(Terminator::Return(Some(obj)));
    }

    /// `toString()` padrão de um enum: `Enum.valor`.
    pub fn lower_to_string_de_enum(&mut self, cid: ClassId) {
        let this = Operand::Val(self.add_param("this".to_string(), Type::Ref));
        let nome = self.ctx.symbol_name(self.ctx.program.classes[cid.0 as usize].name).to_string();
        let prefixo = self.emit(Instruction::Const(Constant::String(format!("{nome}."))), Type::Ref);
        let n = self.membro_de_enum(cid, "name", this).expect("enum");
        let r = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_string_concat".to_string(),
                args: vec![(prefixo, Type::Ref), (n, Type::Ref)],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        );
        self.terminate(Terminator::Return(Some(r)));
    }
}

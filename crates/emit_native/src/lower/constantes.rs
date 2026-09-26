//! `const` canônico (P3): o valor de uma expressão constante é um objeto só,
//! por valor estrutural (especificação §17.3 "Constants": duas expressões
//! constantes com o mesmo valor denotam o mesmo objeto, então
//! `identical(const C(1), const C(1))` é verdadeiro).
//!
//! A chave de uma constante é o valor estrutural dela, escrito como texto
//! (`o:<construtor>(<args>)`, `l:[…]`, `i:1`…). Cada chave tem um global
//! preguiçoso `dfc.<hash>` com o getter `dfc.<hash>.get`, gerado por quem a
//! usa primeiro; o getter avalia a expressão uma vez e marca as coleções
//! como imutáveis. Expressão que não tem chave (argumento que o lowering não
//! sabe avaliar como constante) é baixada como antes, sem canonização.

use super::fn_builder::FnBuilder;
use crate::hir::*;
use dartforge_elements::model::Element;
use dartforge_frontend::ast::{self, CollectionElement, CreationKeyword, ExprId, ExprKind, UnaryOp};
use dartforge_types::table::{Type as DartType, TypeId};
use dartforge_types::resolved::{MemberRef, Resolved};

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// Nome de um literal `#x`; nomes privados carregam a identidade da
    /// biblioteca e usam sufixo hexadecimal, como o nome mangled da VM.
    pub fn nome_literal_simbolo(&self, names: &[ast::Name]) -> String {
        let mut name = names.iter().map(|n| self.ctx.symbol_name(n.sym)).collect::<Vec<_>>().join(".");
        if name.starts_with('_') {
            let lib = self.ctx.program.unit(self.unit_id).library;
            let uri = &self.ctx.program.library(lib).uri;
            name.push_str(&format!("@{:016x}", super::closures::hash_nome(uri) as u64));
        }
        name
    }

    /// A chave estrutural da expressão constante `e` (`em_const`: dentro de
    /// um contexto constante, onde `C(…)` e `[…]` são implicitamente const).
    pub fn chave_constante(&self, ast: &ast::Ast, e: ExprId, em_const: bool) -> Option<String> {
        let expr = ast.expr(e);
        match &expr.kind {
            ExprKind::Int(s) => {
                let t = self.source()[s.start as usize..s.end as usize].replace('_', "");
                let v: i64 = if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
                    u64::from_str_radix(h, 16).ok()? as i64
                } else {
                    // Ver `lower_expr`: o literal é lido como `u64`.
                    t.parse::<u64>().ok()? as i64
                };
                Some(format!("i:{v}"))
            }
            ExprKind::Double(s) => {
                let t = self.source()[s.start as usize..s.end as usize].replace('_', "");
                let v: f64 = t.parse().ok()?;
                Some(format!("d:{}", v.to_bits()))
            }
            ExprKind::Bool(b) => Some(format!("b:{b}")),
            ExprKind::Null => Some("n".to_string()),
            ExprKind::Symbol(names) => {
                let name = self.nome_literal_simbolo(names);
                Some(format!("sym:{name:?}"))
            }
            ExprKind::String(s) => {
                let v = s.constant_value()?;
                Some(format!("s:{:?}", String::from_utf8_lossy(v.as_bytes())))
            }
            ExprKind::Parenthesized(x) => self.chave_constante(ast, *x, em_const),
            ExprKind::Unary { op: UnaryOp::Neg, operand } => {
                let k = self.chave_constante(ast, *operand, em_const)?;
                Some(format!("neg({k})"))
            }
            ExprKind::Identifier(n) if self.chaves_de_const_locais.contains_key(&n.sym) => {
                self.chaves_de_const_locais.get(&n.sym).map(|(k, _)| k.clone())
            }
            ExprKind::Binary { op, left, right } => {
                let a = self.chave_constante(ast, *left, true)?;
                let b = self.chave_constante(ast, *right, true)?;
                let int = |k: &str| k.strip_prefix("i:").and_then(|v| v.parse::<i64>().ok());
                match (op, int(&a), int(&b)) {
                    (ast::BinaryOp::Add, Some(x), Some(y)) => Some(format!("i:{}", x.wrapping_add(y))),
                    (ast::BinaryOp::Sub, Some(x), Some(y)) => Some(format!("i:{}", x.wrapping_sub(y))),
                    (ast::BinaryOp::Mul, Some(x), Some(y)) => Some(format!("i:{}", x.wrapping_mul(y))),
                    (ast::BinaryOp::Add, None, None) if a.starts_with("s:") && b.starts_with("s:") => {
                        // Duas strings constantes: o texto concatenado.
                        let ta: String = serde_texto(&a)?;
                        let tb: String = serde_texto(&b)?;
                        Some(format!("s:{:?}", ta + &tb))
                    }
                    _ => None,
                }
            }
            ExprKind::Identifier(_) | ExprKind::Property { .. } => {
                let vid = match self.ctx.get_resolved(self.unit_id, e) {
                    Some(Resolved::Element(Element::Variable(v))) => *v,
                    Some(Resolved::Element(Element::Function(f))) => self.ctx.program.functions[f.0 as usize].variable?,
                    Some(Resolved::Member { member: MemberRef::Variable(v), .. }) => *v,
                    Some(Resolved::Member { member: MemberRef::Function(f), .. }) => {
                        self.ctx.program.functions[f.0 as usize].variable?
                    }
                    _ => return None,
                };
                let var = &self.ctx.program.variables[vid.0 as usize];
                let enum_ = matches!(var.node, dartforge_elements::model::VariableRef::EnumConstant { .. });
                if !(var.const_ || enum_) {
                    return None;
                }
                let biblioteca = self.ctx.nome_da_biblioteca(var.library);
                let dono = if let Some(cid) = var.class {
                    format!(".{}", self.ctx.symbol_name(self.ctx.program.class(cid).name))
                } else if let Some(eid) = var.extension {
                    let nome = self.ctx.program.extension(eid).name?;
                    format!(".{}", self.ctx.symbol_name(nome))
                } else {
                    String::new()
                };
                Some(format!("g:{:?}", (biblioteca, dono, self.ctx.symbol_name(var.name))))
            }
            ExprKind::InstanceCreation { keyword, arguments, .. } => {
                let em_const = em_const || *keyword == Some(CreationKeyword::Const);
                if !em_const {
                    return None;
                }
                let Some(Resolved::Constructor(f)) = self.ctx.get_resolved(self.unit_id, e) else {
                    return None;
                };
                self.chave_de_criacao(ast, f.0 as usize, arguments)
            }
            ExprKind::Call { arguments, .. } if em_const => {
                let Some(Resolved::Constructor(f)) = self.ctx.get_resolved(self.unit_id, e) else {
                    return None;
                };
                self.chave_de_criacao(ast, f.0 as usize, arguments)
            }
            ExprKind::List { const_, elements, .. } => {
                if !(em_const || *const_) {
                    return None;
                }
                let mut partes = Vec::new();
                for el in elements.iter() {
                    let CollectionElement::Expression(x) = el else { return None };
                    partes.push(self.chave_constante(ast, *x, true)?);
                }
                Some(format!("l{}:[{}]", self.tipo_na_chave(e)?, partes.join(",")))
            }
            ExprKind::SetOrMap { const_, elements, .. } => {
                if !(em_const || *const_) {
                    return None;
                }
                let conjunto = self.literal_e_conjunto(e, elements);
                let mut partes = Vec::new();
                for el in elements.iter() {
                    match (el, conjunto) {
                        (CollectionElement::Expression(x), true) => partes.push(self.chave_constante(ast, *x, true)?),
                        (
                            CollectionElement::MapEntry {
                                key,
                                value,
                                null_aware_key: false,
                                null_aware_value: false,
                            },
                            false,
                        ) => {
                            let k = self.chave_constante(ast, *key, true)?;
                            let v = self.chave_constante(ast, *value, true)?;
                            partes.push(format!("{k}=>{v}"));
                        }
                        _ => return None,
                    }
                }
                Some(format!("{}{}:{{{}}}", if conjunto { "c" } else { "m" }, self.tipo_na_chave(e)?, partes.join(",")))
            }
            ExprKind::Record { positional, named, .. } if em_const => {
                let mut partes = Vec::new();
                for x in positional.iter() {
                    partes.push(self.chave_constante(ast, *x, true)?);
                }
                let mut nomeados: Vec<(String, String)> = Vec::new();
                for (n, x) in named.iter() {
                    nomeados.push((self.ctx.symbol_name(n.sym).to_string(), self.chave_constante(ast, *x, true)?));
                }
                nomeados.sort();
                for (n, k) in nomeados {
                    partes.push(format!("{n}={k}"));
                }
                Some(format!("r:({})", partes.join(",")))
            }
            _ => None,
        }
    }

    /// O tipo estático na chave (`const <int>[]` e `const <String>[]` são
    /// constantes diferentes).
    fn tipo_na_chave(&self, e: ExprId) -> Option<String> {
        self.ctx
            .get_type(self.unit_id, e)
            .map_or_else(|| Some(String::new()), |t| self.tipo_estavel(t).map(|s| format!("<{s}>")))
    }

    /// Identidade estrutural do tipo, sem `TypeId`/`ClassId`/`SymbolId`, cuja
    /// numeração depende da ordem de carga e de internação dos arquivos.
    /// Se o tipo depende de um parâmetro genérico, a expressão não recebe
    /// getter canônico até haver uma identidade independente da instância.
    fn tipo_estavel(&self, t: TypeId) -> Option<String> {
        let partes = |xs: &[TypeId]| -> Option<Vec<String>> {
            xs.iter().map(|&x| self.tipo_estavel(x)).collect()
        };
        match self.ctx.table.get(t) {
            DartType::Dynamic => Some("dynamic".into()),
            DartType::Void => Some("void".into()),
            DartType::Never => Some("Never".into()),
            DartType::Null => Some("Null".into()),
            DartType::Interface { class, args, nullable }
            | DartType::ExtensionType { decl: class, args, nullable } => {
                let c = self.ctx.program.class(*class);
                let lib = self.ctx.nome_da_biblioteca(c.library);
                Some(format!("{lib:?}.{}<{}>{nullable}", self.ctx.symbol_name(c.name), partes(args)?.join(",")))
            }
            DartType::FutureOr { arg, nullable } => Some(format!("FutureOr<{}>{nullable}", self.tipo_estavel(*arg)?)),
            DartType::Record { positional, named, nullable } => {
                let mut campos: Vec<_> = named
                    .iter()
                    .map(|(n, t)| Some((self.ctx.symbol_name(*n).to_owned(), self.tipo_estavel(*t)?)))
                    .collect::<Option<_>>()?;
                campos.sort();
                Some(format!("record({};{:?}){nullable}", partes(positional)?.join(","), campos))
            }
            DartType::Function { type_params, ret, positional, optional, named, nullable } if type_params.is_empty() => {
                let mut campos: Vec<_> = named
                    .iter()
                    .map(|(n, t, req)| Some((self.ctx.symbol_name(*n).to_owned(), self.tipo_estavel(*t)?, req)))
                    .collect::<Option<_>>()?;
                campos.sort();
                Some(format!(
                    "fn({};{};{:?})->{}{nullable}",
                    partes(positional)?.join(","), partes(optional)?.join(","), campos, self.tipo_estavel(*ret)?
                ))
            }
            DartType::Function { .. } | DartType::TypeParameter { .. } | DartType::Intersection { .. } => None,
        }
    }

    fn chave_de_criacao(&self, ast: &ast::Ast, fid: usize, arguments: &ast::Arguments) -> Option<String> {
        if !super::funcao_do_usuario(self.ctx, fid) {
            return None;
        }
        if self.ctx.program.functions[fid].class == self.ctx.classe_do_sdk("core", "Symbol") {
            let text = arguments.args.first().and_then(|a| match &ast.expr(a.value).kind {
                ExprKind::String(s) => s.constant_value().map(|v| String::from_utf8_lossy(v.as_bytes()).into_owned()),
                _ => None,
            })?;
            return Some(format!("sym:{text:?}"));
        }
        let mut pos = Vec::new();
        let mut nomeados = Vec::new();
        for a in arguments.args.iter() {
            let k = self.chave_constante(ast, a.value, true)?;
            match a.name {
                Some(n) => nomeados.push((self.ctx.symbol_name(n.sym).to_string(), k)),
                None => pos.push(k),
            }
        }
        nomeados.sort();
        for (n, k) in nomeados {
            pos.push(format!("{n}={k}"));
        }
        Some(format!("o:{}({})", super::simbolo_de(self.ctx, fid), pos.join(",")))
    }

    /// Se `e` é uma constante canônica (`const`, literal constante de
    /// coleção), o valor dela pelo global canônico; senão `None`.
    pub fn constante_canonica(&mut self, ast: &ast::Ast, e: ExprId) -> Option<Operand> {
        if self.constante_em_curso == Some(e) {
            return None;
        }
        let ctx_const = self.em_contexto_const;
        let explicita = match &ast.expr(e).kind {
            ExprKind::InstanceCreation { keyword, .. } => ctx_const || *keyword == Some(CreationKeyword::Const),
            ExprKind::Symbol(_) => true,
            ExprKind::List { const_, .. } | ExprKind::SetOrMap { const_, .. } => ctx_const || *const_,
            ExprKind::Call { .. } | ExprKind::Record { .. } => ctx_const,
            _ => false,
        };
        if !explicita {
            return None;
        }
        let chave = self.chave_constante(ast, e, true)?;
        let hash = super::closures::hash_nome(&chave) as u64;
        let simbolo_valor = format!("dfc.{hash:016x}");
        let getter = format!("{simbolo_valor}.get");
        if !self.entradas_feitas.contains(&getter) {
            self.entradas_feitas.insert(getter.clone());
            let raiz = 0x4000_0000 | (hash as u32 & 0x3fff_ffff);
            self.globais_extras.push((raiz, Type::Ref, simbolo_valor.clone()));
            let mut g = FnBuilder::new(self.ctx, self.unit_id, getter.clone(), "const".to_string(), Type::Ref);
            g.constante_em_curso = Some(e);
            g.em_contexto_const = true;
            // As `const` locais visíveis aqui (o getter é outra função: ele
            // as avalia de novo, pelo inicializador, que é constante).
            g.chaves_de_const_locais = self.chaves_de_const_locais.clone();
            g.lower_getter_constante(ast, e, &simbolo_valor, raiz);
            self.globais_extras.extend(std::mem::take(&mut g.globais_extras));
            self.absorver(g);
        }
        Some(self.emit_call_with_check(
            Instruction::CallStatic {
                symbol: getter,
                args: Vec::new(),
                ret_ty: Type::Ref,
            },
            Type::Ref,
        ))
    }

    /// Corpo do getter de uma constante canônica.
    fn lower_getter_constante(&mut self, ast: &ast::Ast, e: ExprId, valor: &str, raiz: u32) {
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
                simbolo: valor.to_string(),
                ty: Type::Ref,
            },
            Type::Ref,
        );
        self.terminate(Terminator::Return(Some(v)));
        self.set_block(b_init);
        let v = self.lower_expr(ast, e);
        let v = self.coagir(v, Type::Ref);
        let v = if matches!(ast.expr(e).kind, ExprKind::List { .. } | ExprKind::SetOrMap { .. }) {
            self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_collection_mark_unmodifiable".to_string(),
                    args: vec![(v, Type::Ref)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            )
        } else {
            v
        };
        self.emit(
            Instruction::StoreGlobal {
                simbolo: valor.to_string(),
                val: v.clone(),
                ty: Type::Ref,
                raiz: Some(raiz),
            },
            Type::Void,
        );
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_marcar_permanente".to_string(),
                args: vec![(v.clone(), Type::Ref)],
                ret_ty: Type::Void,
            },
            Type::Void,
        );
        self.emit(
            Instruction::StoreGlobal {
                simbolo: bandeira,
                val: Operand::Constant(Constant::Int(1)),
                ty: Type::I8,
                raiz: None,
            },
            Type::Void,
        );
        self.terminate(Terminator::Return(Some(v)));
    }

    /// Baixa `e` num contexto constante (inicializador `const`, valor
    /// padrão de parâmetro).
    pub fn lower_em_contexto_const(&mut self, ast: &ast::Ast, e: ExprId) -> Operand {
        let salvo = std::mem::replace(&mut self.em_contexto_const, true);
        let v = self.lower_expr(ast, e);
        self.em_contexto_const = salvo;
        v
    }
}

/// O texto de uma chave `s:"…"` (o `{:?}` de uma `String`).
fn serde_texto(chave: &str) -> Option<String> {
    let aspas = chave.strip_prefix("s:")?;
    let dentro = aspas.strip_prefix('"')?.strip_suffix('"')?;
    // Só o caso sem escapes: o resto não é canonizado por texto.
    (!dentro.contains('\\')).then(|| dentro.to_string())
}

//! `const` canônico (P3): o valor de uma expressão constante é um objeto só,
//! por valor estrutural (especificação §17.3 "Constants": duas expressões
//! constantes com o mesmo valor denotam o mesmo objeto, então
//! `identical(const C(1), const C(1))` é verdadeiro).
//!
//! A chave de uma constante é o valor estrutural dela, escrito como texto
//! (`o:<constructor>(<args>)`, `l:[…]`, `i:1`…). Cada chave tem um global
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
    pub fn symbol_literal_name(&self, names: &[ast::Name]) -> String {
        let mut name = names.iter().map(|n| self.ctx.symbol_name(n.sym)).collect::<Vec<_>>().join(".");
        if name.starts_with('_') {
            let lib = self.ctx.program.unit(self.unit_id).library;
            let uri = &self.ctx.program.library(lib).uri;
            name.push_str(&format!("@{:016x}", super::closures::hash_name(uri) as u64));
        }
        name
    }

    /// A chave estrutural da expressão constante `e` (`in_const`: dentro de
    /// um contexto constante, onde `C(…)` e `[…]` são implicitamente const).
    pub fn constant_key(&self, ast: &ast::Ast, e: ExprId, in_const: bool) -> Option<String> {
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
                let name = self.symbol_literal_name(names);
                Some(format!("sym:{name:?}"))
            }
            ExprKind::String(s) => {
                let v = s.constant_value()?;
                Some(format!("s:{:?}", String::from_utf8_lossy(v.as_bytes())))
            }
            ExprKind::Parenthesized(x) => self.constant_key(ast, *x, in_const),
            ExprKind::Unary { op: UnaryOp::Neg, operand } => {
                let k = self.constant_key(ast, *operand, in_const)?;
                Some(format!("neg({k})"))
            }
            ExprKind::Identifier(n) if self.local_const_keys.contains_key(&n.sym) => {
                self.local_const_keys.get(&n.sym).map(|(k, _)| k.clone())
            }
            ExprKind::Binary { op, left, right } => {
                let a = self.constant_key(ast, *left, true)?;
                let b = self.constant_key(ast, *right, true)?;
                let int = |k: &str| k.strip_prefix("i:").and_then(|v| v.parse::<i64>().ok());
                match (op, int(&a), int(&b)) {
                    (ast::BinaryOp::Add, Some(x), Some(y)) => Some(format!("i:{}", x.wrapping_add(y))),
                    (ast::BinaryOp::Sub, Some(x), Some(y)) => Some(format!("i:{}", x.wrapping_sub(y))),
                    (ast::BinaryOp::Mul, Some(x), Some(y)) => Some(format!("i:{}", x.wrapping_mul(y))),
                    (ast::BinaryOp::Add, None, None) if a.starts_with("s:") && b.starts_with("s:") => {
                        // Duas strings constantes: o texto concatenado.
                        let ta: String = serde_string(&a)?;
                        let tb: String = serde_string(&b)?;
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
                if let Some(k) = self.primitive_const_key(vid) {
                    return Some(k);
                }
                let library = self.ctx.library_name(var.library);
                let owner = if let Some(cid) = var.class {
                    format!(".{}", self.ctx.symbol_name(self.ctx.program.class(cid).name))
                } else if let Some(eid) = var.extension {
                    let name_str = self.ctx.program.extension(eid).name?;
                    format!(".{}", self.ctx.symbol_name(name_str))
                } else {
                    String::new()
                };
                Some(format!("g:{:?}", (library, owner, self.ctx.symbol_name(var.name))))
            }
            ExprKind::InstanceCreation { keyword, arguments, .. } => {
                let in_const = in_const || *keyword == Some(CreationKeyword::Const);
                if !in_const {
                    return None;
                }
                let Some(Resolved::Constructor(f)) = self.ctx.get_resolved(self.unit_id, e) else {
                    return None;
                };
                self.creation_key(ast, e, f.0 as usize, arguments)
            }
            ExprKind::Call { arguments, .. } if in_const => {
                let Some(Resolved::Constructor(f)) = self.ctx.get_resolved(self.unit_id, e) else {
                    return None;
                };
                self.creation_key(ast, e, f.0 as usize, arguments)
            }
            ExprKind::List { const_, elements, .. } => {
                if !(in_const || *const_) {
                    return None;
                }
                let parts = self.elements_parts(ast, elements, false)?;
                Some(format!("l{}:[{}]", self.type_in_key(e)?, parts.join(",")))
            }
            ExprKind::SetOrMap { const_, elements, .. } => {
                if !(in_const || *const_) {
                    return None;
                }
                let set = self.literal_is_set(e, elements);
                let parts = self.elements_parts(ast, elements, !set)?;
                Some(format!("{}{}:{{{}}}", if set { "c" } else { "m" }, self.type_in_key(e)?, parts.join(",")))
            }
            ExprKind::Record { positional, named, .. } if in_const => {
                let mut parts = Vec::new();
                for x in positional.iter() {
                    parts.push(self.constant_key(ast, *x, true)?);
                }
                let mut named_args: Vec<(String, String)> = Vec::new();
                for (n, x) in named.iter() {
                    named_args.push((self.ctx.symbol_name(n.sym).to_string(), self.constant_key(ast, *x, true)?));
                }
                named_args.sort();
                for (n, k) in named_args {
                    parts.push(format!("{n}={k}"));
                }
                Some(format!("r:({})", parts.join(",")))
            }
            _ => None,
        }
    }

    /// As chaves dos elementos de uma coleção constante, na ordem: os
    /// espalhamentos de outra coleção constante entram elemento a elemento e
    /// o `if` de condição constante pelo ramo escolhido (`const [...a, 3]` é
    /// a mesma constante que `const [1, 2, 3]`). `map`: entradas `k=>v`.
    fn elements_parts(&self, ast: &ast::Ast, elements: &[CollectionElement], map: bool) -> Option<Vec<String>> {
        let mut parts = Vec::new();
        for el in elements {
            self.element_parts(ast, el, map, &mut parts)?;
        }
        Some(parts)
    }

    fn element_parts(&self, ast: &ast::Ast, el: &CollectionElement, map: bool, parts: &mut Vec<String>) -> Option<()> {
        match el {
            CollectionElement::Expression(x) if !map => parts.push(self.constant_key(ast, *x, true)?),
            CollectionElement::MapEntry { key, value, null_aware_key: false, null_aware_value: false } if map => {
                let k = self.constant_key(ast, *key, true)?;
                let v = self.constant_key(ast, *value, true)?;
                parts.push(format!("{k}=>{v}"));
            }
            CollectionElement::Spread { value, null_aware } => {
                if *null_aware && self.constant_key(ast, *value, true).as_deref() == Some("n") {
                    return Some(());
                }
                parts.extend(self.spread_parts(ast, *value, map)?);
            }
            CollectionElement::If { condition, case_pattern: None, guard: None, then, else_ } => {
                match self.constant_key(ast, *condition, true)?.as_str() {
                    "b:true" => self.element_parts(ast, then, map, parts)?,
                    "b:false" => {
                        if let Some(e) = else_ {
                            self.element_parts(ast, e, map, parts)?;
                        }
                    }
                    _ => return None,
                }
            }
            _ => return None,
        }
        Some(())
    }

    /// Os elementos (ou entradas) da coleção constante que `e` denota: o
    /// literal, uma `const` local, ou uma `const` de topo/estática desta
    /// unidade.
    fn spread_parts(&self, ast: &ast::Ast, e: ExprId, map: bool) -> Option<Vec<String>> {
        match &ast.expr(e).kind {
            ExprKind::Parenthesized(x) => self.spread_parts(ast, *x, map),
            ExprKind::List { elements, .. } if !map => self.elements_parts(ast, elements, false),
            ExprKind::SetOrMap { elements, .. } => {
                let set = self.literal_is_set(e, elements);
                if set == map {
                    return None;
                }
                self.elements_parts(ast, elements, map)
            }
            ExprKind::Identifier(n) if self.local_const_keys.contains_key(&n.sym) => {
                let (_, init) = self.local_const_keys.get(&n.sym)?;
                self.spread_parts(ast, *init, map)
            }
            ExprKind::Identifier(_) | ExprKind::Property { .. } => {
                let vid = match self.ctx.get_resolved(self.unit_id, e) {
                    Some(Resolved::Element(Element::Variable(v))) => *v,
                    Some(Resolved::Element(Element::Function(f))) => self.ctx.program.functions[f.0 as usize].variable?,
                    Some(Resolved::Member { member: MemberRef::Variable(v), .. }) => *v,
                    _ => return None,
                };
                let var = &self.ctx.program.variables[vid.0 as usize];
                if !var.const_ {
                    return None;
                }
                let unit_ref = match var.node {
                    dartforge_elements::model::VariableRef::Field { unit, .. }
                    | dartforge_elements::model::VariableRef::TopLevel { unit, .. } => unit,
                    _ => return None,
                };
                let init = self.variable_initializer_in(vid)?;
                if unit_ref == self.unit_id {
                    return self.spread_parts(ast, init, map);
                }
                // A `const` de outra unidade (outra biblioteca ou outra
                // `part`): o inicializador é um `ExprId` da AST *dela*, lido
                // com as resoluções e a fonte dela. Com a AST corrente, o
                // índice apontava para outra expressão (a chave saía errada,
                // `identical` dava falso) ou para fora da arena (pânico).
                let other = FnBuilder::new(self.ctx, unit_ref, String::new(), String::new(), Type::Ref);
                let ast_other = &self.ctx.program.unit(unit_ref).ast;
                other.spread_parts(ast_other, init, map)
            }
            _ => None,
        }
    }

    /// O tipo estático na chave (`const <int>[]` e `const <String>[]` são
    /// constantes diferentes).
    fn type_in_key(&self, e: ExprId) -> Option<String> {
        self.ctx
            .get_type(self.unit_id, e)
            .map_or_else(|| Some(String::new()), |t| self.stable_type(t).map(|s| format!("<{s}>")))
    }

    /// Identidade estrutural do tipo, sem `TypeId`/`ClassId`/`SymbolId`, cuja
    /// numeração depende da ordem de carga e de internação dos arquivos.
    /// Se o tipo depende de um parâmetro genérico, a expressão não recebe
    /// getter canônico até haver uma identidade independente da instância.
    fn stable_type(&self, t: TypeId) -> Option<String> {
        let parts = |xs: &[TypeId]| -> Option<Vec<String>> {
            xs.iter().map(|&x| self.stable_type(x)).collect()
        };
        match self.ctx.table.get(t) {
            DartType::Dynamic => Some("dynamic".into()),
            DartType::Void => Some("void".into()),
            DartType::Never => Some("Never".into()),
            DartType::Null => Some("Null".into()),
            DartType::Interface { class, args, nullable }
            | DartType::ExtensionType { decl: class, args, nullable } => {
                let c = self.ctx.program.class(*class);
                let lib = self.ctx.library_name(c.library);
                Some(format!("{lib:?}.{}<{}>{nullable}", self.ctx.symbol_name(c.name), parts(args)?.join(",")))
            }
            DartType::FutureOr { arg, nullable } => Some(format!("FutureOr<{}>{nullable}", self.stable_type(*arg)?)),
            DartType::Record { positional, named, nullable } => {
                let mut fields: Vec<_> = named
                    .iter()
                    .map(|(n, t)| Some((self.ctx.symbol_name(*n).to_owned(), self.stable_type(*t)?)))
                    .collect::<Option<_>>()?;
                fields.sort();
                Some(format!("record({};{:?}){nullable}", parts(positional)?.join(","), fields))
            }
            DartType::Function { type_params, ret, positional, optional, named, nullable } if type_params.is_empty() => {
                let mut fields: Vec<_> = named
                    .iter()
                    .map(|(n, t, req)| Some((self.ctx.symbol_name(*n).to_owned(), self.stable_type(*t)?, req)))
                    .collect::<Option<_>>()?;
                fields.sort();
                Some(format!(
                    "fn({};{};{:?})->{}{nullable}",
                    parts(positional)?.join(","), parts(optional)?.join(","), fields, self.stable_type(*ret)?
                ))
            }
            DartType::Function { .. } | DartType::TypeParameter { .. } | DartType::Intersection { .. } => None,
        }
    }

    /// A chave de `const C<…>(…)`: o construtor, os argumentos e — numa
    /// classe genérica — o tipo da criação: `const Link<Token>()` e `const
    /// Link<BeginToken>()` são objetos distintos (os argumentos de tipo fazem
    /// parte da identidade da constante), senão o segundo seria o primeiro,
    /// com o tipo reificado errado (o `groupingStack` do scanner do analyzer).
    fn creation_key(&self, ast: &ast::Ast, e: ExprId, fid: usize, arguments: &ast::Arguments) -> Option<String> {
        if !super::user_function(self.ctx, fid) {
            return None;
        }
        if self.ctx.program.functions[fid].class == self.ctx.sdk_class("core", "Symbol") {
            let text = arguments.args.first().and_then(|a| match &ast.expr(a.value).kind {
                ExprKind::String(s) => s.constant_value().map(|v| String::from_utf8_lossy(v.as_bytes()).into_owned()),
                _ => None,
            })?;
            return Some(format!("sym:{text:?}"));
        }
        let mut pos = Vec::new();
        let mut named_args = Vec::new();
        for a in arguments.args.iter() {
            let k = self.constant_key(ast, a.value, true)?;
            match a.name {
                Some(n) => named_args.push((self.ctx.symbol_name(n.sym).to_string(), k)),
                None => pos.push(k),
            }
        }
        named_args.sort();
        for (n, k) in named_args {
            pos.push(format!("{n}={k}"));
        }
        let generic = self.ctx.program.functions[fid]
            .class
            .and_then(|c| self.ctx.outline.classes.get(c.0 as usize))
            .is_some_and(|d| !d.type_params.is_empty());
        let ty = if generic { self.type_in_key(e)? } else { String::new() };
        Some(format!("o:{}{ty}({})", super::symbol_of(self.ctx, fid), pos.join(",")))
    }

    /// Se `e` é uma constante canônica (`const`, literal constante de
    /// coleção), o valor dela pelo global canônico; senão `None`.
    pub fn canonical_constant(&mut self, ast: &ast::Ast, e: ExprId) -> Option<Operand> {
        if self.constant_in_progress == Some(e) {
            return None;
        }
        let ctx_const = self.in_const_context;
        let explicit = match &ast.expr(e).kind {
            ExprKind::InstanceCreation { keyword, .. } => ctx_const || *keyword == Some(CreationKeyword::Const),
            ExprKind::Symbol(_) => true,
            ExprKind::List { const_, .. } | ExprKind::SetOrMap { const_, .. } => ctx_const || *const_,
            ExprKind::Call { .. } | ExprKind::Record { .. } => ctx_const,
            _ => false,
        };
        if !explicit {
            return None;
        }
        let getter = self.constant_getter(ast, e)?;
        Some(self.emit_call_with_check(
            Instruction::CallStatic {
                symbol: getter,
                args: Vec::new(),
                ret_ty: Type::Ref,
            },
            Type::Ref,
        ))
    }

    /// O getter canônico (`dfc.<hash>.get`) da expressão constante `e`,
    /// criado na primeira vez, sem chamá-lo (a tabela de um literal grande
    /// o cita, `literals.rs`); `None` sem chave.
    pub fn constant_getter(&mut self, ast: &ast::Ast, e: ExprId) -> Option<String> {
        if self.constant_in_progress == Some(e) {
            return None;
        }
        let key = self.constant_key(ast, e, true)?;
        let hash = super::closures::hash_name(&key) as u64;
        let value_symbol = format!("dfc.{hash:016x}");
        let getter = format!("{value_symbol}.get");
        if !self.done_entries.contains(&getter) {
            self.done_entries.insert(getter.clone());
            if self.ctx.arc_memory {
                self.arc_shared_functions.insert(getter.clone());
                // O getter devolve a constante como as funções Dart: Owned.
                self.dart_ref_returns.insert(getter.clone());
            }
            let root = 0x4000_0000 | (hash as u32 & 0x3fff_ffff);
            self.extra_globals.push((root, Type::Ref, value_symbol.clone()));
            let mut g = FnBuilder::new(self.ctx, self.unit_id, getter.clone(), "const".to_string(), Type::Ref);
            g.constant_in_progress = Some(e);
            g.in_const_context = true;
            // As `const` locais visíveis aqui (o getter é outra função: ele
            // as avalia de novo, pelo inicializador, que é constante).
            g.local_const_keys = self.local_const_keys.clone();
            g.lower_constant_getter(ast, e, &value_symbol, root);
            self.extra_globals.extend(std::mem::take(&mut g.extra_globals));
            self.absorb(g);
        }
        Some(getter)
    }

    /// Corpo do getter de uma constante canônica.
    fn lower_constant_getter(&mut self, ast: &ast::Ast, e: ExprId, value: &str, root: u32) {
        // `const {…}` é um `_ConstMap`/`_ConstSet` (imutável, como na VM),
        // montado do `_Map`/`_Set` do literal.
        if let ExprKind::SetOrMap { elements, .. } = &ast.expr(e).kind {
            let set = self.literal_is_set(e, elements);
            let ty = self.ctx.get_type(self.unit_id, e);
            self.lower_canonical_getter(value, root, false, |b| {
                let source = b.lower_expr(ast, e);
                b.source_constant_collection(source, ty, set, ast.expr(e).span)
            });
            return;
        }
        let collection = matches!(ast.expr(e).kind, ExprKind::List { .. } | ExprKind::SetOrMap { .. });
        self.lower_canonical_getter(value, root, collection, |b| b.lower_expr(ast, e));
    }

    /// O `_ConstMap<K, V>`/`_ConstSet<E>` que adota o conteúdo de `source`
    /// (o `_Map`/`_Set` do literal), com os argumentos de tipo de `ty`.
    fn source_constant_collection(&mut self, source: Operand, ty: Option<TypeId>, set: bool, span: dartforge_diagnostics::Span) -> Operand {
        let name_str = if set { "_ConstSet" } else { "_ConstMap" };
        let Some(cid) = self.ctx.sdk_class("_compact_hash", name_str) else {
            return self.unsupported(&format!("constante sem `{name_str}`"), span);
        };
        let ctor = self
            .ctx
            .interner
            .lookup("_deFonte")
            .and_then(|k| self.ctx.program.classes[cid.0 as usize].constructors.get(&k).copied());
        let Some(ctor) = ctor else {
            return self.unsupported(&format!("`{name_str}._deFonte` ausente"), span);
        };
        let n = if set { 1 } else { 2 };
        let args: Vec<TypeId> = match ty.map(|t| self.ctx.table.get(t)) {
            Some(DartType::Interface { args, .. }) if args.len() == n => args.to_vec(),
            _ => vec![self.ctx.core.dynamic_; n],
        };
        let mut recipe = super::rti::Recipe { text: String::new(), variables: false };
        for (i, a) in args.iter().enumerate() {
            if i > 0 {
                recipe.text.push(',');
            }
            let r = self.type_recipe(*a);
            recipe.text.push_str(&r.text);
            recipe.variables |= r.variables;
        }
        let object = self.recipe_rti(&super::rti::Recipe {
            text: format!("C{}<{}>", self.ctx.id_rti(cid), recipe.text),
            variables: recipe.variables,
        });
        let source = self.coerce(source, Type::Ref);
        self.instantiate_evaluated_with_rti(ctor, &[(None, source)], span, Some(object), None)
    }

    /// A constante canônica que `gerar` produz, pelo getter `value_symbol`
    /// + `.get` (criada na primeira leitura; `collection`: fica imutável).
    pub fn generated_constant(&mut self, value_symbol: &str, collection: bool, gerar: impl FnOnce(&mut Self) -> Operand) -> Operand {
        let getter = format!("{value_symbol}.get");
        if !self.done_entries.contains(&getter) {
            self.done_entries.insert(getter.clone());
            if self.ctx.arc_memory {
                self.arc_shared_functions.insert(getter.clone());
                // O getter devolve a constante como as funções Dart: Owned.
                self.dart_ref_returns.insert(getter.clone());
            }
            let hash = super::closures::hash_name(value_symbol) as u64;
            let root = 0x4000_0000 | (hash as u32 & 0x3fff_ffff);
            self.extra_globals.push((root, Type::Ref, value_symbol.to_string()));
            let mut g = FnBuilder::new(self.ctx, self.unit_id, getter.clone(), "const".to_string(), Type::Ref);
            g.in_const_context = true;
            g.lower_canonical_getter(value_symbol, root, collection, gerar);
            self.extra_globals.extend(std::mem::take(&mut g.extra_globals));
            self.absorb(g);
        }
        self.emit_call_with_check(Instruction::CallStatic { symbol: getter, args: Vec::new(), ret_ty: Type::Ref }, Type::Ref)
    }

    fn lower_canonical_getter(&mut self, value: &str, root: u32, collection: bool, gerar: impl FnOnce(&mut Self) -> Operand) {
        let flag = format!("{value}$ok");
        let ok = self.emit(
            Instruction::LoadGlobal {
                symbol: flag.clone(),
                ty: Type::I8,
            },
            Type::I8,
        );
        let ready = self.emit(
            Instruction::ICmp(ICmpOp::Ne, ok, Operand::Constant(Constant::Int(0))),
            Type::I1,
        );
        let b_read = self.new_block();
        let b_init = self.new_block();
        self.terminate(Terminator::CondBranch {
            cond: ready,
            then_block: b_read,
            else_block: b_init,
        });
        self.set_block(b_read);
        let v = self.emit(
            Instruction::LoadGlobal {
                symbol: value.to_string(),
                ty: Type::Ref,
            },
            Type::Ref,
        );
        self.terminate(Terminator::Return(Some(v)));
        self.set_block(b_init);
        let v = gerar(self);
        let v = self.coerce(v, Type::Ref);
        // A lista constante vira uma `_ImmutableList` NOVA (a classe não muda
        // depois de publicada, docs/NATIVO-ESPACO-UNIFICADO.md §2.16): o valor
        // guardado e devolvido é o que o runtime devolve.
        let v = if collection {
            self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_collection_mark_unmodifiable".to_string(),
                    args: vec![(v, Type::Ref)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            )
        } else {
            // Uma `String` montada (interpolação constante) vira o literal
            // canônico de mesmo conteúdo; outro valor volta como está.
            self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_constante_canonica".to_string(),
                    args: vec![(v, Type::Ref)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            )
        };
        self.emit(
            Instruction::StoreGlobal {
                symbol: value.to_string(),
                val: v.clone(),
                ty: Type::Ref,
                root: Some(root),
            },
            Type::Void,
        );
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_marcar_constante".to_string(),
                args: vec![(v.clone(), Type::Ref), (Operand::Constant(Constant::Function(self.func.symbol.clone())), Type::I64)],
                ret_ty: Type::Void,
            },
            Type::Void,
        );
        self.emit(
            Instruction::StoreGlobal {
                symbol: flag,
                val: Operand::Constant(Constant::Int(1)),
                ty: Type::I8,
                root: None,
            },
            Type::Void,
        );
        self.terminate(Terminator::Return(Some(v)));
    }

    /// Baixa `e` num contexto constante (inicializador `const`, valor
    /// padrão de parâmetro).
    pub fn lower_in_const_context(&mut self, ast: &ast::Ast, e: ExprId) -> Operand {
        let saved = std::mem::replace(&mut self.in_const_context, true);
        let v = self.lower_expr(ast, e);
        self.in_const_context = saved;
        v
    }
}

/// O texto de uma chave `s:"…"` (o `{:?}` de uma `String`).
fn serde_string(key: &str) -> Option<String> {
    let quotes = key.strip_prefix("s:")?;
    let inside = quotes.strip_prefix('"')?.strip_suffix('"')?;
    // Só o caso sem escapes: o resto não é canonizado por texto.
    (!inside.contains('\\')).then(|| inside.to_string())
}

//! Literais de coleção com elementos de controle (P3): `...e`, `...?e`,
//! `?e`, `if (c) e else f`, `for (…) e`, e o literal de conjunto.
//!
//! A especificação (§17.9 "Collection literals") avalia os elementos em
//! ordem e acrescenta cada valor produzido; `...e` acrescenta os elementos de
//! `e` (`...?e` ignora null); `if` e `for` produzem zero ou mais valores. O
//! literal de lista só de expressões continua na alocação direta
//! (`AllocList`, `llvm/ir_lists.rs`); os outros começam vazios (a
//! `_GrowableList` do literal, o `_Map`/`_Set` da fonte) e acrescentam.

use super::fn_builder::FnBuilder;
use crate::hir::*;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{self, CollectionElement, ExprId, ForInit};

/// A partir de quantos elementos um literal constante vai em pedaços
/// (`FnBuilder::fill_in_chunks`).
const CHUNK_THRESHOLD: usize = 256;
/// Quantos elementos cada função auxiliar de um literal em pedaços avalia.
const LITERAL_CHUNK: usize = 128;

/// Que coleção o literal constrói.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Collection {
    Lista,
    Set,
    Mapa,
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// O literal `{…}` é um conjunto (e não um mapa)? Pelo tipo estático, ou
    /// pela forma dos elementos (`{}` vazio é mapa).
    pub fn literal_is_set(&self, expr_id: ExprId, elements: &[CollectionElement]) -> bool {
        if let Some(t) = self.ctx.get_type(self.unit_id, expr_id)
            && let dartforge_types::table::Type::Interface { class, .. } = self.ctx.table.get(t)
        {
            match self.ctx.symbol_name(self.ctx.program.classes[class.0 as usize].name) {
                "Set" | "LinkedHashSet" => return true,
                "Map" | "LinkedHashMap" => return false,
                _ => {}
            }
        }
        fn leaf(el: &CollectionElement) -> Option<bool> {
            match el {
                CollectionElement::Expression(_) | CollectionElement::NullAwareExpression(_) => Some(true),
                CollectionElement::MapEntry { .. } => Some(false),
                CollectionElement::If { then, .. } => leaf(then),
                CollectionElement::For { body, .. } | CollectionElement::ForIn { body, .. } => leaf(body),
                CollectionElement::Spread { .. } => None,
            }
        }
        elements.iter().find_map(leaf).unwrap_or(false)
    }

    /// Coleção vazia do tipo pedido: a `_GrowableList` do literal (`AllocList`
    /// sem elementos) ou o `_Map`/`_Set` da fonte.
    fn empty_collection(&mut self, typ: Collection) -> Operand {
        match typ {
            Collection::Lista => self.emit(Instruction::AllocList { elements: Vec::new() }, Type::Ref),
            _ => self.source_empty_collection(typ == Collection::Mapa),
        }
    }

    /// Acrescenta um valor à lista ou ao conjunto. A lista é a `_GrowableList`
    /// que o literal acabou de criar (ainda sem o `E` reificado, gravado no fim
    /// por quem baixa o literal): o `add` direto do runtime
    /// (`dartforge_lista_acrescentar`, o valor numa posição `Ref`), sem o
    /// despacho nem a conferência de covariância, que o tipo estático do
    /// elemento já garante. O conjunto usa o `add` do `_Set`.
    fn append(&mut self, alvo: Operand, typ: Collection, v: Operand) {
        if typ == Collection::Set {
            self.call_by_name(alvo, super::sdk_source::AccessKind::Call, "add", &[(None, v)]);
            return;
        }
        let v = self.coerce(v, Type::Ref);
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_lista_acrescentar".to_string(),
                args: vec![(alvo, Type::Ref), (v, Type::Ref)],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        );
    }

    /// Acrescenta uma entrada ao mapa (o `[]=` do `_Map`).
    fn append_entry(&mut self, alvo: Operand, k: Operand, v: Operand) {
        self.call_by_name(alvo, super::sdk_source::AccessKind::Call, "[]=", &[(None, k), (None, v)]);
    }

    /// Literal de coleção com elementos de controle (ou de conjunto).
    pub fn lower_collection_literal(
        &mut self,
        ast: &ast::Ast,
        typ: Collection,
        elements: &[CollectionElement],
        span: Span,
    ) -> Operand {
        let alvo = self.empty_collection(typ);
        if self.fill_from_table(ast, alvo.clone(), typ, elements) {
            return alvo;
        }
        if self.fill_in_chunks(ast, alvo.clone(), typ, elements, span) {
            return alvo;
        }
        if self.fill_map_at_once(ast, alvo.clone(), typ, elements) {
            return alvo;
        }
        for el in elements {
            self.collection_element(ast, alvo.clone(), typ, el, span);
        }
        alvo
    }

    /// Um literal grande só de strings, bools e null (as tabelas de nomes e
    /// de entidades dos pacotes reais) vira DADOS: os elementos numa tabela
    /// de bytes que o runtime converte numa lista
    /// (`dartforge_lista_de_tabela`), e um apoio Dart que a copia para a
    /// coleção — em vez de uma chamada de `add`/`[]=` por elemento, que
    /// fazia do getter de uma constante de 2 mil entradas uma função de 50
    /// mil instruções. Os números entram pelo tipo estático do literal
    /// (`int` como `i`, `double` como `d`); um número de outro tipo estático
    /// (`num`, `Object`: o valor é o `int`, mas sem garantia aqui) deixa o
    /// literal fora da tabela.
    ///
    /// Num contexto constante (o getter de uma constante), um elemento que é
    /// outra constante (`C(…)`, outro literal, `E.value`) entra como `g` e o
    /// índice do getter canônico dele (`dfc.<hash>.get`) num vetor de
    /// endereços (`FunctionTable`), que o runtime chama
    /// (`dartforge_lista_de_tabela_g`): os mapas constantes grandes de
    /// valores constantes (`{'a': C(1), …}`) faziam um `[]=` por seletor por
    /// entrada — 30 MB de IR em 234 getters no new_sali/backend
    /// (docs/NATIVO-PRODUCAO-GRANDE.md §6.5).
    fn fill_from_table(&mut self, ast: &ast::Ast, alvo: Operand, typ: Collection, elements: &[CollectionElement]) -> bool {
        const MINIMUM: usize = 8;
        if elements.len() < MINIMUM {
            return false;
        }
        let mut table: Vec<u8> = Vec::new();
        let mut by_getter: Vec<ast::ExprId> = Vec::new();
        let in_const = self.in_const_context;
        let scalar = |e: ast::ExprId, table: &mut Vec<u8>, by_getter: &mut Vec<ast::ExprId>| -> bool {
            // Número literal (ou `-literal`): pelo TIPO ESTÁTICO da expressão,
            // que já diz se o contexto fez de um literal inteiro um `double`
            // (as tabelas `Map<int, int>` do `enough_convert`, 30 mil
            // entradas).
            let number = matches!(ast.expr(e).kind, ast::ExprKind::Int(_) | ast::ExprKind::Double(_))
                || matches!(&ast.expr(e).kind, ast::ExprKind::Unary { op: ast::UnaryOp::Neg, operand }
                    if matches!(ast.expr(*operand).kind, ast::ExprKind::Int(_) | ast::ExprKind::Double(_)));
            if number {
                let Some(map_key) = self.constant_key(ast, e, true) else { return false };
                let typ = self.ctx.get_type(self.unit_id, e);
                let (int, double) = (Some(self.ctx.core.int), Some(self.ctx.core.double));
                let val_ref = match map_key.as_str() {
                    k if k.starts_with("i:") => k[2..].parse::<i64>().ok().map(|v| (v, false)),
                    k if k.starts_with("d:") => k[2..].parse::<u64>().ok().map(|b| (b as i64, true)),
                    k if k.starts_with("neg(i:") && k.ends_with(')') => {
                        k[6..k.len() - 1].parse::<i64>().ok().map(|v| (v.wrapping_neg(), false))
                    }
                    k if k.starts_with("neg(d:") && k.ends_with(')') => {
                        k[6..k.len() - 1].parse::<u64>().ok().map(|b| ((-f64::from_bits(b)).to_bits() as i64, true))
                    }
                    _ => None,
                };
                let Some((v, is_double)) = val_ref else { return false };
                if typ == int && !is_double {
                    table.push(b'i');
                    table.extend_from_slice(&v.to_le_bytes());
                } else if typ == double {
                    let bits = if is_double { v as u64 } else { (v as f64).to_bits() };
                    table.push(b'd');
                    table.extend_from_slice(&bits.to_le_bytes());
                } else {
                    return false;
                }
                return true;
            }
            let other_constant = |table: &mut Vec<u8>, by_getter: &mut Vec<ast::ExprId>| -> bool {
                if !in_const || self.constant_key(ast, e, true).is_none() {
                    return false;
                }
                table.push(b'g');
                table.extend_from_slice(&(by_getter.len() as u32).to_le_bytes());
                by_getter.push(e);
                true
            };
            match &ast.expr(e).kind {
                ast::ExprKind::Null => table.push(b'n'),
                ast::ExprKind::Bool(b) => table.push(if *b { b't' } else { b'f' }),
                ast::ExprKind::String(s) => {
                    let Some(text) = s.constant_value() else { return false };
                    let bytes = text.as_bytes();
                    table.push(b's');
                    table.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
                    table.extend_from_slice(bytes);
                }
                _ => return other_constant(table, by_getter),
            }
            true
        };
        for el in elements {
            let ok = match (el, typ) {
                (CollectionElement::Expression(e), Collection::Lista | Collection::Set) => scalar(*e, &mut table, &mut by_getter),
                (
                    CollectionElement::MapEntry { key, value, null_aware_key: false, null_aware_value: false },
                    Collection::Mapa,
                ) => scalar(*key, &mut table, &mut by_getter) && scalar(*value, &mut table, &mut by_getter),
                _ => false,
            };
            if !ok {
                return false;
            }
        }
        let mut getters: Vec<String> = Vec::with_capacity(by_getter.len());
        for e in by_getter {
            let Some(g) = self.constant_getter(ast, e) else { return false };
            getters.push(g);
        }
        let support = match typ {
            Collection::Lista => "_dartforgePreencherLista",
            Collection::Set => "_dartforgePreencherConjunto",
            Collection::Mapa => "_dartforgePreencherMapa",
        };
        let Some(fid) = self.top_level_function("dart:_compact_hash", support) else { return false };
        let words: Vec<i64> = table
            .chunks(8)
            .map(|c| {
                let mut w = [0u8; 8];
                w[..c.len()].copy_from_slice(c);
                i64::from_le_bytes(w)
            })
            .collect();
        let data = self.emit(Instruction::ConstArray(words), Type::Ptr);
        let size = (Operand::Constant(Constant::Int(table.len() as i64)), Type::I64);
        let values = if getters.is_empty() {
            self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_lista_de_tabela".to_string(),
                    args: vec![(data, Type::Ptr), size],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            )
        } else {
            let fns = self.emit(Instruction::FunctionTable(getters), Type::Ptr);
            self.emit_call_with_check(
                Instruction::CallRuntime {
                    name: "dartforge_lista_de_tabela_g".to_string(),
                    args: vec![(data, Type::Ptr), size, (fns, Type::Ptr)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            )
        };
        let evaluated: Vec<super::members::Evaluated> = vec![(None, alvo), (None, values)];
        let args = self.match_args(fid, &evaluated);
        self.call_direct(fid, None, args);
        true
    }

    /// O literal de mapa só de entradas (`{k: v, …}`, sem elementos de
    /// controle nem `?`), como a VM o monta: avalia as chaves e os valores na
    /// ordem, guarda-os numa lista `[k0, v0, k1, v1, …]` (`AllocList`, uma
    /// alocação só) e insere tudo no fim (`_dartforgePreencherMapa`). A VM
    /// faz o mesmo (`Map._fromLiteral`,
    /// runtime/vm/compiler/frontend/kernel_binary_flowgraph.cc): o `hashCode`
    /// e o `==` das chaves só rodam depois de todos os valores avaliados.
    /// Antes era um `[]=` por seletor por entrada — o vetor de argumentos, a
    /// busca do seletor, a chamada indireta e a conferência da exceção a cada
    /// uma: os `toMap`/`toJson` de um backend real são feitos disso. Fora do
    /// ARC, que monta a lista pela fábrica Owned (`ir_lists.rs`).
    fn fill_map_at_once(&mut self, ast: &ast::Ast, alvo: Operand, typ: Collection, elements: &[CollectionElement]) -> bool {
        // Até `LITERAL_CHUNK` entradas: os valores ficam vivos (e
        // enraizados) até a lista, no quadro.
        if typ != Collection::Mapa || elements.len() < 2 || elements.len() > LITERAL_CHUNK || self.ctx.arc_memory {
            return false;
        }
        let only_entries = elements.iter().all(|el| {
            matches!(el, CollectionElement::MapEntry { null_aware_key: false, null_aware_value: false, .. })
        });
        if !only_entries {
            return false;
        }
        let Some(fid) = self.top_level_function("dart:_compact_hash", "_dartforgePreencherMapa") else { return false };
        let mut values: Vec<(Operand, u8)> = Vec::with_capacity(elements.len() * 2);
        for el in elements {
            let CollectionElement::MapEntry { key, value, .. } = el else { unreachable!("conferido acima") };
            for e in [*key, *value] {
                let x = self.lower_expr(ast, e);
                if self.is_terminated() {
                    return true;
                }
                let tag = self.operand_tag(&x);
                values.push((x, tag));
            }
        }
        let list = self.emit(Instruction::AllocList { elements: values }, Type::Ref);
        let evaluated: Vec<super::members::Evaluated> = vec![(None, alvo), (None, list)];
        let args = self.match_args(fid, &evaluated);
        self.call_direct(fid, None, args);
        true
    }

    /// O literal grande de elementos constantes que a tabela não cobre
    /// (inteiros, valores de enum, `const` de outras declarações — as
    /// tabelas Unicode do `bidi`, 16 mil entradas `65: CharacterCategory.lu`).
    ///
    /// Numa função só, cada elemento deixava no quadro o seu buffer de
    /// argumentos (`[]=` por seletor) e, no `-O0`, uma palavra de
    /// derramamento por valor que atravessa a conferência de exceção: o
    /// quadro do getter passava de 1 MiB e estourava a pilha do isolado
    /// principal (a thread principal do processo, 1 MiB no Windows — a
    /// mesma reserva da VM, `OSThread::GetMaxStackSize`,
    /// runtime/vm/os_thread_win.cc:101-104). A VM não tem o problema porque
    /// o literal é um vetor preenchido por `StoreIndexed` e um só
    /// `Map._fromLiteral`/`_GrowableList._literal`
    /// (runtime/vm/compiler/frontend/kernel_binary_flowgraph.cc:4051-4140).
    ///
    /// Aqui os elementos vão, em ordem, por funções auxiliares de
    /// [`LITERAL_CHUNK`] elementos cada (`<função>$lit<pos>_<i>(alvo)`), que
    /// acrescentam ao `alvo` como o caminho comum. Só com elementos
    /// constantes (a chave canônica existe): uma constante não lê locais,
    /// `this` nem parâmetros de tipo, então a avaliação fora da função que
    /// contém o literal é a mesma, na mesma ordem — e as `const` locais vão
    /// junto (`local_const_keys`), reavaliadas pelo inicializador
    /// como no getter de constante.
    fn fill_in_chunks(
        &mut self,
        ast: &ast::Ast,
        alvo: Operand,
        typ: Collection,
        elements: &[CollectionElement],
        span: Span,
    ) -> bool {
        if elements.len() < CHUNK_THRESHOLD {
            return false;
        }
        let constants = elements.iter().all(|el| match (el, typ) {
            (CollectionElement::Expression(e), Collection::Lista | Collection::Set) => {
                self.constant_key(ast, *e, true).is_some()
            }
            (
                CollectionElement::MapEntry { key, value, null_aware_key: false, null_aware_value: false },
                Collection::Mapa,
            ) => self.constant_key(ast, *key, true).is_some() && self.constant_key(ast, *value, true).is_some(),
            _ => false,
        });
        if !constants {
            return false;
        }
        let alvo = self.coerce(alvo, Type::Ref);
        for (i, chunk) in elements.chunks(LITERAL_CHUNK).enumerate() {
            let symbol = format!("{}$lit{}_{i}", self.func.symbol, span.start);
            let mut g = FnBuilder::new(self.ctx, self.unit_id, symbol.clone(), "literal".to_string(), Type::Ref);
            g.in_const_context = self.in_const_context;
            g.local_const_keys = self.local_const_keys.clone();
            let p = Operand::Val(g.add_param("alvo".to_string(), Type::Ref));
            for el in chunk {
                if g.is_terminated() {
                    break;
                }
                g.collection_element(ast, p.clone(), typ, el, span);
            }
            if !g.is_terminated() {
                g.terminate(Terminator::Return(Some(Operand::Constant(Constant::Null))));
            }
            // Chamado como função Dart (`CallStatic`): o retorno Ref é Owned.
            if self.ctx.arc_memory {
                g.dart_ref_returns.insert(symbol.clone());
            }
            self.absorb(g);
            self.emit_call_with_check(
                Instruction::CallStatic { symbol: symbol, args: vec![alvo.clone()], ret_ty: Type::Ref },
                Type::Ref,
            );
        }
        true
    }

    /// O literal de lista só de expressões vai pelo caminho em pedaços?
    /// (grande, e todo elemento constante: `fill_in_chunks`).
    pub fn large_literal_is_constant(&self, ast: &ast::Ast, elements: &[CollectionElement]) -> bool {
        elements.len() >= CHUNK_THRESHOLD
            && elements.iter().all(|el| match el {
                CollectionElement::Expression(e) => self.constant_key(ast, *e, true).is_some(),
                _ => false,
            })
    }

    /// `e` nomeia uma `const` (de topo ou estática, desta ou de outra
    /// unidade) cujo inicializador é um literal de lista, conjunto ou mapa:
    /// a unidade e o literal.
    fn constant_literal(&self, ast: &ast::Ast, e: ExprId) -> Option<(dartforge_elements::model::UnitId, ExprId)> {
        use dartforge_elements::model::{Element, VariableRef};
        use dartforge_types::resolved::{MemberRef, Resolved};
        if !matches!(ast.expr(e).kind, ast::ExprKind::Identifier(_) | ast::ExprKind::Property { .. }) {
            return None;
        }
        let vid = match self.ctx.get_resolved(self.unit_id, e)? {
            Resolved::Element(Element::Variable(v)) => *v,
            Resolved::Element(Element::Function(f)) => self.ctx.program.functions[f.0 as usize].variable?,
            Resolved::Member { member: MemberRef::Variable(v), .. } => *v,
            Resolved::Member { member: MemberRef::Function(f), .. } => self.ctx.program.functions[f.0 as usize].variable?,
            _ => return None,
        };
        let var = &self.ctx.program.variables[vid.0 as usize];
        if !var.const_ {
            return None;
        }
        let unit_ref = match var.node {
            VariableRef::Field { unit, .. } | VariableRef::TopLevel { unit, .. } => unit,
            _ => return None,
        };
        let mut init = self.variable_initializer_in(vid)?;
        let ast_c = &self.ctx.program.unit(unit_ref).ast;
        while let ast::ExprKind::Parenthesized(x) = &ast_c.expr(init).kind {
            init = *x;
        }
        matches!(ast_c.expr(init).kind, ast::ExprKind::List { .. } | ast::ExprKind::SetOrMap { .. }).then_some((unit_ref, init))
    }

    /// A função de topo `name` da biblioteca `uri`.
    pub fn top_level_function(&self, uri: &str, name: &str) -> Option<usize> {
        let lib = self.ctx.program.libraries.iter().position(|l| l.uri == uri)?;
        let sym = self.ctx.interner.lookup(name)?;
        let b = self.ctx.program.lookup(dartforge_elements::model::LibraryId(lib as u32), sym)?;
        match b.getter? {
            dartforge_elements::model::Element::Function(f) => Some(f.0 as usize),
            _ => None,
        }
    }

    fn collection_element(
        &mut self,
        ast: &ast::Ast,
        alvo: Operand,
        typ: Collection,
        el: &CollectionElement,
        span: Span,
    ) {
        match el {
            CollectionElement::Expression(e) => {
                let v = self.lower_expr(ast, *e);
                if typ == Collection::Mapa {
                    self.unsupported("expressão num literal de mapa", span);
                    return;
                }
                self.append(alvo, typ, v);
            }
            CollectionElement::NullAwareExpression(e) => {
                let v = self.lower_expr(ast, *e);
                self.if_non_null(v, |s, v| s.append(alvo.clone(), typ, v));
            }
            CollectionElement::MapEntry {
                key,
                value,
                null_aware_key,
                null_aware_value,
            } => {
                if *null_aware_key || *null_aware_value {
                    self.unsupported("entrada de mapa null-aware", span);
                    return;
                }
                let k = self.lower_expr(ast, *key);
                let v = self.lower_expr(ast, *value);
                self.append_entry(alvo, k, v);
            }
            CollectionElement::Spread { value, null_aware } => {
                // Num contexto constante, `...c` de uma `const` cujo
                // inicializador é um literal acrescenta os elementos DO
                // LITERAL (§17.9.1: o resultado é o mesmo), em vez de ler o
                // global `c`. Ler o global criava um ciclo quando a
                // constante que se monta tem a mesma chave canônica que a
                // de `c` (`const [...c]` com `const c = [1, 2]`): o getter
                // canônico avaliava `c`, cujo inicializador chamava o mesmo
                // getter ainda em curso — o `StackOverflowError` do global
                // reentrante (corpus/native/120).
                if self.in_const_context
                    && let Some((unit_ref, init)) = self.constant_literal(ast, *value)
                {
                    let ctx = self.ctx;
                    let ast_c = &ctx.program.unit(unit_ref).ast;
                    let element_list = match &ast_c.expr(init).kind {
                        ast::ExprKind::List { elements, .. } | ast::ExprKind::SetOrMap { elements, .. } => elements,
                        _ => unreachable!("literal_da_constante devolve um literal"),
                    };
                    let saved = std::mem::replace(&mut self.unit_id, unit_ref);
                    for el in element_list.iter() {
                        self.collection_element(ast_c, alvo.clone(), typ, el, span);
                    }
                    self.unit_id = saved;
                    return;
                }
                let source = self.lower_expr(ast, *value);
                let source = self.coerce(source, Type::Ref);
                // `...e` é o `addAll` da coleção (§17.9.1).
                let spread = |s: &mut Self, f: Operand| {
                    s.call_by_name(alvo.clone(), super::sdk_source::AccessKind::Call, "addAll", &[(None, f)]);
                };
                if *null_aware {
                    self.if_non_null(source, spread);
                } else {
                    spread(self, source);
                }
            }
            CollectionElement::If {
                condition,
                case_pattern,
                guard,
                then,
                else_,
            } => {
                let c = self.lower_expr(ast, *condition);
                let b_else = self.new_block();
                let end = self.new_block();
                self.open_scope();
                if let Some(p) = case_pattern {
                    let mut linked = std::collections::HashSet::new();
                    let saved = std::mem::replace(&mut self.refutable_pattern, true);
                    self.match_pattern(ast, *p, c, b_else, super::patterns::Linkage::Declare, &mut linked, *condition);
                    self.refutable_pattern = saved;
                    if let Some(g) = guard {
                        let ok = self.lower_expr(ast, *g);
                        let ok = self.to_bool(ok);
                        let follows = self.new_block();
                        self.terminate(Terminator::CondBranch {
                            cond: ok,
                            then_block: follows,
                            else_block: b_else,
                        });
                        self.set_block(follows);
                    }
                } else {
                    let c = self.to_bool(c);
                    let b_then = self.new_block();
                    self.terminate(Terminator::CondBranch {
                        cond: c,
                        then_block: b_then,
                        else_block: b_else,
                    });
                    self.set_block(b_then);
                }
                self.collection_element(ast, alvo.clone(), typ, then, span);
                self.close_scope();
                self.terminate(Terminator::Branch(end));
                self.set_block(b_else);
                if let Some(e) = else_ {
                    self.collection_element(ast, alvo, typ, e, span);
                }
                self.terminate(Terminator::Branch(end));
                self.set_block(end);
            }
            CollectionElement::For {
                init,
                condition,
                updates,
                body,
                ..
            } => {
                self.open_scope();
                match init {
                    Some(ForInit::Variables(list)) => {
                        for var in list.variables.iter() {
                            let ty = self.local_repr_of(var.name.span.start as usize);
                            let v = match var.initializer {
                                Some(i) => self.lower_expr(ast, i),
                                None => Self::zero_value(ty),
                            };
                            self.declare_variable(var.name.sym, var.name.span.start as usize, ty, v);
                        }
                    }
                    Some(ForInit::Expression(e)) => {
                        self.lower_expr(ast, *e);
                    }
                    Some(ForInit::Pattern { pattern, value, .. }) => {
                        let v = self.lower_expr(ast, *value);
                        self.match_irrefutable(ast, *pattern, v, super::patterns::Linkage::Declare, *value);
                    }
                    None => {}
                }
                let head = self.new_block();
                let fn_body = self.new_block();
                let end = self.new_block();
                self.terminate(Terminator::Branch(head));
                self.set_block(head);
                match condition {
                    Some(c) => {
                        let c = self.lower_expr(ast, *c);
                        let c = self.to_bool(c);
                        self.terminate(Terminator::CondBranch {
                            cond: c,
                            then_block: fn_body,
                            else_block: end,
                        });
                    }
                    None => self.terminate(Terminator::Branch(fn_body)),
                }
                self.set_block(fn_body);
                self.emit_safepoint();
                self.collection_element(ast, alvo, typ, body, span);
                if let Some(ForInit::Variables(list)) = init {
                    for var in list.variables.iter() {
                        self.renew_cell(var.name.sym);
                    }
                }
                for &u in updates.iter() {
                    self.lower_expr(ast, u);
                }
                self.terminate(Terminator::Branch(head));
                self.set_block(end);
                self.close_scope();
            }
            CollectionElement::ForIn {
                target,
                iterable,
                body,
                ..
            } => {
                let source = self.lower_expr(ast, *iterable);
                let source = self.coerce(source, Type::Ref);
                let fn_body: &CollectionElement = body;
                self.iterate_source(source, &mut |s: &mut Self, x: Operand| {
                    s.open_scope();
                    s.bind_for_in_target(ast, target, x, *iterable, span);
                    s.collection_element(ast, alvo.clone(), typ, fn_body, span);
                    s.close_scope();
                });
            }
        }
    }

    /// `if (v != null) f(v)`.
    fn if_non_null(&mut self, v: Operand, f: impl FnOnce(&mut Self, Operand)) {
        if self.operand_type(&v) != Type::Ref {
            f(self, v);
            return;
        }
        let nn = self.emit(
            Instruction::ICmp(ICmpOp::Ne, v.clone(), Operand::Constant(Constant::Int(0))),
            Type::I1,
        );
        let b = self.new_block();
        let end = self.new_block();
        self.terminate(Terminator::CondBranch {
            cond: nn,
            then_block: b,
            else_block: end,
        });
        self.set_block(b);
        f(self, v);
        self.terminate(Terminator::Branch(end));
        self.set_block(end);
    }

    /// `for (i = 0; i < n; i++) f(i)` com o índice num `alloca` (R6).
    pub fn loop_index(&mut self, n: Operand, f: impl FnOnce(&mut Self, Operand)) {
        let p = self.entry_alloca(Type::I64);
        self.emit(
            Instruction::Store {
                ptr: p.clone(),
                val: Operand::Constant(Constant::Int(0)),
            },
            Type::Void,
        );
        let head = self.new_block();
        let fn_body = self.new_block();
        let end = self.new_block();
        self.terminate(Terminator::Branch(head));
        self.set_block(head);
        let i = self.emit(
            Instruction::Load {
                ptr: p.clone(),
                ty: Type::I64,
            },
            Type::I64,
        );
        let c = self.emit(Instruction::ICmp(ICmpOp::Slt, i.clone(), n), Type::I1);
        self.terminate(Terminator::CondBranch {
            cond: c,
            then_block: fn_body,
            else_block: end,
        });
        self.set_block(fn_body);
        f(self, i.clone());
        let next = self.emit(Instruction::Add(i, Operand::Constant(Constant::Int(1))), Type::I64);
        self.emit(Instruction::Store { ptr: p, val: next }, Type::Void);
        self.terminate(Terminator::Branch(head));
        self.set_block(end);
    }

    /// O elemento `i` de uma lista ou conjunto (`for-in`, espalhamento) na
    /// representação `repr` (R5).
    pub fn read_iterable_element(&mut self, source: Operand, i: Operand, repr: Type) -> Operand {
        if repr == Type::Ref {
            return self.emit_call_with_check(
                Instruction::CallRuntime {
                    name: "dartforge_iteravel_get_ref".to_string(),
                    args: vec![(source, Type::Ref), (i, Type::I64)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            );
        }
        let bits = self.emit_call_with_check(
            Instruction::CallRuntime {
                name: "dartforge_iteravel_get_bits".to_string(),
                args: vec![(source, Type::Ref), (i, Type::I64)],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        self.bits_to(bits, repr)
    }
}

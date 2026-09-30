//! O caminho rápido de `[]`, `[]=` e `length` quando o tipo estático do
//! receptor é uma lista tipada numérica (`Int8List` … `Float64List` de
//! `dart:typed_data`) ou uma `List<E>` do núcleo, e os membros externos de
//! lista tipada feitos em linha (`tipada_em_linha`).
//!
//! Essas classes de `dart:typed_data` são `final` no SDK: o valor é sempre a
//! lista interna (`_Int32List`, também sobre memória de fora), ou uma visão
//! (`_Int32ArrayView`, e a não modificável) do `typed_data_patch.dart`. No
//! espaço unificado (docs/NATIVO-ESPACO-UNIFICADO.md §2.5) as três formas têm
//! o comprimento em `h+14` e o endereço dos bytes em `h+22`, invariantes
//! enquanto a lista vive: `@df.tipada_len`/`@df.tipada_dados`
//! (`llvm/tipados_ir.rs`) os leem em linha com `!invariant.load`, e o LLVM tira
//! as cargas dos laços (e vetoriza). Na escrita, a visão não modificável
//! (conferida pelo cid, também invariante) dá comprimento 0. Um índice fora dos
//! limites cai no despacho de sempre, com os mesmos erros da VM.
//!
//! `List` não é `final`: uma classe do usuário pode implementá-la. A classe do
//! receptor (`@df.classe`) diz se é lista do runtime (`_List`,
//! `_ImmutableList`, `_GrowableList`, cids 8–10); a outra recebe comprimento 0
//! e fica com o despacho. O comprimento está em `h+14` nas três; os elementos,
//! no armazenamento (a própria lista, ou a `_List` em `h+22` da expansível),
//! uma palavra cada a partir de `a+22`, com a forma no `flags` dele (§2.16):
//! `REFS` (cada palavra um `Ref`) ou compacta de `int`, `double` ou `bool`
//! (os bits sem caixa). A leitura confere a forma (salvo nas voltas de N13,
//! que a conferem antes); a gravação direta só com `E` igual a `int`,
//! `double` ou `bool` (que nenhuma classe estende; com outro `E`, a lista pode
//! ser de um subtipo e o `[]=` do SDK confere o valor), numa `_List` ou
//! `_GrowableList` (a `_ImmutableList` não aceita) de forma compacta do `E`
//! (`@df.nucleo_len_gravavel`): um escalar sem barreira.

use super::fn_builder::FnBuilder;
use crate::hir::*;
use dartforge_types::table::Type as T;
use dartforge_types::TypeId;

impl FnBuilder<'_, '_> {
    /// A lista fixa das voltas em emissão que `alvo` (um local) nomeia (N13).
    pub(super) fn lista_fixa_de(&self, ast: &dartforge_frontend::ast::Ast, alvo: dartforge_frontend::ast::ExprId) -> Option<super::fn_builder::ListaFixa> {
        if self.listas_fixas.is_empty() {
            return None;
        }
        let dartforge_frontend::ast::ExprKind::Identifier(n) = &ast.expr(alvo).kind else { return None };
        let super::locais::Modo::Memoria(chave) = self.buscar_local(n.sym)?.modo else { return None };
        self.listas_fixas.iter().rev().find(|f| f.chave == chave).cloned()
    }

    /// A lista tipada provada das voltas rápidas em emissão
    /// (`comandos::Contado::Tipadas`) que `alvo` (um local) nomeia; com
    /// `indice`, só se ele é o contador do laço (`alvo[i]`). O local é
    /// identificado pelo `alloca`, então um homônimo declarado no corpo não
    /// casa.
    pub(super) fn provada_de(
        &self,
        ast: &dartforge_frontend::ast::Ast,
        alvo: dartforge_frontend::ast::ExprId,
        indice: Option<dartforge_frontend::ast::ExprId>,
    ) -> Option<super::fn_builder::TipadaProvada> {
        if self.tipadas_provadas.is_empty() {
            return None;
        }
        let endereco = |e: dartforge_frontend::ast::ExprId| {
            let dartforge_frontend::ast::ExprKind::Identifier(n) = &ast.expr(e).kind else { return None };
            match self.buscar_local(n.sym)?.modo {
                super::locais::Modo::Memoria(p) => Some(p),
                _ => None,
            }
        };
        let chave = endereco(alvo)?;
        let contador = match indice {
            Some(i) => Some(endereco(i)?),
            None => None,
        };
        self.tipadas_provadas
            .iter()
            .rev()
            .find(|p| p.chave == chave && contador.as_ref().is_none_or(|c| *c == p.contador))
            .cloned()
    }

    /// O endereço dos elementos e o comprimento de uma lista tipada ou SIMD
    /// (`lista`, do tipo estático `ix`), lidos antes das voltas de um laço
    /// versionado; `escrita`: o comprimento de uma visão não modificável é 0.
    pub(super) fn dados_da_provada(&mut self, lista: &Operand, ix: Indexavel, escrita: bool) -> Option<(Operand, Operand)> {
        let lista = self.coagir(lista.clone(), Type::Ref);
        match ix {
            Indexavel::Tipada(ListaTipada { tipo, .. }) | Indexavel::Simd { tipo, .. } => {
                Some(self.dados_e_comprimento_tipados(&lista, tipo, escrita))
            }
            Indexavel::Nucleo { .. } => None,
        }
    }
}

/// Um receptor com caminho rápido de índice.
#[derive(Debug, Clone, Copy)]
pub enum Indexavel {
    Tipada(ListaTipada),
    /// `List<E>`: a representação do elemento que se grava direto (`E`
    /// igual a `int` ou `double`), ou `None` (só leitura direta).
    Nucleo { gravacao: Option<Type> },
    /// `Float32x4List`, `Int32x4List`, `Float64x2List`: o tipo no runtime
    /// e o vetor sem caixa do elemento (16 bytes, lidos e gravados como um
    /// vetor LLVM).
    Simd { tipo: i64, k: Type },
}

/// Uma lista tipada numérica: o tipo do elemento no runtime (`TIPO_*` de
/// `runtime/src/typed_data.rs`) e o tipo C do elemento.
#[derive(Debug, Clone, Copy)]
pub struct ListaTipada {
    pub tipo: i64,
    pub elemento: TipoC,
}

impl ListaTipada {
    /// A representação Dart do elemento.
    fn repr(self) -> Type {
        if matches!(self.elemento, TipoC::F32 | TipoC::F64) { Type::F64 } else { Type::I64 }
    }

    /// `[]=` pode gravar direto? A `Uint8ClampedList` satura o valor; ela
    /// fica com o `[]=` do SDK.
    fn gravacao_direta(self) -> bool {
        self.tipo != 2
    }
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// O caminho rápido de índice do tipo estático `ty` (não anulável), se
    /// houver.
    pub(super) fn indexavel(&self, ty: Option<TypeId>) -> Option<Indexavel> {
        let T::Interface { class, args, nullable: false } = self.ctx.table.get(ty?) else {
            return None;
        };
        let c = self.ctx.program.classes.get(class.0 as usize)?;
        let biblioteca = self.ctx.program.library(c.library).uri.as_str();
        let nome = self.ctx.symbol_name(c.name);
        if biblioteca == "dart:core" && nome == "List" {
            let gravacao = match args.first().map(|&e| self.ctx.table.get(e)) {
                Some(T::Interface { class: e, nullable: false, .. }) => {
                    let e = self.ctx.program.classes.get(e.0 as usize)?;
                    let nucleo = self.ctx.program.library(e.library).uri == "dart:core";
                    match self.ctx.symbol_name(e.name) {
                        "int" if nucleo => Some(Type::I64),
                        "double" if nucleo => Some(Type::F64),
                        "bool" if nucleo => Some(Type::I1),
                        _ => None,
                    }
                }
                _ => None,
            };
            return Some(Indexavel::Nucleo { gravacao });
        }
        if biblioteca != "dart:typed_data" {
            return None;
        }
        let (tipo, elemento) = match nome {
            "Int8List" => (0, TipoC::I8),
            "Uint8List" => (1, TipoC::U8),
            "Uint8ClampedList" => (2, TipoC::U8),
            "Int16List" => (3, TipoC::I16),
            "Uint16List" => (4, TipoC::U16),
            "Int32List" => (5, TipoC::I32),
            "Uint32List" => (6, TipoC::U32),
            "Int64List" => (7, TipoC::I64),
            "Uint64List" => (8, TipoC::U64),
            "Float32List" => (9, TipoC::F32),
            "Float64List" => (10, TipoC::F64),
            "Float32x4List" => return self.lista_simd(11, Type::V4F32),
            "Int32x4List" => return self.lista_simd(12, Type::V4I32),
            "Float64x2List" => return self.lista_simd(13, Type::V2F64),
            _ => return None,
        };
        Some(Indexavel::Tipada(ListaTipada { tipo, elemento }))
    }

    /// `lista.fillRange(inicio, fim, valor)` de uma lista tipada numérica
    /// (tipo estático `Int8List` … `Float64List`, menos a
    /// `Uint8ClampedList`), com os três argumentos `int`/`int`/elemento não
    /// anuláveis: o runtime preenche a faixa de uma vez
    /// (`dartforge_typed_fill_int`/`_double`); faixa inválida, lista vazia
    /// ou visão não modificável voltam ao `fillRange` do SDK com os mesmos
    /// valores, que lança o erro da VM. As classes de `dart:typed_data` são
    /// `final`: o membro é sempre o do SDK.
    pub(super) fn preencher_tipada(
        &mut self,
        ast: &dartforge_frontend::ast::Ast,
        e: dartforge_frontend::ast::ExprId,
    ) -> Option<Operand> {
        use dartforge_frontend::ast::{ExprId, ExprKind};
        let ExprKind::Call { target, arguments } = &ast.expr(e).kind else { return None };
        if arguments.args.len() != 3 || arguments.args.iter().any(|a| a.name.is_some()) || !arguments.type_args.is_empty() {
            return None;
        }
        let ExprKind::Property { target: recv, name, null_aware: false } = &ast.expr(*target).kind else {
            return None;
        };
        if self.ctx.symbol_name(name.sym) != "fillRange" {
            return None;
        }
        let Some(Indexavel::Tipada(l)) = self.indexavel(self.ctx.get_type(self.unit_id, *recv)) else {
            return None;
        };
        if !l.gravacao_direta() {
            return None;
        }
        let repr = l.repr();
        let classe_de = |s: &Self, x: ExprId| match s.ctx.get_type(s.unit_id, x).map(|t| s.ctx.table.get(t)) {
            Some(T::Interface { class, nullable: false, .. }) => Some(*class),
            _ => None,
        };
        let int = self.ctx.core.int_class;
        let elemento = if repr == Type::F64 { self.ctx.core.double_class } else { int };
        let [a, b, v] = [arguments.args[0].value, arguments.args[1].value, arguments.args[2].value];
        if int.is_none() || classe_de(self, a) != int || classe_de(self, b) != int || classe_de(self, v) != elemento {
            return None;
        }
        // Ordem de avaliação do Dart: receptor, depois os argumentos.
        let mut ops = Vec::with_capacity(4);
        for (x, t) in [(*recv, Type::Ref), (a, Type::I64), (b, Type::I64), (v, repr)] {
            let op = self.lower_expr(ast, x);
            if self.is_terminated() {
                return Some(Operand::Constant(Constant::Null));
            }
            ops.push(self.coagir(op, t));
        }
        let nome = if repr == Type::F64 { "dartforge_typed_fill_double" } else { "dartforge_typed_fill_int" };
        let feito = self.emit(
            Instruction::CallRuntime {
                name: nome.to_string(),
                args: vec![
                    (ops[0].clone(), Type::Ref),
                    (Operand::Constant(Constant::Int(l.tipo)), Type::I64),
                    (ops[1].clone(), Type::I64),
                    (ops[2].clone(), Type::I64),
                    (ops[3].clone(), repr),
                ],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        let ok = self.emit(Instruction::ICmp(ICmpOp::Ne, feito, Operand::Constant(Constant::Int(0))), Type::I1);
        let lento = self.new_block();
        let juncao = self.new_block();
        self.terminate(Terminator::CondBranch { cond: ok, then_block: juncao, else_block: lento });
        self.set_block(lento);
        let args: Vec<super::membros::Avaliado> = ops[1..].iter().map(|o| (None, o.clone())).collect();
        self.chamar_por_nome(ops[0].clone(), super::sdk_fonte::Tipo::Chamar, "fillRange", &args);
        if !self.is_terminated() {
            self.terminate(Terminator::Branch(juncao));
        }
        self.set_block(juncao);
        Some(Operand::Constant(Constant::Null))
    }

    /// Uma lista SIMD: o vetor sem caixa não atravessa quadros assíncronos
    /// (lá o elemento fica com o despacho).
    fn lista_simd(&self, tipo: i64, k: Type) -> Option<Indexavel> {
        self.async_estado.is_none().then_some(Indexavel::Simd { tipo, k })
    }


    /// Um ajudante do prelúdio (`llvm/tipados_ir.rs`): leitura em linha do
    /// bloco, sem efeitos (`EFEITOS_DOS_AJUDANTES`).
    fn ajudante_tipado(&mut self, nome: &str, args: Vec<(Operand, Type)>, ret: Type) -> Operand {
        self.emit(Instruction::CallRuntime { name: nome.to_string(), args, ret_ty: ret }, ret)
    }

    /// O comprimento para o caminho rápido, ou 0 se ele não serve: o da lista
    /// tipada (0 na visão não modificável, na escrita), ou o da lista do
    /// runtime (0 para quem não é; na escrita direta, 0 também para a
    /// imutável ou de outra forma).
    fn comprimento_rapido(&mut self, lista: &Operand, ix: Indexavel, escrita: bool) -> Operand {
        let int = |x: i64| Operand::Constant(Constant::Int(x));
        let gravacao = match ix {
            Indexavel::Tipada(ListaTipada { tipo, .. }) | Indexavel::Simd { tipo, .. } => {
                let (dados, n) = self.dados_e_comprimento_tipados(lista, tipo, escrita);
                self.dados_tipados = Some((lista.clone(), dados));
                return n;
            }
            Indexavel::Nucleo { gravacao } => gravacao,
        };
        let Some(t) = gravacao.filter(|_| escrita) else {
            // O comprimento lógico, em linha; numa lista fixa das voltas
            // (N13), o lido antes delas.
            return match &self.fixa_do_acesso {
                Some(f) => f.comprimento.clone(),
                None => self.ajudante_tipado("df.nucleo_len", vec![(lista.clone(), Type::Ref)], Type::I64),
            };
        };
        // N14: nas voltas de N13, a gravação direta foi conferida antes delas.
        if let Some(n) = self.fixa_do_acesso.as_ref().and_then(|f| f.comprimento_gravavel.clone()) {
            return n;
        }
        // Gravação direta: `_List` ou `_GrowableList` (a classe diz que é
        // modificável) com o armazenamento na forma compacta do `E` (que só
        // existe com o `E` reificado exatamente `int`, `double` ou `bool`:
        // a covariância está satisfeita).
        self.ajudante_tipado(
            "df.nucleo_len_gravavel",
            vec![(lista.clone(), Type::Ref), (int(codigo_da_forma(t)), Type::I64)],
            Type::I64,
        )
    }

    /// `lista.length`.
    pub(super) fn length_indexado(&mut self, lista: Operand, ix: Indexavel) -> Operand {
        // Lista tipada provada das voltas rápidas: o comprimento lido antes.
        if let Some(p) = self.provada_do_acesso.take()
            && let Indexavel::Tipada(_) | Indexavel::Simd { .. } = ix
        {
            return p.comprimento;
        }
        let lista = self.coagir(lista, Type::Ref);
        // N13: a lista do runtime das voltas (comprimento não nulo, fixo).
        if let (Indexavel::Nucleo { .. }, Some(f)) = (ix, &self.fixa_do_acesso) {
            return f.comprimento.clone();
        }
        let n = self.comprimento_rapido(&lista, ix, false);
        self.dados_tipados = None;
        if let Indexavel::Tipada(_) | Indexavel::Simd { .. } = ix {
            // O tipo estático garante a lista tipada do tipo: o comprimento
            // é esse.
            return n;
        }
        // 0: vazia, ou não é lista do runtime — o getter responde.
        let vazia = self.emit(Instruction::ICmp(ICmpOp::Eq, n.clone(), Operand::Constant(Constant::Int(0))), Type::I1);
        let bloco_rapido = self.new_block();
        let bloco_lento = self.new_block();
        let juncao = self.new_block();
        self.terminate(Terminator::CondBranch { cond: vazia, then_block: bloco_lento, else_block: bloco_rapido });
        self.set_block(bloco_rapido);
        self.terminate(Terminator::Branch(juncao));
        self.set_block(bloco_lento);
        let s = self.chamar_por_nome(lista, super::sdk_fonte::Tipo::Ler, "length", &[]);
        let s = self.coagir(s, Type::I64);
        let fim_lento = self.current_block;
        if self.is_terminated() {
            self.set_block(juncao);
            return n;
        }
        self.terminate(Terminator::Branch(juncao));
        self.set_block(juncao);
        self.emit(Instruction::Phi { incoming: vec![(bloco_rapido, n), (fim_lento, s)], ty: Type::I64 }, Type::I64)
    }

    /// Desvia para o caminho rápido quando `indice` está nos limites da
    /// lista apta, senão para o `lento`; junta os dois resultados na
    /// representação `repr`.
    #[allow(clippy::too_many_arguments)]
    fn desviar_indexado(
        &mut self,
        lista: &Operand,
        indice: &Operand,
        ix: Indexavel,
        escrita: bool,
        repr: Type,
        rapido: &mut dyn FnMut(&mut Self) -> Operand,
        lento: &mut dyn FnMut(&mut Self) -> Operand,
    ) -> Operand {
        self.dados_tipados = None;
        let n = self.comprimento_rapido(lista, ix, escrita);
        let ok = self.emit(Instruction::ICmp(ICmpOp::Ult, indice.clone(), n), Type::I1);
        let bloco_rapido = self.new_block();
        let bloco_lento = self.new_block();
        let juncao = self.new_block();
        self.terminate(Terminator::CondBranch { cond: ok, then_block: bloco_rapido, else_block: bloco_lento });

        self.set_block(bloco_rapido);
        let r = rapido(self);
        self.dados_tipados = None;
        let r = self.coagir(r, repr);
        let fim_rapido = self.current_block;
        self.terminate(Terminator::Branch(juncao));

        self.set_block(bloco_lento);
        let s = lento(self);
        let s = self.coagir(s, repr);
        let fim_lento = self.current_block;
        let lento_chega = !self.is_terminated();
        if lento_chega {
            self.terminate(Terminator::Branch(juncao));
        }

        self.set_block(juncao);
        if lento_chega {
            self.emit(Instruction::Phi { incoming: vec![(fim_rapido, r), (fim_lento, s)], ty: repr }, repr)
        } else {
            r
        }
    }

    /// O endereço dos elementos de uma lista tipada apta: o que
    /// `comprimento_rapido` leu para o teste de limites que guarda este
    /// acesso, ou `@df.tipada_dados`.
    fn enderecos_tipados(&mut self, lista: &Operand) -> Operand {
        if let Some((l, d)) = &self.dados_tipados
            && l == lista
        {
            return d.clone();
        }
        self.ajudante_tipado("df.tipada_dados", vec![(lista.clone(), Type::Ref)], Type::I64)
    }

    /// O endereço dos elementos e o comprimento de uma lista tipada do tipo
    /// `tipo` (interna, externa ou visão: os mesmos deslocamentos, §2.5), em
    /// linha e invariantes (`@df.tipada_dados`/`@df.tipada_len`); `escrita`: 0
    /// na visão não modificável do tipo (`@df.tipada_len_gravavel`, pelo cid).
    /// Antes, um cabeçalho de endereço fixo por chamada pura ao runtime e um
    /// cache por ponto de acesso.
    fn dados_e_comprimento_tipados(&mut self, lista: &Operand, tipo: i64, escrita: bool) -> (Operand, Operand) {
        let dados = self.ajudante_tipado("df.tipada_dados", vec![(lista.clone(), Type::Ref)], Type::I64);
        let n = if escrita {
            let imutavel = i64::from(dartforge_runtime::layout::cid::visao(tipo as u8, true));
            self.ajudante_tipado(
                "df.tipada_len_gravavel",
                vec![(lista.clone(), Type::Ref), (Operand::Constant(Constant::Int(imutavel)), Type::I64)],
                Type::I64,
            )
        } else {
            self.ajudante_tipado("df.tipada_len", vec![(lista.clone(), Type::Ref)], Type::I64)
        };
        (dados, n)
    }

    /// `lista[indice]`, no resultado `repr`. `None` quando o índice não é
    /// um `int` sem caixa (fica o despacho).
    pub(super) fn ler_indexado(&mut self, lista: Operand, indice: Operand, ix: Indexavel, repr: Type) -> Option<Operand> {
        if self.operand_type(&indice) != Type::I64 {
            return None;
        }
        // `lista[i]` nas voltas rápidas de um laço versionado: `i` está
        // provado em `0..comprimento` — a carga, sem teste de limites nem
        // caminho lento (que chamaria o runtime e impediria o LLVM de
        // vetorizar o laço).
        if let Some(p) = self.provada_do_acesso.take() {
            match ix {
                Indexavel::Tipada(l) => {
                    let v = self.emit(Instruction::CargaNativa { endereco: p.dados, indice, tipo: l.elemento }, l.repr());
                    return Some(self.coagir(v, repr));
                }
                Indexavel::Simd { k, .. } => {
                    let v = self.emit(Instruction::Simd { op: OpSimd::Carrega, args: vec![p.dados, indice] }, k);
                    return Some(self.coagir(v, repr));
                }
                Indexavel::Nucleo { .. } => {}
            }
        }
        let lista = self.coagir(lista, Type::Ref);
        let (lista2, indice2) = (lista.clone(), indice.clone());
        let mut rapido = |s: &mut Self| match ix {
            Indexavel::Tipada(l) => {
                let endereco = s.enderecos_tipados(&lista2);
                s.emit(Instruction::CargaNativa { endereco, indice: indice2.clone(), tipo: l.elemento }, l.repr())
            }
            Indexavel::Nucleo { .. } => s.ler_elemento_da_lista(&lista2, &indice2, repr),
            Indexavel::Simd { k, .. } => {
                let endereco = s.enderecos_tipados(&lista2);
                s.emit(Instruction::Simd { op: OpSimd::Carrega, args: vec![endereco, indice2.clone()] }, k)
            }
        };
        let mut lento = |s: &mut Self| {
            // O receptor tem tipo estático (a lista indexável): o seletor
            // tipado (`entrada_tipada.rs`).
            s.chamar_por_nome_com_receptor_tipado(lista2.clone(), true, super::sdk_fonte::Tipo::Chamar, "[]", &[(None, indice2.clone())])
        };
        Some(self.desviar_indexado(&lista, &indice, ix, false, repr, &mut rapido, &mut lento))
    }

    /// `lista[indice] = valor`. `false` quando não há caminho rápido (o
    /// chamador faz o despacho).
    pub(super) fn gravar_indexado(&mut self, lista: Operand, indice: Operand, valor: Operand, ix: Indexavel) -> bool {
        let direta = match ix {
            Indexavel::Tipada(l) => l.gravacao_direta(),
            Indexavel::Nucleo { gravacao } => gravacao.is_some(),
            Indexavel::Simd { .. } => true,
        };
        let provada = self.provada_do_acesso.take();
        if !direta || self.operand_type(&indice) != Type::I64 {
            return false;
        }
        // `lista[i] = v` nas voltas rápidas de um laço versionado, com a
        // lista provada também para a gravação: a gravação, sem teste de
        // limites nem caminho lento. `Int32List` etc. guardam os bits baixos
        // (o `trunc` de `GravacaoNativa`), como o `[]=` da VM.
        if let Some(p) = provada.filter(|p| p.escrita) {
            match ix {
                Indexavel::Tipada(l) => {
                    let v = self.coagir(valor, l.repr());
                    self.emit(Instruction::GravacaoNativa { endereco: p.dados, indice, tipo: l.elemento, valor: v }, Type::Void);
                    return true;
                }
                Indexavel::Simd { k, .. } => {
                    let v = self.coagir(valor, k);
                    self.emit(Instruction::Simd { op: OpSimd::Grava, args: vec![p.dados, indice, v] }, Type::Void);
                    return true;
                }
                Indexavel::Nucleo { .. } => {}
            }
        }
        let lista = self.coagir(lista, Type::Ref);
        let (lista2, indice2, valor2) = (lista.clone(), indice.clone(), valor.clone());
        let mut rapido = |s: &mut Self| {
            match ix {
                Indexavel::Tipada(l) => {
                    let endereco = s.enderecos_tipados(&lista2);
                    let v = s.coagir(valor2.clone(), l.repr());
                    s.emit(
                        Instruction::GravacaoNativa { endereco, indice: indice2.clone(), tipo: l.elemento, valor: v },
                        Type::Void,
                    );
                }
                Indexavel::Nucleo { gravacao: Some(t) } => {
                    let v = s.coagir(valor2.clone(), t);
                    s.gravar_elemento_da_lista(&lista2, &indice2, v, t);
                }
                Indexavel::Nucleo { gravacao: None } => unreachable!("gravação direta conferida acima"),
                Indexavel::Simd { k, .. } => {
                    let endereco = s.enderecos_tipados(&lista2);
                    let v = s.coagir(valor2.clone(), k);
                    s.emit(
                        Instruction::Simd { op: OpSimd::Grava, args: vec![endereco, indice2.clone(), v] },
                        Type::Void,
                    );
                }
            }
            Operand::Constant(Constant::Int(0))
        };
        let mut lento = |s: &mut Self| {
            // Um vetor SIMD sem caixa só é encaixotado aqui, no caminho lento.
            let valor = if s.operand_type(&valor).e_vetor() { s.coagir(valor.clone(), Type::Ref) } else { valor.clone() };
            s.chamar_por_nome_com_receptor_tipado(
                lista2.clone(),
                true,
                super::sdk_fonte::Tipo::Chamar,
                "[]=",
                &[(None, indice2.clone()), (None, valor)],
            );
            Operand::Constant(Constant::Int(0))
        };
        self.desviar_indexado(&lista, &indice, ix, true, Type::I64, &mut rapido, &mut lento);
        true
    }

    /// O armazenamento de uma lista do runtime (a própria `_List` ou
    /// `_ImmutableList`, ou os dados da `_GrowableList`): o endereço do
    /// elemento 0 (`a+22`) e a forma dele (`flags & (FORMA | ELEMENTO)`, o
    /// [`codigo_da_forma`]). O chamador sabe que `lista` é lista do runtime.
    pub(super) fn armazenamento_da_lista(&mut self, lista: &Operand) -> (Operand, Operand) {
        let a = self.ajudante_tipado("df.nucleo_armazenamento", vec![(lista.clone(), Type::Ref)], Type::I64);
        let forma = self.ajudante_tipado("df.nucleo_forma", vec![(a.clone(), Type::I64)], Type::I64);
        let dados = self.emit(Instruction::Add(a, Operand::Constant(Constant::Int(ELEMENTOS_DO_ARMAZENAMENTO))), Type::I64);
        (dados, forma)
    }

    /// O comprimento lógico de `lista` se ela é lista do runtime, senão 0
    /// (`@df.nucleo_len`).
    pub(super) fn comprimento_do_nucleo(&mut self, lista: &Operand) -> Operand {
        self.ajudante_tipado("df.nucleo_len", vec![(lista.clone(), Type::Ref)], Type::I64)
    }

    /// O comprimento de `lista` para gravações diretas na forma `forma`
    /// (`@df.nucleo_len_gravavel`): 0 se ela não é `_List`/`_GrowableList`
    /// dessa forma.
    pub(super) fn comprimento_gravavel_do_nucleo(&mut self, lista: &Operand, forma: i64) -> Operand {
        self.ajudante_tipado(
            "df.nucleo_len_gravavel",
            vec![(lista.clone(), Type::Ref), (Operand::Constant(Constant::Int(forma)), Type::I64)],
            Type::I64,
        )
    }

    /// O endereço do elemento 0 e a forma: os conferidos antes das voltas de
    /// N13 (forma conhecida na emissão), ou lidos em linha.
    fn elementos_e_forma(&mut self, lista: &Operand) -> (Operand, Result<i64, Operand>) {
        if let Some(f) = &self.fixa_do_acesso {
            return (f.dados.clone(), Ok(f.forma));
        }
        let (dados, forma) = self.armazenamento_da_lista(lista);
        (dados, Err(forma))
    }

    /// O elemento `indice` de um armazenamento da forma `k`, na
    /// representação natural dela: o `Ref` (`@df.palavra_ref`, que o
    /// emissor enraíza como qualquer `Ref`), ou os bits do escalar.
    fn carregar_na_forma(&mut self, dados: &Operand, indice: &Operand, k: i64) -> Operand {
        match k {
            FORMA_INT => self.emit(
                Instruction::CargaNativa { endereco: dados.clone(), indice: indice.clone(), tipo: TipoC::I64 },
                Type::I64,
            ),
            FORMA_DOUBLE => self.emit(
                Instruction::CargaNativa { endereco: dados.clone(), indice: indice.clone(), tipo: TipoC::F64 },
                Type::F64,
            ),
            FORMA_BOOL => {
                let b = self.emit(
                    Instruction::CargaNativa { endereco: dados.clone(), indice: indice.clone(), tipo: TipoC::I64 },
                    Type::I64,
                );
                self.emit(Instruction::ICmp(ICmpOp::Ne, b, Operand::Constant(Constant::Int(0))), Type::I1)
            }
            _ => self.ajudante_tipado("df.palavra_ref", vec![(dados.clone(), Type::I64), (indice.clone(), Type::I64)], Type::Ref),
        }
    }

    /// O elemento `indice` pelo `[]` do SDK (a forma que não serve à
    /// representação pedida), em `repr`.
    fn elemento_pelo_sdk(&mut self, lista: &Operand, indice: &Operand, repr: Type) -> Operand {
        let r = self.chamar_por_nome_com_receptor_tipado(
            lista.clone(),
            true,
            super::sdk_fonte::Tipo::Chamar,
            "[]",
            &[(None, indice.clone())],
        );
        if self.is_terminated() {
            return Operand::Constant(Constant::Null);
        }
        self.coagir(r, repr)
    }

    /// O elemento `indice` (já conferido) de uma lista do runtime, na
    /// representação `repr`.
    ///
    /// Na forma compacta de `repr` (`int`, `double`, `bool`), os 8 bytes dos
    /// bits em `dados + 8·indice`; na lista geral, o `Ref` (desencaixado se
    /// `repr` é escalar). Lido como `Ref`, um elemento compacto sai
    /// encaixotado. O que sobra (uma forma compacta que não é a de `repr`
    /// escalar) vai ao `[]` do SDK. A forma conhecida na emissão (as voltas
    /// de N13) poupa a conferência.
    pub(super) fn ler_elemento_da_lista(&mut self, lista: &Operand, indice: &Operand, repr: Type) -> Operand {
        let escalar = matches!(repr, Type::I64 | Type::F64 | Type::I1);
        let formas: Vec<i64> = if escalar {
            vec![codigo_da_forma(repr), FORMA_GERAL]
        } else {
            vec![FORMA_GERAL, FORMA_INT, FORMA_DOUBLE, FORMA_BOOL]
        };
        let (dados, forma) = self.elementos_e_forma(lista);
        let f = match forma {
            Ok(k) if formas.contains(&k) => {
                let v = self.carregar_na_forma(&dados, indice, k);
                return self.coagir(v, repr);
            }
            Ok(_) => return self.elemento_pelo_sdk(lista, indice, repr),
            Err(f) => f,
        };
        let juncao = self.new_block();
        let mut entradas = Vec::with_capacity(formas.len() + 1);
        for k in formas {
            let ok = self.emit(Instruction::ICmp(ICmpOp::Eq, f.clone(), Operand::Constant(Constant::Int(k))), Type::I1);
            let sim = self.new_block();
            let nao = self.new_block();
            self.terminate(Terminator::CondBranch { cond: ok, then_block: sim, else_block: nao });
            self.set_block(sim);
            let v = self.carregar_na_forma(&dados, indice, k);
            let v = self.coagir(v, repr);
            if !self.is_terminated() {
                entradas.push((self.current_block, v));
                self.terminate(Terminator::Branch(juncao));
            }
            self.set_block(nao);
        }
        let v = self.elemento_pelo_sdk(lista, indice, repr);
        if !self.is_terminated() {
            entradas.push((self.current_block, v));
            self.terminate(Terminator::Branch(juncao));
        }
        self.set_block(juncao);
        match entradas.len() {
            0 => {
                self.terminate(Terminator::Unreachable);
                Operand::Constant(Constant::Null)
            }
            1 => entradas.pop().expect("uma entrada").1,
            _ => self.emit(Instruction::Phi { incoming: entradas, ty: repr }, repr),
        }
    }

    /// Grava `valor` (`int`, `double` ou `bool`, sem caixa) no elemento
    /// `indice` (já conferido) de uma lista do runtime cuja gravação direta
    /// foi conferida (`@df.nucleo_len_gravavel`): o armazenamento é compacto
    /// da forma de `repr`, e o elemento são os 8 bytes dos bits (escalar:
    /// sem barreira).
    fn gravar_elemento_da_lista(&mut self, lista: &Operand, indice: &Operand, valor: Operand, repr: Type) {
        let (tipo, valor) = match repr {
            Type::F64 => (TipoC::F64, valor),
            Type::I1 => {
                let b = self.emit(Instruction::ZExt { op: valor, from: Type::I1, to: Type::I64 }, Type::I64);
                (TipoC::I64, b)
            }
            _ => (TipoC::I64, valor),
        };
        let (dados, _) = self.elementos_e_forma(lista);
        self.emit(Instruction::GravacaoNativa { endereco: dados, indice: indice.clone(), tipo, valor }, Type::Void);
    }
}

/// A forma de um armazenamento de lista do runtime: os bits `FORMA` e
/// `ELEMENTO` do `flags` (`layout::flags`, §2.3 e §2.16).
pub(super) const FORMA_GERAL: i64 = dartforge_runtime::layout::flags::REFS as i64;
const FORMA_INT: i64 = (dartforge_runtime::layout::flags::BRUTO | dartforge_runtime::layout::flags::ELEMENTO_INT) as i64;
const FORMA_DOUBLE: i64 = (dartforge_runtime::layout::flags::BRUTO | dartforge_runtime::layout::flags::ELEMENTO_DOUBLE) as i64;
const FORMA_BOOL: i64 = (dartforge_runtime::layout::flags::BRUTO | dartforge_runtime::layout::flags::ELEMENTO_BOOL) as i64;

/// O elemento 0 de um armazenamento, a partir do handle (`b+24` = `h+22`).
const ELEMENTOS_DO_ARMAZENAMENTO: i64 =
    dartforge_runtime::layout::desl::ELEMENTOS as i64 - dartforge_runtime::layout::DESLOCAMENTO_DO_HANDLE;

/// O código da forma compacta dos elementos de representação `t` (`int`,
/// `double`, `bool`), o que `@df.nucleo_forma` lê do armazenamento; a forma
/// geral para as outras.
pub(super) fn codigo_da_forma(t: Type) -> i64 {
    match t {
        Type::I64 => FORMA_INT,
        Type::F64 => FORMA_DOUBLE,
        Type::I1 => FORMA_BOOL,
        _ => FORMA_GERAL,
    }
}

/// O tipo de elemento e o tipo C de uma classe de lista tipada numérica do
/// `typed_data_patch.dart` (`_Int8List`, `_Int8ArrayView`,
/// `_UnmodifiableInt8ArrayView`…); `None` para SIMD e o resto.
fn elemento_da_classe(classe: &str) -> Option<(i64, TipoC)> {
    let base = classe.strip_prefix("_Unmodifiable").or_else(|| classe.strip_prefix('_'))?;
    let base = base.strip_suffix("ArrayView").or_else(|| base.strip_suffix("List"))?;
    elemento_do_nome(base)
}

/// `Int8` … `Float64` → (o `TIPO_*`, o tipo C).
fn elemento_do_nome(nome: &str) -> Option<(i64, TipoC)> {
    Some(match nome {
        "Int8" => (0, TipoC::I8),
        "Uint8" => (1, TipoC::U8),
        "Uint8Clamped" => (2, TipoC::U8),
        "Int16" => (3, TipoC::I16),
        "Uint16" => (4, TipoC::U16),
        "Int32" => (5, TipoC::I32),
        "Uint32" => (6, TipoC::U32),
        "Int64" => (7, TipoC::I64),
        "Uint64" => (8, TipoC::U64),
        "Float32" => (9, TipoC::F32),
        "Float64" => (10, TipoC::F64),
        _ => return None,
    })
}

impl FnBuilder<'_, '_> {
    /// O membro externo de lista tipada ou visão feito em linha (P4,
    /// docs/NATIVO-ESPACO-UNIFICADO.md §3.8), ou `None` (a chamada de sempre):
    ///
    /// * `TypedDataBase_length`, `TypedDataView_offsetInBytes`,
    ///   `TypedDataView_typedData`: a carga do bloco;
    /// * o `[]` reconhecido de cada lista numérica e visão
    ///   (`DartForge_typed_indexar_*`): o teste de limites e a carga, com o
    ///   runtime como caminho lento (que lança o `RangeError` da VM);
    /// * `_TypedList._getX`/`_setX` numéricos (os acessos por bytes do
    ///   `ByteData` e das cópias do patch): o teste `0 <= off`, `off + N <=
    ///   bytes` e a carga ou gravação, com o runtime como caminho lento.
    ///
    /// Os resultados saem na representação natural (`int` `I64`, `double`
    /// `F64`), como `texto_em_linha`. SIMD e `_memMove*` ficam no runtime.
    pub(super) fn tipada_em_linha(
        &mut self,
        membro: &str,
        native: Option<&str>,
        this: Option<&Operand>,
        args: &[Operand],
    ) -> Option<Operand> {
        let this = this?.clone();
        if self.operand_type(&this) != Type::Ref {
            return None;
        }
        let nome_do_ajudante = match (native, args.len()) {
            (Some("TypedDataBase_length"), 0) => Some(("df.tipada_len", Type::I64)),
            (Some("TypedDataView_offsetInBytes"), 0) => Some(("df.tipada_deslocamento", Type::I64)),
            (Some("TypedDataView_typedData"), 0) => Some(("df.tipada_base", Type::Ref)),
            (Some(_), _) => return None,
            (None, _) => None,
        };
        if let Some((nome, ret)) = nome_do_ajudante {
            return Some(self.ajudante_tipado(nome, vec![(this, Type::Ref)], ret));
        }
        let lento = crate::nativos::intrinseco(membro)?;
        if let Some(classe) = membro.strip_suffix(".[]") {
            let (_, tc) = elemento_da_classe(classe)?;
            if args.len() != 1 {
                return None;
            }
            return Some(self.indexar_tipada_em_linha(this, args[0].clone(), tc, lento));
        }
        let (grava, nome) = match membro.strip_prefix("_TypedList._get") {
            Some(n) => (false, n),
            None => (true, membro.strip_prefix("_TypedList._set")?),
        };
        let (_, tc) = elemento_do_nome(nome)?;
        if args.len() != usize::from(grava) + 1 {
            return None;
        }
        Some(self.bytes_tipados_em_linha(this, args, tc, grava, lento))
    }

    /// `this[i]` de uma lista numérica ou visão (o elemento `tc`): em linha
    /// quando `0 <= i < length`, senão o intrínseco `lento` do runtime.
    fn indexar_tipada_em_linha(&mut self, this: Operand, i: Operand, tc: TipoC, lento: &str) -> Operand {
        let repr = tc.tipo_hir();
        let i = self.coagir(i, Type::I64);
        let n = self.ajudante_tipado("df.tipada_len", vec![(this.clone(), Type::Ref)], Type::I64);
        let ok = self.emit(Instruction::ICmp(ICmpOp::Ult, i.clone(), n), Type::I1);
        let rapido = self.new_block();
        let devagar = self.new_block();
        let juncao = self.new_block();
        self.terminate(Terminator::CondBranch { cond: ok, then_block: rapido, else_block: devagar });
        self.set_block(rapido);
        let dados = self.ajudante_tipado("df.tipada_dados", vec![(this.clone(), Type::Ref)], Type::I64);
        let v = self.emit(Instruction::CargaNativa { endereco: dados, indice: i.clone(), tipo: tc }, repr);
        let fim_rapido = self.current_block;
        self.terminate(Terminator::Branch(juncao));
        self.set_block(devagar);
        let s = self.emit_call_with_check(
            Instruction::CallRuntime {
                name: crate::nativos::simbolo(lento),
                args: vec![(this, Type::Ref), (i, Type::I64)],
                ret_ty: repr,
            },
            repr,
        );
        let fim_lento = self.current_block;
        let chega = !self.is_terminated();
        if chega {
            self.terminate(Terminator::Branch(juncao));
        }
        self.set_block(juncao);
        if chega {
            self.emit(Instruction::Phi { incoming: vec![(fim_rapido, v), (fim_lento, s)], ty: repr }, repr)
        } else {
            v
        }
    }

    /// `this._getX(off)` / `this._setX(off, v)` (o elemento `tc` de `N`
    /// bytes em `off`, sem alinhamento): em linha quando `off u< bytes` e
    /// `off + N <= bytes`, senão o intrínseco `lento` do runtime (que lança).
    fn bytes_tipados_em_linha(&mut self, this: Operand, args: &[Operand], tc: TipoC, grava: bool, lento: &str) -> Operand {
        let int = |x: i64| Operand::Constant(Constant::Int(x));
        let repr = tc.tipo_hir();
        let off = self.coagir(args[0].clone(), Type::I64);
        let valor = if grava { Some(self.coagir(args[1].clone(), repr)) } else { None };
        let bytes = self.ajudante_tipado("df.tipada_bytes", vec![(this.clone(), Type::Ref)], Type::I64);
        let dentro = self.emit(Instruction::ICmp(ICmpOp::Ult, off.clone(), bytes.clone()), Type::I1);
        let cabe_bloco = self.new_block();
        let rapido = self.new_block();
        let devagar = self.new_block();
        let juncao = self.new_block();
        self.terminate(Terminator::CondBranch { cond: dentro, then_block: cabe_bloco, else_block: devagar });
        self.set_block(cabe_bloco);
        let fim = self.emit(Instruction::Add(off.clone(), int(tc.tamanho_c() as i64)), Type::I64);
        let cabe = self.emit(Instruction::ICmp(ICmpOp::Sle, fim, bytes), Type::I1);
        self.terminate(Terminator::CondBranch { cond: cabe, then_block: rapido, else_block: devagar });

        self.set_block(rapido);
        let dados = self.ajudante_tipado("df.tipada_dados", vec![(this.clone(), Type::Ref)], Type::I64);
        let endereco = self.emit(Instruction::Add(dados, off.clone()), Type::I64);
        let v = match &valor {
            Some(x) => {
                self.emit(Instruction::GravacaoNativa { endereco, indice: int(0), tipo: tc, valor: x.clone() }, Type::Void);
                None
            }
            None => Some(self.emit(Instruction::CargaNativa { endereco, indice: int(0), tipo: tc }, repr)),
        };
        let fim_rapido = self.current_block;
        self.terminate(Terminator::Branch(juncao));

        self.set_block(devagar);
        let mut a = vec![(this, Type::Ref), (off, Type::I64)];
        if let Some(x) = valor {
            a.push((x, repr));
        }
        let ret = if grava { Type::Void } else { repr };
        let s = self.emit_call_with_check(Instruction::CallRuntime { name: crate::nativos::simbolo(lento), args: a, ret_ty: ret }, ret);
        let fim_lento = self.current_block;
        let chega = !self.is_terminated();
        if chega {
            self.terminate(Terminator::Branch(juncao));
        }
        self.set_block(juncao);
        match v {
            None => Operand::Constant(Constant::Null),
            Some(v) if chega => self.emit(Instruction::Phi { incoming: vec![(fim_rapido, v), (fim_lento, s)], ty: repr }, repr),
            Some(v) => v,
        }
    }
}

//! O caminho rápido de `[]`, `[]=` e `length` quando o tipo estático do
//! receptor é uma lista tipada numérica (`Int8List` … `Float64List` de
//! `dart:typed_data`) ou uma `List<E>` do núcleo.
//!
//! Essas classes são `final` no SDK: o valor é sempre a lista interna
//! (`_Int32List`) ou uma visão (`_Int32ArrayView`, e a não modificável) do
//! `typed_data_patch.dart`. Então `a[i]`, `a[i] = v` e `a.length` não
//! precisam do despacho pela classe dinâmica: o comprimento e o endereço
//! dos elementos vêm de duas funções do runtime que só leem o heap dele
//! (`dartforge_typed_len`/`dartforge_typed_ptr`, funções puras do handle,
//! declaradas `memory(none) speculatable`, que o LLVM tira dos laços),
//! e o elemento é lido ou gravado direto. Um índice fora dos limites, uma
//! visão não modificável na escrita ou um tipo de elemento inesperado caem
//! no despacho de sempre, com os mesmos erros da VM.
//!
//! `List` não é `final`: uma classe do usuário pode implementá-la. As
//! listas do runtime (`_List`, `_GrowableList`, `_ImmutableList`) têm um
//! cabeçalho de endereço fixo (`heap::CabecalhoDeLista`), que
//! `dartforge_lista_cabecalho` dá (função pura do handle, fora dos laços):
//! o endereço dos elementos e o comprimento são lidos dele em linha, a cada
//! uso; as outras recebem um cabeçalho de comprimento 0 e ficam com o
//! despacho. Na escrita, o runtime confere uma vez por lista que ela é
//! modificável e compacta do escalar, e marca um bit no cabeçalho;
//! depois o código gerado só testa o bit. Os elementos saem sem caixa na representação
//! do resultado (`int`, `double`, `bool`); gravar direto só com `E` igual a
//! `int`, `double` ou `bool`, que nenhuma classe estende — com outro `E`, a
//! lista pode ser de um subtipo e o `[]=` do SDK confere o valor
//! (covariância).
//!
//! N14: a forma dos elementos está no cabeçalho (`heap::FormaDeLista`). A
//! lista cujo `E` reificado é exatamente `int`, `double` ou `bool` é
//! compacta: cada elemento são os 8 bytes dos bits (`dados + 8·i`), sem tag
//! — só o escalar do `E` entra nela, a covariância do SDK recusa o resto. A
//! lista geral guarda um `TaggedValue` de 16 bytes (os bits, e `is_ref` e a
//! tag nos bytes 8 e 9); a leitura de uma referência confere a tag. A
//! leitura confere a forma (salvo nas voltas de N13, que a conferem antes):
//! um elemento de outra forma sai por `dartforge_lista_ref`.

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
        if !self.ctx.sdk_da_fonte {
            return None;
        }
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

    /// Uma lista SIMD: o vetor sem caixa não atravessa quadros assíncronos
    /// (lá o elemento fica com o despacho).
    fn lista_simd(&self, tipo: i64, k: Type) -> Option<Indexavel> {
        self.async_estado.is_none().then_some(Indexavel::Simd { tipo, k })
    }

    /// O comprimento para o caminho rápido, ou 0 se ele não serve
    /// (`dartforge_typed_len`, ou o cabeçalho da lista do runtime).
    fn comprimento_rapido(&mut self, lista: &Operand, ix: Indexavel, escrita: bool) -> Operand {
        let (tipo, gravacao) = match ix {
            Indexavel::Tipada(ListaTipada { tipo, elemento }) => {
                let (dados, n) = self.dados_e_comprimento_tipados(lista, tipo, log2_do_elemento(elemento), escrita);
                self.dados_tipados = Some((lista.clone(), dados));
                return n;
            }
            Indexavel::Simd { tipo, .. } => {
                let (dados, n) = self.dados_e_comprimento_tipados(lista, tipo, 4, escrita);
                self.dados_tipados = Some((lista.clone(), dados));
                return n;
            }
            Indexavel::Nucleo { gravacao } => (0, gravacao),
        };
        debug_assert_eq!(tipo, 0);
        // O comprimento lógico, em linha (0 para quem não é lista do
        // runtime: o `CABECALHO_VAZIO`); numa lista fixa das voltas (N13),
        // o lido antes delas.
        let cab = self.cabecalho_da_lista(lista);
        let len = match &self.fixa_do_acesso {
            Some(f) => f.comprimento.clone(),
            None => self.campo_do_cabecalho(&cab, 1),
        };
        let Some(t) = gravacao.filter(|_| escrita) else { return len };
        // N14: nas voltas de N13, a gravação direta foi conferida antes delas.
        if let Some(n) = self.fixa_do_acesso.as_ref().and_then(|f| f.comprimento_gravavel.clone()) {
            return n;
        }
        // Gravação: o `E` reificado tem de aceitar o valor (covariância) e a
        // lista, ser modificável. Conferido uma vez por lista pelo runtime,
        // que marca o bit no cabeçalho; depois, só o bit.
        let codigo = codigo_da_forma(t);
        let gravavel = self.campo_do_cabecalho(&cab, 2);
        let bit = self.emit(Instruction::And(gravavel, Operand::Constant(Constant::Int(1 << codigo))), Type::I64);
        let conferida = self.emit(Instruction::ICmp(ICmpOp::Ne, bit, Operand::Constant(Constant::Int(0))), Type::I1);
        let direto = self.new_block();
        let conferir = self.new_block();
        let juncao = self.new_block();
        self.terminate(Terminator::CondBranch { cond: conferida, then_block: direto, else_block: conferir });
        self.set_block(direto);
        self.terminate(Terminator::Branch(juncao));
        self.set_block(conferir);
        let n = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_lista_len_gravavel".to_string(),
                args: vec![(lista.clone(), Type::Ref), (Operand::Constant(Constant::Int(codigo)), Type::I64)],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        self.terminate(Terminator::Branch(juncao));
        self.set_block(juncao);
        self.emit(Instruction::Phi { incoming: vec![(direto, len), (conferir, n)], ty: Type::I64 }, Type::I64)
    }

    /// O cabeçalho de uma lista do runtime (`heap::CabecalhoDeLista`).
    pub(super) fn cabecalho_da_lista(&mut self, lista: &Operand) -> Operand {
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_lista_cabecalho".to_string(),
                args: vec![(lista.clone(), Type::Ref)],
                ret_ty: Type::I64,
            },
            Type::I64,
        )
    }

    /// A palavra `i` do cabeçalho: 0 os dados, 1 o comprimento, 2 as
    /// gravações conferidas.
    pub(super) fn campo_do_cabecalho(&mut self, cab: &Operand, i: i64) -> Operand {
        self.emit(
            Instruction::CargaNativa { endereco: cab.clone(), indice: Operand::Constant(Constant::Int(i)), tipo: TipoC::I64 },
            Type::I64,
        )
    }

    /// `lista.length`.
    pub(super) fn length_indexado(&mut self, lista: Operand, ix: Indexavel) -> Operand {
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
    /// acesso, ou `dartforge_typed_ptr`.
    fn enderecos_tipados(&mut self, lista: &Operand) -> Operand {
        if let Some((l, d)) = &self.dados_tipados
            && l == lista
        {
            return d.clone();
        }
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_typed_ptr".to_string(),
                args: vec![(lista.clone(), Type::Ref)],
                ret_ty: Type::I64,
            },
            Type::I64,
        )
    }

    /// O endereço dos elementos e o comprimento de uma lista tipada do tipo
    /// `tipo` (elementos de `1 << log2` bytes). N17: a lista interna tem um
    /// cabeçalho de endereço fixo (`heap::CabecalhoTipado`, dado por
    /// `dartforge_typed_cabecalho`, pura do handle): o endereço e o tamanho
    /// em bytes são lidos dele em linha. Uma visão (sem cabeçalho próprio:
    /// o do runtime é o vazio, de endereço nulo) volta a
    /// `dartforge_typed_len`/`dartforge_typed_ptr`, que resolvem a base, o
    /// deslocamento e a imutabilidade. Antes eram as duas chamadas a cada
    /// acesso, cada uma com a busca do slot (~100 instruções).
    fn dados_e_comprimento_tipados(&mut self, lista: &Operand, tipo: i64, log2: i64, escrita: bool) -> (Operand, Operand) {
        let int = |x: i64| Operand::Constant(Constant::Int(x));
        let cab = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_typed_cabecalho".to_string(),
                args: vec![(lista.clone(), Type::Ref), (int(tipo), Type::I64)],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        let dados = self.emit(Instruction::CargaNativa { endereco: cab.clone(), indice: int(0), tipo: TipoC::I64 }, Type::I64);
        let bytes = self.emit(Instruction::CargaNativa { endereco: cab, indice: int(1), tipo: TipoC::I64 }, Type::I64);
        let n = self.emit(Instruction::LShr(bytes, int(log2)), Type::I64);
        let tem = self.emit(Instruction::ICmp(ICmpOp::Ne, dados.clone(), int(0)), Type::I1);
        let pelo_cabecalho = self.current_block;
        let visao = self.new_block();
        let juncao = self.new_block();
        self.terminate(Terminator::CondBranch { cond: tem, then_block: juncao, else_block: visao });
        self.set_block(visao);
        let args = vec![(lista.clone(), Type::Ref), (int(tipo), Type::I64), (int(i64::from(escrita)), Type::I64)];
        let n_visao = self.emit(
            Instruction::CallRuntime { name: "dartforge_typed_len".to_string(), args, ret_ty: Type::I64 },
            Type::I64,
        );
        let dados_visao = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_typed_ptr".to_string(),
                args: vec![(lista.clone(), Type::Ref)],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        let fim_visao = self.current_block;
        self.terminate(Terminator::Branch(juncao));
        self.set_block(juncao);
        let dados = self.emit(
            Instruction::Phi { incoming: vec![(pelo_cabecalho, dados), (fim_visao, dados_visao)], ty: Type::I64 },
            Type::I64,
        );
        let n = self.emit(Instruction::Phi { incoming: vec![(pelo_cabecalho, n), (fim_visao, n_visao)], ty: Type::I64 }, Type::I64);
        (dados, n)
    }

    /// `lista[indice]`, no resultado `repr`. `None` quando o índice não é
    /// um `int` sem caixa (fica o despacho).
    pub(super) fn ler_indexado(&mut self, lista: Operand, indice: Operand, ix: Indexavel, repr: Type) -> Option<Operand> {
        if self.operand_type(&indice) != Type::I64 {
            return None;
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
        if !direta || self.operand_type(&indice) != Type::I64 {
            return false;
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
            s.chamar_por_nome_com_receptor_tipado(
                lista2.clone(),
                true,
                super::sdk_fonte::Tipo::Chamar,
                "[]=",
                &[(None, indice2.clone()), (None, valor.clone())],
            );
            Operand::Constant(Constant::Int(0))
        };
        self.desviar_indexado(&lista, &indice, ix, true, Type::I64, &mut rapido, &mut lento);
        true
    }

    /// O endereço dos elementos de uma lista do runtime apta, do cabeçalho.
    fn dados_da_lista(&mut self, lista: &Operand) -> Operand {
        if let Some(f) = &self.fixa_do_acesso {
            return f.dados.clone();
        }
        let cab = self.cabecalho_da_lista(lista);
        self.campo_do_cabecalho(&cab, 0)
    }

    /// `indice * k + d`, em `int`.
    fn escala(&mut self, indice: &Operand, k: i64, d: i64) -> Operand {
        let m = self.emit(Instruction::Mul(indice.clone(), Operand::Constant(Constant::Int(k))), Type::I64);
        if d == 0 {
            return m;
        }
        self.emit(Instruction::Add(m, Operand::Constant(Constant::Int(d))), Type::I64)
    }

    /// `dartforge_lista_ref(lista, indice)`: o elemento numa posição `Ref`.
    fn elemento_ref(&mut self, lista: &Operand, indice: &Operand) -> Operand {
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_lista_ref".to_string(),
                args: vec![(lista.clone(), Type::Ref), (indice.clone(), Type::I64)],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        )
    }

    /// A forma dos elementos de uma lista do runtime (`heap::FormaDeLista`):
    /// a constante conferida antes das voltas de N13, ou lida do cabeçalho.
    fn forma_da_lista(&mut self, lista: &Operand) -> Result<i64, Operand> {
        if let Some(f) = &self.fixa_do_acesso {
            return Ok(f.forma);
        }
        let cab = self.cabecalho_da_lista(lista);
        Err(self.campo_do_cabecalho(&cab, 3))
    }

    /// O elemento `indice` (já conferido) de uma lista do runtime, na
    /// representação `repr`.
    ///
    /// Numa lista compacta (N14) da forma de `repr` (`int`, `double`,
    /// `bool`), os 8 bytes dos bits em `dados + 8·indice`; numa lista geral
    /// lida como `Ref`, o `TaggedValue` de 16 bytes quando a tag é de
    /// referência. O resto sai pela caixa (`dartforge_lista_ref`: um escalar
    /// numa posição `Ref` vira `Smi` ou caixa), e a forma conhecida na
    /// emissão (as voltas de N13) poupa a conferência.
    pub(super) fn ler_elemento_da_lista(&mut self, lista: &Operand, indice: &Operand, repr: Type) -> Operand {
        let alvo = match repr {
            Type::I64 | Type::F64 | Type::I1 => codigo_da_forma(repr),
            Type::Ref => 0,
            _ => return self.elemento_ref(lista, indice),
        };
        let dados = self.dados_da_lista(lista);
        let forma = self.forma_da_lista(lista);
        // A lista geral lida como escalar (a forma 0 conhecida nas voltas):
        // os 16 bytes com a tag, como antes das listas compactas.
        let geral = alvo == 0 || forma == Ok(0);
        if let Ok(k) = forma
            && k != alvo
            && k != 0
        {
            let r = self.elemento_ref(lista, indice);
            return self.coagir(r, repr);
        }
        let mut caixa = None;
        if let Err(f) = forma {
            let ok = self.emit(Instruction::ICmp(ICmpOp::Eq, f, Operand::Constant(Constant::Int(alvo))), Type::I1);
            let segue = self.new_block();
            let b = self.new_block();
            self.terminate(Terminator::CondBranch { cond: ok, then_block: segue, else_block: b });
            self.set_block(segue);
            caixa = Some(b);
        }
        let (tipo, tag) = match repr {
            Type::F64 => (TipoC::F64, TAG_DOUBLE),
            Type::Ref => (TipoC::I64, TAG_REF),
            Type::I1 => (TipoC::I64, TAG_BOOL),
            _ => (TipoC::I64, TAG_INT),
        };
        let i_bits = if geral {
            let i_tag = self.escala(indice, 16, 9);
            let t = self.emit(Instruction::CargaNativa { endereco: dados.clone(), indice: i_tag, tipo: TipoC::U8 }, Type::I64);
            let ok = self.emit(Instruction::ICmp(ICmpOp::Eq, t, Operand::Constant(Constant::Int(tag))), Type::I1);
            let direto = self.new_block();
            let b = *caixa.get_or_insert_with(|| self.new_block());
            self.terminate(Terminator::CondBranch { cond: ok, then_block: direto, else_block: b });
            self.set_block(direto);
            self.escala(indice, 2, 0)
        } else {
            indice.clone()
        };
        let v = if repr == Type::I1 {
            let b = self.emit(Instruction::CargaNativa { endereco: dados, indice: i_bits, tipo }, Type::I64);
            self.emit(Instruction::ICmp(ICmpOp::Ne, b, Operand::Constant(Constant::Int(0))), Type::I1)
        } else {
            self.emit(Instruction::CargaNativa { endereco: dados, indice: i_bits, tipo }, repr)
        };
        let Some(caixa) = caixa else { return v };
        let fim_direto = self.current_block;
        let juncao = self.new_block();
        self.terminate(Terminator::Branch(juncao));

        self.set_block(caixa);
        let r = self.elemento_ref(lista, indice);
        let r = self.coagir(r, repr);
        let fim_caixa = self.current_block;
        let caixa_chega = !self.is_terminated();
        if caixa_chega {
            self.terminate(Terminator::Branch(juncao));
        }

        self.set_block(juncao);
        if caixa_chega {
            self.emit(Instruction::Phi { incoming: vec![(fim_direto, v), (fim_caixa, r)], ty: repr }, repr)
        } else {
            v
        }
    }

    /// Grava `valor` (`int`, `double` ou `bool`, sem caixa) no elemento
    /// `indice` (já conferido) de uma lista do runtime cuja gravação direta
    /// foi conferida (`dartforge_lista_len_gravavel`): a lista é compacta da
    /// forma de `repr` (N14), e o elemento são os 8 bytes dos bits.
    fn gravar_elemento_da_lista(&mut self, lista: &Operand, indice: &Operand, valor: Operand, repr: Type) {
        let (tipo, valor) = match repr {
            Type::F64 => (TipoC::F64, valor),
            Type::I1 => {
                let b = self.emit(Instruction::ZExt { op: valor, from: Type::I1, to: Type::I64 }, Type::I64);
                (TipoC::I64, b)
            }
            _ => (TipoC::I64, valor),
        };
        let dados = self.dados_da_lista(lista);
        self.emit(Instruction::GravacaoNativa { endereco: dados, indice: indice.clone(), tipo, valor }, Type::Void);
    }
}

/// O código da forma compacta (`heap::FormaDeLista`) dos elementos de
/// representação `t`: 1 `int`, 2 `double`, 3 `bool` — o mesmo das
/// gravações diretas.
pub(super) fn codigo_da_forma(t: Type) -> i64 {
    match t {
        Type::I64 => 1,
        Type::F64 => 2,
        _ => 3,
    }
}

/// As tags de `ValueTag` do runtime (`heap.rs`, `#[repr(u8)]`).
const TAG_INT: i64 = 0;
const TAG_BOOL: i64 = 1;
const TAG_DOUBLE: i64 = 2;
const TAG_REF: i64 = 3;

/// O `log2` do tamanho em bytes de um elemento de lista tipada numérica.
fn log2_do_elemento(e: TipoC) -> i64 {
    match e {
        TipoC::I8 | TipoC::U8 | TipoC::Bool => 0,
        TipoC::I16 | TipoC::U16 => 1,
        TipoC::I32 | TipoC::U32 | TipoC::F32 => 2,
        _ => 3,
    }
}

//! Variáveis locais: escopo léxico e `alloca` na representação do tipo (R6),
//! e as variáveis capturadas por closures (P1).
//!
//! Antes, um local era um `alloca i64` criado onde a declaração aparecia (no
//! meio de um laço, o `alloca` crescia a pilha a cada volta) e um mapa único
//! por nome: um bloco que sombreava `current` sobrescrevia o `current` de
//! fora para sempre, e a variável de um `for (var i = 0; …; i++)` nem tinha
//! `alloca` — o `i++` trocava o valor SSA no mapa, e a condição do laço,
//! baixada antes, continuava lendo o `0` da entrada.
//!
//! Uma variável capturada por uma closure e **atribuída** em algum ponto
//! (`captura.rs` decide, antes de baixar a função) mora numa `Cell` do heap: o
//! `alloca` guarda o handle da célula, e quem lê ou grava passa por ela — a
//! closure vê a gravação feita depois da captura, e a gravação dela é vista
//! de fora. Capturada e nunca atribuída, ela é copiada para o ambiente da
//! closure na criação. Dentro da closure, a variável de fora é uma posição do
//! ambiente (`Ambiente`), com o handle da célula ou a cópia.

use super::fn_builder::FnBuilder;
use crate::hir::*;
use dartforge_elements::model::UnitId;
use dartforge_frontend::ast::ExprId;
use dartforge_intern::SymbolId;
use std::collections::HashMap;

/// Identidade semântica preservada ao retirar/restaurar casos de switch.
/// Zero é a invocação; os demais IDs não são profundidades reutilizáveis.
#[derive(Debug, Clone)]
pub(super) struct EscopoLocal {
    pub id: u32,
    locais: HashMap<SymbolId, Local>,
}

impl EscopoLocal {
    pub(super) fn novo(id: u32) -> Self {
        Self {
            id,
            locais: HashMap::new(),
        }
    }
}

impl std::ops::Deref for EscopoLocal {
    type Target = HashMap<SymbolId, Local>;
    fn deref(&self) -> &Self::Target {
        &self.locais
    }
}

impl std::ops::DerefMut for EscopoLocal {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.locais
    }
}

/// Onde um local visível mora.
#[derive(Debug, Clone)]
pub enum Modo {
    /// `alloca` na representação do local.
    Memoria(Operand),
    /// Valor imutável já calculado (o parâmetro de uma closure baixada em
    /// linha pelo código congelado de `sdk_por_nome.rs`).
    Valor(Operand),
    /// `alloca` `Ref` com o handle da célula que guarda o valor.
    Celula(Operand),
    /// Posição `indice` do ambiente da closure corrente (`env`); com
    /// `celula`, o que está lá é o handle da célula.
    Ambiente {
        env: Operand,
        indice: usize,
        celula: bool,
    },
}

/// Um local visível.
#[derive(Debug, Clone)]
pub struct Local {
    pub modo: Modo,
    pub ty: Type,
    /// Tipo Dart antes de apagar a representação, para obrigações léxicas
    /// como Finalizable. None indica informação ainda não fornecida; não
    /// prova ausência da obrigação. Capturas preservam o TypeId original.
    pub tipo_estatico: Option<dartforge_types::TypeId>,
    /// Classificação da obrigação léxica. None ainda exige prova; não emite
    /// nem substitui ArcKeepAlive ou uma ocorrência proprietária.
    pub finalizavel: Option<bool>,
    /// Escopo desta ligação, atribuído ao inseri-la no corpo corrente.
    /// Capturas preservam o tipo original, mas não o escopo de outra função.
    /// Não é uma prova de cleanup nem uma ocorrência proprietária.
    pub escopo: u32,
    /// Offset do nome na declaração (a chave de `captura.rs`); `None` para
    /// os ligados por valor.
    pub offset: Option<usize>,
    /// Marca separada do valor, pois zero/null são atribuições legítimas.
    pub late: Option<LateLocal>,
}

#[derive(Debug, Clone)]
pub struct LateLocal {
    pub inicializado: Operand,
    /// `true`: `inicializado` é uma `Cell` do heap e usa índice -1 na
    /// tabela lateral; `false`: é um `alloca i8` desta função.
    pub celula: bool,
    pub final_: bool,
    pub nome: String,
    pub inicializador: Option<(UnitId, ExprId)>,
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    pub fn abrir_escopo(&mut self) {
        let id = self.proximo_escopo;
        self.proximo_escopo = id.checked_add(1).expect("limite de escopos léxicos");
        debug_assert!(self.escopos.iter().all(|e| e.id != id));
        self.escopos.push(EscopoLocal::novo(id));
    }

    pub fn fechar_escopo(&mut self) {
        if self.escopos.len() > 1 {
            self.escopos.pop();
        }
    }

    /// `alloca` no começo do bloco de entrada: executa uma vez por
    /// ativação e domina todos os usos (o Clang recusava `alloca` criado
    /// num ramo e usado depois da junção).
    pub fn alloca_na_entrada(&mut self, ty: Type) -> Operand {
        let vid = ValueId(self.next_value);
        self.next_value += 1;
        self.value_types.insert(vid, Type::Ptr);
        let b0 = self
            .func
            .blocks
            .iter()
            .position(|b| b.id == BlockId(0))
            .expect("bloco de entrada");
        self.func.blocks[b0]
            .instructions
            .insert(self.n_allocas, (vid, Instruction::Alloca(ty), Type::Ptr));
        self.n_allocas += 1;
        Operand::Val(vid)
    }

    fn repr_de_local(ty: Type) -> Type {
        if matches!(ty, Type::Void | Type::I8 | Type::Ptr) {
            Type::Ref
        } else {
            ty
        }
    }

    fn inserir_local(&mut self, sym: SymbolId, mut local: Local) {
        let escopo = self.escopos.last_mut().expect("escopo da função");
        local.escopo = escopo.id;
        escopo.insert(sym, local);
    }

    /// Declara um local (sem offset: nunca é célula) no escopo corrente,
    /// guardado na representação `ty`.
    pub fn declarar_local(&mut self, sym: SymbolId, ty: Type) -> Operand {
        let ty = Self::repr_de_local(ty);
        let ptr = self.alloca_na_entrada(ty);
        self.inserir_local(
            sym,
            Local {
                modo: Modo::Memoria(ptr.clone()),
                ty,
                tipo_estatico: None,
                finalizavel: None,
                escopo: 0,
                offset: None,
                late: None,
            },
        );
        ptr
    }

    /// Declara e inicializa: o valor é coagido para a representação do local.
    pub fn declarar_local_com_valor(&mut self, sym: SymbolId, ty: Type, valor: Operand) {
        let ptr = self.declarar_local(sym, ty);
        let ty = self.buscar_local(sym).map_or(ty, |l| l.ty);
        let v = self.coagir(valor, ty);
        self.emit(Instruction::Store { ptr, val: v }, Type::Void);
    }

    /// Declara a variável do usuário cujo nome começa em `offset` e a
    /// inicializa com `valor`. Se `captura.rs` a marcou (capturada e
    /// atribuída), ela mora numa célula nova — cada execução da declaração
    /// cria uma variável nova (a do corpo de um laço, uma por volta).
    pub fn declarar_variavel(&mut self, sym: SymbolId, offset: usize, ty: Type, valor: Operand) {
        let tipo_estatico = self.ctx.tipo_local_semantico(self.unit_id, offset);
        let finalizavel = tipo_estatico.and_then(|t| self.ctx.classificar_finalizavel(t));
        // Outra declaração do mesmo nome no mesmo escopo: o programa é
        // inválido (a de um escopo de fora pode ser sombreada; a mesma
        // declaração baixada de novo, num `finally`, não conta).
        // Com curingas (Dart 3.7), `_` pode repetir: não liga nome.
        let biblioteca = self.ctx.program.unit(self.unit_id).library;
        let curinga = self.ctx.symbol_name(sym) == "_"
            && self.ctx.program.library(biblioteca).features.tem(dartforge_frontend::Feature::WildcardVariables);
        let repetida = !curinga
            && self.escopos.last().and_then(|e| e.get(&sym)).is_some_and(|l| l.offset.is_some_and(|o| o != offset));
        if repetida {
            let nome = self.ctx.symbol_name(sym).to_string();
            let fim = offset + nome.len();
            self.erro_de_linguagem(&format!("'{nome}' is already declared in this scope."), dartforge_diagnostics::Span { start: offset, end: fim });
        }
        let ty = Self::repr_de_local(ty);
        // Capturada só por funções diretas e atribuída: vai pelo endereço do
        // `alloca` (`funcoes_diretas.rs`); o de um `Ref` mora no slot do
        // quadro de raízes (`llvm/mod.rs`, `allocas_no_quadro`), onde o
        // coletor vê o que a função direta grava.
        let celula = self.celulas.contains(&offset);
        if !celula {
            let ptr = self.alloca_na_entrada(ty);
            let v = self.coagir(valor, ty);
            self.emit(
                Instruction::Store {
                    ptr: ptr.clone(),
                    val: v,
                },
                Type::Void,
            );
            self.inserir_local(
                sym,
                Local {
                    modo: Modo::Memoria(ptr),
                    ty,
                    tipo_estatico,
                    finalizavel,
                    escopo: 0,
                    offset: Some(offset),
                    late: None,
                },
            );
            return;
        }
        // A célula guarda uma palavra, na representação do local (a mesma
        // com que `CellGet`/`CellSet` a leem e gravam); um vetor SIMD vai na
        // caixa.
        let ty = if ty.e_vetor() { Type::Ref } else { ty };
        let ptr = self.alloca_na_entrada(Type::Ref);
        let v = self.coagir(valor, ty);
        let celula = self.emit(Instruction::AllocCell { value: v }, Type::Ref);
        self.emit(
            Instruction::Store {
                ptr: ptr.clone(),
                val: celula,
            },
            Type::Void,
        );
        self.inserir_local(
            sym,
            Local {
                modo: Modo::Celula(ptr),
                ty,
                tipo_estatico,
                finalizavel,
                escopo: 0,
                offset: Some(offset),
                late: None,
            },
        );
    }

    /// Ativa a semântica de `late`. O estado de um local capturado segue o
    /// handle da sua `Cell`, compartilhado entre função e closures.
    pub fn configurar_local_late(&mut self, sym: SymbolId, final_: bool, init: Option<ExprId>) -> bool {
        let Some(local) = self.buscar_local(sym) else { return false };
        let (estado, celula) = match &local.modo {
            Modo::Memoria(_) => {
                let estado = self.alloca_na_entrada(Type::I8);
                self.emit(
                    Instruction::Store { ptr: estado.clone(), val: Operand::Constant(Constant::Int(0)) },
                    Type::Void,
                );
                (estado, false)
            }
            Modo::Celula(_) if init.is_none() => (self.celula_do_local(&local).expect("célula late"), true),
            _ => return false,
        };
        let nome = self.ctx.symbol_name(sym).to_string();
        if let Some(local) = self.escopos.last_mut().and_then(|e| e.get_mut(&sym)) {
            local.late = Some(LateLocal {
                inicializado: estado,
                celula,
                final_,
                nome,
                inicializador: init.map(|e| (self.unit_id, e)),
            });
        }
        true
    }

    fn ler_estado_late(&mut self, late: &LateLocal) -> Operand {
        if late.celula {
            self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_late_field_initialized".to_string(),
                    args: vec![
                        (late.inicializado.clone(), Type::Ref),
                        (Operand::Constant(Constant::Int(-1)), Type::I64),
                    ],
                    ret_ty: Type::I8,
                },
                Type::I8,
            )
        } else {
            self.emit(Instruction::Load { ptr: late.inicializado.clone(), ty: Type::I8 }, Type::I8)
        }
    }

    fn definir_estado_late(&mut self, late: &LateLocal, estado: i64) {
        if late.celula {
            assert_eq!(estado, 1, "célula late capturada não tem inicializador preguiçoso");
            self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_late_field_mark_initialized".to_string(),
                    args: vec![
                        (late.inicializado.clone(), Type::Ref),
                        (Operand::Constant(Constant::Int(-1)), Type::I64),
                    ],
                    ret_ty: Type::Void,
                },
                Type::Void,
            );
        } else {
            self.emit(
                Instruction::Store { ptr: late.inicializado.clone(), val: Operand::Constant(Constant::Int(estado)) },
                Type::Void,
            );
        }
    }

    /// Liga um nome a um posição do ambiente da closure corrente.
    pub fn ligar_ambiente(
        &mut self,
        sym: SymbolId,
        env: Operand,
        indice: usize,
        celula: bool,
        ty: Type,
        origem: &Local,
    ) {
        let late = origem.late.as_ref().filter(|l| l.celula && celula).map(|l| {
            let estado = self.emit(Instruction::EnvGet { env: env.clone(), index: indice }, Type::Ref);
            LateLocal {
                inicializado: estado,
                celula: true,
                final_: l.final_,
                nome: l.nome.clone(),
                inicializador: None,
            }
        });
        self.inserir_local(
            sym,
            Local {
                modo: Modo::Ambiente {
                    env,
                    indice,
                    celula,
                },
                ty,
                tipo_estatico: origem.tipo_estatico,
                finalizavel: origem.finalizavel,
                escopo: 0,
                offset: None,
                late,
            },
        );
    }

    /// Liga um nome a um local já montado (as capturas de uma função
    /// direta, `funcoes_diretas.rs`).
    pub(super) fn ligar_local_como(&mut self, sym: SymbolId, local: Local) {
        self.inserir_local(sym, local);
    }

    /// Liga um nome a um valor já calculado (parâmetro de closure em linha).
    pub fn ligar_local(&mut self, sym: SymbolId, valor: Operand) {
        let ty = self.operand_type(&valor);
        self.inserir_local(
            sym,
            Local {
                modo: Modo::Valor(valor),
                ty,
                tipo_estatico: None,
                finalizavel: None,
                escopo: 0,
                offset: None,
                late: None,
            },
        );
    }

    /// Completa o tipo de um local já ligado usando a origem semântica.
    /// None mantém a obrigação indeterminada; não infere pelo tipo HIR.
    ///
    /// # Panics
    /// Se o chamador não ligou o local no escopo visível antes desta atualização.
    pub(super) fn atribuir_tipo_semantico(&mut self, sym: SymbolId, tipo: Option<dartforge_types::TypeId>) {
        let finalizavel = tipo.and_then(|t| self.ctx.classificar_finalizavel(t));
        let local = self.escopos.iter_mut().rev().find_map(|e| e.get_mut(&sym))
            .expect("local semântico deve estar ligado");
        local.tipo_estatico = tipo;
        local.finalizavel = finalizavel;
    }

    /// O local visível com esse nome, do escopo mais interno para fora.
    pub fn buscar_local(&self, sym: SymbolId) -> Option<Local> {
        self.escopos.iter().rev().find_map(|e| e.get(&sym).cloned())
    }

    /// O handle da célula de um local que mora numa (declarado aqui ou
    /// vindo do ambiente); `None` se o local não é célula.
    pub fn celula_do_local(&mut self, local: &Local) -> Option<Operand> {
        match &local.modo {
            Modo::Celula(ptr) => Some(self.emit(
                Instruction::Load {
                    ptr: ptr.clone(),
                    ty: Type::Ref,
                },
                Type::Ref,
            )),
            Modo::Ambiente {
                env,
                indice,
                celula: true,
            } => Some(self.emit(
                Instruction::EnvGet {
                    env: env.clone(),
                    index: *indice,
                },
                Type::Ref,
            )),
            _ => None,
        }
    }

    /// Lê um local já encontrado, na representação dele.
    pub fn ler_local(&mut self, local: &Local) -> Operand {
        if let Some(late) = &local.late {
            if let Some((unit, init)) = late.inicializador
                && !local.offset.is_some_and(|off| self.late_inicializadores_em_lowering.contains(&off))
            {
                let estado = self.ler_estado_late(late);
                let vazio = self.emit(
                    Instruction::ICmp(ICmpOp::Eq, estado, Operand::Constant(Constant::Int(0))),
                    Type::I1,
                );
                let b_init = self.new_block();
                let b_verificar = self.new_block();
                self.terminate(Terminator::CondBranch { cond: vazio, then_block: b_init, else_block: b_verificar });
                self.set_block(b_init);
                self.definir_estado_late(late, 2);
                let b_falha = self.new_block();
                self.exception_targets.push(b_falha);
                if let Some(off) = local.offset {
                    self.late_inicializadores_em_lowering.insert(off);
                }
                let valor = self.lower_expr_de(unit, init);
                if let Some(off) = local.offset {
                    self.late_inicializadores_em_lowering.remove(&off);
                }
                self.exception_targets.pop();
                let valor = self.coagir(valor, local.ty);
                let Modo::Memoria(ptr) = &local.modo else { unreachable!("late capturado recusado") };
                self.emit(Instruction::Store { ptr: ptr.clone(), val: valor }, Type::Void);
                self.definir_estado_late(late, 1);
                self.terminate(Terminator::Branch(b_verificar));
                self.set_block(b_falha);
                self.definir_estado_late(late, 0);
                // Encaminha a exceção ao `catch`/`finally` externo após
                // restaurar a célula para permitir nova tentativa de leitura.
                self.emit_call_with_check(
                    Instruction::CallRuntime {
                        name: "dartforge_exception_pending".to_string(),
                        args: Vec::new(),
                        ret_ty: Type::I8,
                    },
                    Type::I8,
                );
                self.terminate(Terminator::Unreachable);
                self.set_block(b_verificar);
            }
            let estado = self.ler_estado_late(late);
            let pronto = self.emit(
                Instruction::ICmp(ICmpOp::Eq, estado, Operand::Constant(Constant::Int(1))),
                Type::I1,
            );
            let b_erro = self.new_block();
            let b_ler = self.new_block();
            self.terminate(Terminator::CondBranch { cond: pronto, then_block: b_ler, else_block: b_erro });
            self.set_block(b_erro);
            self.lancar_erro_late(&late.nome, 2);
            self.set_block(b_ler);
        }
        if let Some(c) = self.celula_do_local(local) {
            return self.emit(Instruction::CellGet { cell: c }, local.ty);
        }
        match &local.modo {
            Modo::Memoria(ptr) => self.emit(
                Instruction::Load {
                    ptr: ptr.clone(),
                    ty: local.ty,
                },
                local.ty,
            ),
            Modo::Valor(v) => v.clone(),
            Modo::Ambiente { env, indice, .. } => self.emit(
                Instruction::EnvGet {
                    env: env.clone(),
                    index: *indice,
                },
                local.ty,
            ),
            Modo::Celula(_) => unreachable!("célula tratada acima"),
        }
    }

    /// Lê um local (ou parâmetro) pelo nome, na representação dele.
    pub fn ler_local_por_nome(&mut self, sym: SymbolId) -> Option<Operand> {
        let local = self.buscar_local(sym)?;
        Some(self.ler_local(&local))
    }

    /// Grava num local visível; devolve o valor já na representação dele.
    /// Um local copiado para o ambiente nunca é gravado (`captura.rs` põe
    /// numa célula toda variável capturada que é atribuída).
    pub fn gravar_local(&mut self, sym: SymbolId, valor: Operand) -> Option<Operand> {
        let local = self.buscar_local(sym)?;
        let v = self.coagir(valor, local.ty);
        if let Some(late) = &local.late {
            let estado = self.ler_estado_late(late);
            let iniciando = self.emit(
                Instruction::ICmp(ICmpOp::Eq, estado.clone(), Operand::Constant(Constant::Int(2))),
                Type::I1,
            );
            let b_adi = self.new_block();
            let b_proximo = self.new_block();
            self.terminate(Terminator::CondBranch { cond: iniciando, then_block: b_adi, else_block: b_proximo });
            self.set_block(b_adi);
            self.lancar_erro_late(&late.nome, 5);
            self.set_block(b_proximo);
            if late.final_ {
                let ja_inicializado = self.emit(
                    Instruction::ICmp(ICmpOp::Ne, estado, Operand::Constant(Constant::Int(0))),
                    Type::I1,
                );
                let b_erro = self.new_block();
                let b_gravar = self.new_block();
                self.terminate(Terminator::CondBranch { cond: ja_inicializado, then_block: b_erro, else_block: b_gravar });
                self.set_block(b_erro);
                self.lancar_erro_late(&late.nome, 3);
                self.set_block(b_gravar);
            }
            self.definir_estado_late(late, 1);
        }
        if let Some(c) = self.celula_do_local(&local) {
            self.emit(
                Instruction::CellSet {
                    cell: c,
                    value: v.clone(),
                },
                Type::Void,
            );
            return Some(v);
        }
        match local.modo {
            Modo::Memoria(ptr) => {
                self.emit(
                    Instruction::Store {
                        ptr,
                        val: v.clone(),
                    },
                    Type::Void,
                );
            }
            Modo::Valor(_) => {
                // Valor imutável religado no escopo onde ele mora.
                for e in self.escopos.iter_mut().rev() {
                    if let Some(l) = e.get_mut(&sym) {
                        l.modo = Modo::Valor(v.clone());
                        break;
                    }
                }
            }
            Modo::Ambiente { .. } | Modo::Celula(_) => return None,
        }
        Some(v)
    }

    /// Uma volta nova de um `for` clássico (a especificação, "for loop":
    /// cada iteração tem a sua variável, iniciada com o valor da anterior).
    /// Só importa para a variável que mora numa célula — uma closure da
    /// volta anterior continua com a célula antiga.
    pub fn renovar_celula(&mut self, sym: SymbolId) {
        let Some(local) = self.buscar_local(sym) else {
            return;
        };
        let Modo::Celula(ptr) = &local.modo else {
            return;
        };
        let antiga = self.emit(
            Instruction::Load {
                ptr: ptr.clone(),
                ty: Type::Ref,
            },
            Type::Ref,
        );
        let valor = self.emit(Instruction::CellGet { cell: antiga }, local.ty);
        let nova = self.emit(Instruction::AllocCell { value: valor }, Type::Ref);
        self.emit(
            Instruction::Store {
                ptr: ptr.clone(),
                val: nova,
            },
            Type::Void,
        );
    }

    /// Tira um nome de todos os escopos (os `this.x` no corpo do construtor).
    pub fn remover_local(&mut self, sym: SymbolId) {
        for e in &mut self.escopos {
            e.remove(&sym);
        }
    }

    /// Representação guardada de um local declarado em `offset` (R6): o
    /// tipo declarado, ou o inferido do inicializador. Sem informação, `Ref`
    /// — a caixa é sempre correta, só mais lenta.
    pub fn repr_do_local(&self, offset: usize) -> Type {
        let tipo = self.ctx.tipo_local(self.unit_id, offset);
        // SIMD sem caixa (`simd.rs`), salvo o local capturado (a célula
        // guarda 64 bits: a caixa).
        if let Some(k) = self.tipo_simd(tipo)
            && !self.celulas.contains(&offset)
        {
            return k;
        }
        tipo.map_or(Type::Ref, |t| self.repr(t))
    }
}

#[cfg(test)]
mod testes_finalizavel_semantico {
    use super::*;
    use dartforge_intern::Interner;
    use dartforge_types::table::{CoreTypes, TypeTable};

    #[test]
    fn parametros_e_this_preservam_identidade_semantica_do_sdk() {
        std::thread::Builder::new()
            .stack_size(64 << 20)
            .spawn(|| {
                let dir = tempfile::tempdir().unwrap();
                let entrada = dir.path().join("main.dart");
                std::fs::write(
                    &entrada,
                    r#"
import 'dart:ffi' as ffi;
final class Recurso implements ffi.Finalizable {
  void usar(Recurso? recurso) {}
}
class Finalizable {}
extension type Envelope(Recurso recurso) implements Recurso {}
void testar(Envelope envelope, Recurso? recurso, Finalizable homonimo,
    dynamic outro, Never nunca) {}
int escalares<T>(int numero, double fracao, bool ligado, T valor) => numero;
void main() {}
"#,
                )
                .unwrap();
                let sdk = crate::sdk_modulo::carregar_sdk_nativo(crate::sdk_testes()).unwrap();
                let mut interner = Interner::new();
                let (program, diags) =
                    dartforge_elements::load::load_lenient(&entrada, &sdk, None, &mut interner);
                assert!(diags.is_empty(), "{diags:?}");
                let mut table = TypeTable::new();
                let core = CoreTypes::init(&mut table, &program, &interner);
                let (mut outline, diags_outline) =
                    dartforge_types::resolve_outline(&program, &interner, &mut table, &core);
                let (bodies, diags_body) = dartforge_types::infer_program_bodies(
                    &program,
                    &interner,
                    &mut table,
                    &core,
                    &mut outline,
                );
                for d in diags_outline.iter().chain(diags_body.iter()) {
                    assert!(
                        !dartforge_types::codes::e_erro_de_compilacao(&d.message),
                        "{d:?}"
                    );
                }
                let te = crate::apagamento::calcular(&program, &outline, &mut table);
                let mut ctx = crate::context::Context::new(
                    &program, &interner, &table, &core, &outline, &bodies,
                );
                ctx.te = te;
                let lib = program.entry.unwrap();
                let unit = program.library(lib).units[0];
                for nome in ["testar", "usar"] {
                    let fid = program
                        .functions
                        .iter()
                        .position(|f| f.library == lib && interner.resolve(f.name) == nome)
                        .unwrap();
                    let mut b = FnBuilder::new(&ctx, unit, nome.into(), nome.into(), Type::Void);
                    b.declarar_parametros(fid, nome == "usar");
                    if nome == "usar" {
                        assert_eq!(b.this_finalizavel, Some(true));
                    }
                    for p in &outline.functions[fid].parameters {
                        let sym = p.name.unwrap();
                        let nome = interner.resolve(sym);
                        let local = b.buscar_local(sym).unwrap();
                        assert_eq!(local.escopo, 0, "parâmetro {nome}");
                        assert_eq!(local.tipo_estatico, Some(p.ty), "{nome}");
                        assert_eq!(
                            local.finalizavel,
                            Some(matches!(nome, "envelope" | "recurso")),
                            "{nome}"
                        );
                        if nome == "envelope" {
                            assert_ne!(
                                ctx.apagar(p.ty),
                                p.ty,
                                "tipo de extensão precisa ser preservado antes do apagamento"
                            );
                        }
                    }
                    // Mesma profundidade não identifica o mesmo escopo.
                    // Casos de switch restauram o registro original completo.
                    let parametro = outline.functions[fid].parameters[0].name.unwrap();
                    assert_eq!(b.escopos[0].id, 0);
                    let externo = b.buscar_local(parametro).unwrap();
                    b.abrir_escopo();
                    let primeiro = b.escopos.last().unwrap().id;
                    b.ligar_local(parametro, Operand::Constant(Constant::Null));
                    assert_eq!(b.buscar_local(parametro).unwrap().escopo, primeiro);
                    let salvo = b.escopos.pop().unwrap();
                    assert_eq!(
                        b.buscar_local(parametro).unwrap().tipo_estatico,
                        externo.tipo_estatico
                    );
                    b.abrir_escopo();
                    let segundo = b.escopos.last().unwrap().id;
                    assert_ne!(primeiro, segundo);
                    b.fechar_escopo();
                    b.escopos.push(salvo.clone());
                    assert_eq!(b.escopos.last().unwrap().id, primeiro);
                    assert_eq!(b.buscar_local(parametro).unwrap().escopo, primeiro);
                    assert_eq!(
                        b.buscar_local(parametro).unwrap().tipo_estatico,
                        None
                    );
                    b.fechar_escopo();
                    b.abrir_escopo();
                    let terceiro = b.escopos.last().unwrap().id;
                    assert!(terceiro > segundo);
                    // Copiar a descrição de uma captura não transporta a
                    // identidade léxica do corpo que a declarou.
                    let mut captura = externo.clone();
                    captura.escopo = primeiro;
                    b.ligar_local_como(parametro, captura);
                    assert_eq!(b.buscar_local(parametro).unwrap().escopo, terceiro);
                    assert_eq!(
                        b.buscar_local(parametro).unwrap().tipo_estatico,
                        externo.tipo_estatico
                    );
                    b.ligar_ambiente(
                        parametro,
                        Operand::Constant(Constant::Null),
                        0,
                        false,
                        externo.ty,
                        &externo,
                    );
                    let ambiente = b.buscar_local(parametro).unwrap();
                    assert_eq!(ambiente.escopo, terceiro);
                    assert_eq!(ambiente.tipo_estatico, externo.tipo_estatico);
                    assert_eq!(ambiente.finalizavel, externo.finalizavel);
                    b.fechar_escopo();
                    assert_eq!(b.buscar_local(parametro).unwrap().escopo, 0);
                    b.fechar_escopo();
                    assert_eq!(b.escopos.len(), 1);
                }
                let fid = program.functions.iter().position(|f| ctx.symbol_name(f.name) == "escalares").unwrap();
                let mut tracing = FnBuilder::new(&ctx, unit, "tracing".into(), "tracing".into(), Type::I64);
                tracing.declarar_parametros(fid, false);
                assert!(tracing.parametros_escalares_dart.is_empty());
                tracing.add_param_rti("$tipos".into());
                assert!(tracing.parametros_rti_dart.is_empty());
                drop(tracing);
                ctx.memoria_arc = true;
                let mut b = FnBuilder::new(&ctx, unit, "escalares".into(), "escalares".into(), Type::I64);
                b.declarar_parametros(fid, false);
                let nativo = b.add_param_rti("$tipos".into());
                assert!(b.parametros_rti_dart["escalares"].contains(&nativo));
                let escalares = &b.parametros_escalares_dart["escalares"];
                for (v, _, ty) in &b.func.params {
                    assert_eq!(escalares.contains(v), *v != nativo && *ty != Type::Ref);
                }
                assert_eq!(escalares.len(), 3);
                b.terminate(Terminator::Return(Some(Operand::Val(b.func.params[0].0))));
                let mut modulo = Module::new();
                b.finalizar(&mut modulo);
                assert_eq!(modulo.parametros_escalares_dart["escalares"].len(), 3);
                assert!(!modulo.parametros_escalares_dart["escalares"].contains(&nativo));
                assert!(modulo.parametros_rti_dart["escalares"].contains(&nativo));
                crate::otimizar::otimizar(&mut modulo);
                assert_eq!(modulo.parametros_escalares_dart["escalares"].len(), 3);
                for id in &modulo.parametros_escalares_dart["escalares"] {
                    assert!(modulo.functions[0].params.iter().any(|(v, _, _)| v == id));
                }
                modulo.memoria_arc = true;
                let mut planos = std::collections::HashMap::from([("escalares".into(), crate::otimizar::arc::PlanoFuncaoDart::default())]);
                assert_eq!(crate::otimizar::arc::produzir_parametros_escalares_do_lowering(&modulo, &mut planos).unwrap(), 3);
                assert!(!planos["escalares"].classes.contains_key(&nativo));
                assert_eq!(crate::otimizar::arc::produzir_parametros_escalares_do_lowering(&modulo, &mut planos).unwrap(), 0);
                assert!(crate::otimizar::arc::inserir_arc_funcoes_dart(&mut modulo.functions, &mut planos).is_err());
                planos.get_mut("escalares").unwrap().classes.insert(nativo, crate::otimizar::arc::Ownership::Trivial);
                assert_eq!(crate::otimizar::arc::inserir_arc_funcoes_dart(&mut modulo.functions, &mut planos).unwrap(), (0, 0));
            })
            .unwrap()
            .join()
            .unwrap();
    }
}

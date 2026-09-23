//! O hospedeiro das macros do DartForge (docs/MACROS-PROTOCOLO.md).
//!
//! * [`aplicacoes`] — detecção barata (custo zero sem classe `macro`) e a
//!   ordem das aplicações, a do CFE 3.6.2;
//! * [`modelo`] e [`consultas`] — o que a macro vê do programa, servido do
//!   `elements` com identificadores estáveis entre as fases;
//! * [`montagem`] — a biblioteca de augmentation com as Regras 1–4, byte a
//!   byte com a do CFE 3.6.2;
//! * [`protocolo`] e [`executor`] — o serviço `macro.*` do `dfexec/1` (o
//!   mesmo protocolo e executor dos builders, docs/BUILD-PROTOCOLO.md) e o
//!   trait [`executor::ExecutorMacros`], com [`executor::Indisponivel`] como
//!   implementação do produto até o executor nativo existir;
//! * [`sessao`] — as três fases com o programa recarregado entre elas;
//! * [`vm`] — o executor de **materialização**: a mesma API de macros (Dart
//!   puro, `pacotes/macros`) numa VM Dart, para gravar a augmentation que o
//!   SDK oficial aceita com a flag experimental
//!   (docs/MACROS-COMPATIBILIDADE.md). Não é o executor do produto.
#![allow(clippy::too_many_arguments)]

pub mod aplicacoes;
pub mod consultas;
pub mod executor;
pub mod modelo;
pub mod montagem;
pub mod protocolo;
pub mod sessao;
pub mod vm;

pub use sessao::{Saida, TextoGerado, aplicar, sessoes};

/// O programa aplica alguma macro? Uma olhada em
/// [`Program::classes_macro`](dartforge_elements::model::Program), que o
/// outline preenche na mesma passada que cria os elementos: sem classe
/// `macro`, nenhuma anotação é olhada (regra de custo zero, item 1).
pub fn tem_macros(program: &dartforge_elements::model::Program) -> bool {
    !program.classes_macro.is_empty()
}

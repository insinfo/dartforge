//! Geradores nativos ligados ao motor pela API **pública** do `gerador_ng`
//! (o motor não toca os internos dele; o que falta está pedido em
//! `docs/BUILD-PEDIDOS-GERADOR-NG.md`).
pub mod ng;
pub mod sass;

use crate::executor::GeradorNativo;
use std::sync::Arc;

pub fn todos() -> Vec<Arc<dyn GeradorNativo>> {
    vec![Arc::new(ng::NgEstagioA), Arc::new(sass::SassNativo)]
}

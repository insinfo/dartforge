//! Native generators hooked into the engine through the **public** API of
//! `ng_generator` (the engine does not touch its internals; what is missing
//! is requested in `docs/BUILD-PEDIDOS-GERADOR-NG.md`).
pub mod i18n;
pub mod ng;
pub mod sass;

use crate::executor::GeradorNativo;
use std::sync::Arc;

pub fn todos() -> Vec<Arc<dyn GeradorNativo>> {
    vec![
        Arc::new(ng::NgStageA::default()),
        Arc::new(ng::NgStageA::with_key("ngx_dart:ngx_dart")),
        Arc::new(sass::SassNative),
        Arc::new(i18n::I18nNative),
    ]
}

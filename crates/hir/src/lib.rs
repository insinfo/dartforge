//! Structural lowering boundary for the supported Dart subset.
//!
//! This currently owns syntax functions and statements rather than a resolved, typed IR. Semantic
//! validation must succeed before lowering; symbol IDs and typed operations are
//! future work. Source spans and borrowed source text remain available.
use dartforge_syntax::{Function, Program, Statement};

#[derive(Debug)]
pub struct Module<'a> {
    pub functions: Vec<Function<'a>>,
    pub statements: Vec<Statement<'a>>,
}

pub fn lower(program: Program<'_>) -> Module<'_> {
    Module {
        functions: program.functions,
        statements: program.statements,
    }
}

//! A repeated `this` becomes a local (`docs/JS-PRODUCAO-TAMANHO.md` §8.4).
//!
//! Methods in the DDC contract cite `this` many times (`this.x`, `this[s]`,
//! `S.new.call(this, …)`), and a minifier cannot shorten `this`. `dart2js`
//! keeps the receiver in a one-letter local (`var s=this`) in the methods
//! that use it a lot (`ssa/codegen.dart`, the receiver variable `s`). Here:
//! in a (non-arrow) function whose body cites `this` `k` times with
//! `3k > 11` (each citation then costs 1 byte instead of 4, against the ~11
//! of the declaration), the body gets `let t$this = this;` at the start and
//! each of its `this` (including inside arrows, which inherit the same
//! `this`) becomes `t$this`, which the renaming shortens.
//!
//! Left as they are: a class `constructor` (before `super()` the `this` does
//! not exist), bodies with directives, and the `this` of class field
//! initializers, which is not the enclosing function's.

use oxc_ast::ast::{Class, Function, MethodDefinition, MethodDefinitionKind, ThisExpression};
use oxc_ast_visit::{Visit, walk};
use oxc_syntax::scope::ScopeFlags;

struct Frame {
    /// The body (`{` … `}`); `None` when the function cannot receive the
    /// local.
    body: Option<(u32, u32)>,
    uses: Vec<(u32, u32)>,
}

struct Collector {
    stack: Vec<Frame>,
    next_is_constructor: bool,
    edits: Vec<(u32, u32, &'static str)>,
}

impl<'a> Visit<'a> for Collector {
    fn visit_method_definition(&mut self, it: &MethodDefinition<'a>) {
        if it.kind == MethodDefinitionKind::Constructor {
            self.next_is_constructor = true;
        }
        walk::walk_method_definition(self, it);
        self.next_is_constructor = false;
    }
    fn visit_class(&mut self, it: &Class<'a>) {
        // Barrier: the `this` of a class field is not the outer function's.
        self.stack.push(Frame {
            body: None,
            uses: Vec::new(),
        });
        walk::walk_class(self, it);
        self.stack.pop();
    }
    fn visit_function(&mut self, it: &Function<'a>, flags: ScopeFlags) {
        let constructor = std::mem::take(&mut self.next_is_constructor);
        let body = it
            .body
            .as_ref()
            .filter(|b| b.directives.is_empty() && !constructor)
            .map(|b| (b.span.start, b.span.end));
        self.stack.push(Frame {
            body,
            uses: Vec::new(),
        });
        walk::walk_function(self, it, flags);
        let q = self.stack.pop().unwrap_or(Frame {
            body: None,
            uses: Vec::new(),
        });
        if let Some((start, end)) = q.body {
            // Only the `this` of the body: the one in the parameters (default
            // value) runs before the declaration.
            let uses: Vec<(u32, u32)> = q
                .uses
                .into_iter()
                .filter(|(a, _)| *a > start && *a < end)
                .collect();
            if 3 * uses.len() > 11 {
                self.edits
                    .push((start + 1, start + 1, "let t$this = this;"));
                for (a, b) in uses {
                    self.edits.push((a, b, "t$this"));
                }
            }
        }
    }
    fn visit_this_expression(&mut self, it: &ThisExpression) {
        if let Some(q) = self.stack.last_mut() {
            q.uses.push((it.span.start, it.span.end));
        }
    }
}

/// Rewrites the text (`source`, already wrapped in the IIFE). Returns the
/// text as it came if it cannot be parsed.
pub fn apply(source: &str) -> String {
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(&allocator, source, oxc_span::SourceType::cjs()).parse();
    if !parsed.diagnostics.is_empty() {
        return source.to_string();
    }
    let mut c = Collector {
        stack: Vec::new(),
        next_is_constructor: false,
        edits: Vec::new(),
    };
    c.visit_program(&parsed.program);
    if c.edits.is_empty() {
        return source.to_string();
    }
    c.edits.sort_by_key(|t| (t.0, t.1));
    let mut out = String::with_capacity(source.len());
    let mut pos = 0usize;
    for (a, b, new) in c.edits {
        let (a, b) = (a as usize, b as usize);
        if a < pos {
            continue;
        }
        out.push_str(&source[pos..a]);
        out.push_str(new);
        pos = b;
    }
    out.push_str(&source[pos..]);
    out
}

#[cfg(test)]
mod tests {
    use super::apply;

    #[test]
    fn four_citations_become_a_local() {
        let s = apply(
            "(function () { function f() { return this.a + this.b + this.c + (() => this.d)(); } })();",
        );
        assert!(s.contains("let t$this = this;"), "{s}");
        assert_eq!(s.matches("this").count(), 1 + 5, "{s}");
        assert!(s.contains("t$this.d"), "{s}");
    }

    #[test]
    fn few_citations_constructor_and_field_stay() {
        let few = "(function () { function f() { return this.a + this.b + this.c; } })();";
        assert_eq!(apply(few), few);
        let constructor = "(function () { class A extends B { constructor() { super(); this.a = this.b = this.c = this.d = 1; } } })();";
        assert_eq!(apply(constructor), constructor);
        let field = "(function () { class A { x = this.a + this.b + this.c + this.d; } })();";
        assert_eq!(apply(field), field);
    }
}

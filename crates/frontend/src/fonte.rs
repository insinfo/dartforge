//! O texto de um nó como o `toSource()` (e o `toString()`) do analyzer
//! 3.6.2: o `ToSourceVisitor` (`analyzer/lib/src/dart/ast/to_source_visitor.dart`),
//! nó a nó, com os mesmos separadores. Os tokens folha (números, strings,
//! identificadores) saem com o lexema da fonte; o resto é recomposto, então
//! brancos e comentários da fonte não aparecem.
//!
//! O que a árvore daqui não guarda e o visitante imprime é lido da fonte,
//! no lugar exato em que o parser o consumiu: o nome do campo posicional de
//! um tipo record, o `var`/`final` de um padrão curinga, o `const` de um
//! padrão constante, o separador (`=` ou `:`) do valor padrão de um
//! parâmetro, os limites das strings adjacentes e das interpolações. A
//! metadata das declarações locais vem de `Ast::metadados_locais`.
//! Escrito sem compilar nem executar (2026-10-05).

use crate::ast::{
    Annotation, Arguments, AssignOp, Ast, BinaryOp, CatchClause, CollectionElement, CreationKeyword, ExprId, ExprKind, ForInTarget, ForInit, FunctionBody,
    FunctionId, AsyncModifier, ListPatternElement, Parameter, ParameterKind, PatternId, PatternKind, StmtId, StmtKind, StringLit, TypeId, TypeKind,
    TypeParameter, UnaryOp, VariableList,
};
use dartforge_intern::Interner;

/// Pula brancos e comentários (os de bloco aninham) a partir de `i`.
pub fn pular_brancos(b: &[u8], mut i: usize) -> usize {
    loop {
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if b[i.min(b.len())..].starts_with(b"//") {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
        } else if b[i.min(b.len())..].starts_with(b"/*") {
            let mut nivel = 0usize;
            while i < b.len() {
                if b[i..].starts_with(b"/*") {
                    nivel += 1;
                    i += 2;
                } else if b[i..].starts_with(b"*/") {
                    nivel -= 1;
                    i += 2;
                    if nivel == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
        } else {
            return i;
        }
    }
}

fn parte_de_identificador(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'$'
}

/// A fonte, a partir de `i`, começa com a palavra `p` inteira.
pub fn palavra_em(fonte: &str, i: usize, p: &str) -> bool {
    let b = fonte.as_bytes();
    b.get(i..).is_some_and(|r| r.starts_with(p.as_bytes())) && !b.get(i + p.len()).is_some_and(|&c| parte_de_identificador(c))
}

/// O fim do nó `FunctionTypedFormalParameter`/`FieldFormalParameter` com
/// lista (sem o valor padrão do `DefaultFormalParameter` que o envolve): o
/// `)` que fecha a lista que começa depois do nome (e dos parâmetros de
/// tipo), e o `?` que o siga.
pub fn fim_do_parametro_funcao(fonte: &str, depois_do_nome: usize) -> usize {
    let b = fonte.as_bytes();
    let mut i = depois_do_nome;
    let mut nivel = 0usize;
    while i < b.len() {
        let j = pular_brancos(b, i);
        if j != i {
            i = j;
            continue;
        }
        match b[i] {
            b'(' | b'[' | b'{' | b'<' => nivel += 1,
            b')' | b']' | b'}' | b'>' => {
                nivel = nivel.saturating_sub(1);
                if nivel == 0 && b[i] == b')' {
                    let k = pular_brancos(b, i + 1);
                    return if b.get(k) == Some(&b'?') { k + 1 } else { i + 1 };
                }
            }
            b'\'' | b'"' => {
                let q = b[i];
                i += 1;
                while i < b.len() && b[i] != q {
                    if b[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    b.len()
}

/// O texto do tipo `t`.
pub fn de_tipo(a: &Ast, fonte: &str, interner: &Interner, t: TypeId) -> String {
    let mut f = Impressor::novo(a, fonte, interner);
    f.tipo(t);
    f.saida
}

/// O texto da expressão `e`.
pub fn de_expr(a: &Ast, fonte: &str, interner: &Interner, e: ExprId) -> String {
    let mut f = Impressor::novo(a, fonte, interner);
    f.expr(e);
    f.saida
}

/// O texto da instrução `s`.
pub fn de_stmt(a: &Ast, fonte: &str, interner: &Interner, s: StmtId) -> String {
    let mut f = Impressor::novo(a, fonte, interner);
    f.stmt(s);
    f.saida
}

/// O texto do padrão `p`.
pub fn de_padrao(a: &Ast, fonte: &str, interner: &Interner, p: PatternId) -> String {
    let mut f = Impressor::novo(a, fonte, interner);
    f.padrao(p);
    f.saida
}

/// O texto da anotação `m`.
pub fn de_anotacao(a: &Ast, fonte: &str, interner: &Interner, m: &Annotation) -> String {
    let mut f = Impressor::novo(a, fonte, interner);
    f.anotacao(m);
    f.saida
}

/// O texto da lista de parâmetros de tipo `ps` (o `TypeParameterList`;
/// vazio sem nenhum).
pub fn de_parametros_de_tipo(a: &Ast, fonte: &str, interner: &Interner, ps: &[TypeParameter]) -> String {
    let mut f = Impressor::novo(a, fonte, interner);
    f.parametros_de_tipo(ps);
    f.saida
}

/// O texto da lista de parâmetros `ps` (o `FormalParameterList`).
pub fn de_parametros(a: &Ast, fonte: &str, interner: &Interner, ps: &[Parameter]) -> String {
    let mut f = Impressor::novo(a, fonte, interner);
    f.parametros(ps);
    f.saida
}

fn lexema_binario(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::TruncDiv => "~/",
        BinaryOp::Rem => "%",
        BinaryOp::Shl => "<<",
        BinaryOp::Shr => ">>",
        BinaryOp::UShr => ">>>",
        BinaryOp::BitAnd => "&",
        BinaryOp::BitOr => "|",
        BinaryOp::BitXor => "^",
        BinaryOp::Eq => "==",
        BinaryOp::NotEq => "!=",
        BinaryOp::Lt => "<",
        BinaryOp::Gt => ">",
        BinaryOp::LtEq => "<=",
        BinaryOp::GtEq => ">=",
        BinaryOp::And => "&&",
        BinaryOp::Or => "||",
        BinaryOp::IfNull => "??",
    }
}

struct Impressor<'a> {
    a: &'a Ast,
    fonte: &'a str,
    interner: &'a Interner,
    saida: String,
    /// O operador da seção de cascata em impressão (`..` ou `?..`): o
    /// `CascadeTarget` sai como ele.
    cascata: &'static str,
}

impl<'a> Impressor<'a> {
    fn novo(a: &'a Ast, fonte: &'a str, interner: &'a Interner) -> Self {
        Impressor { a, fonte, interner, saida: String::new(), cascata: ".." }
    }

    fn w(&mut self, s: &str) {
        self.saida.push_str(s);
    }

    fn nome(&mut self, n: crate::ast::Name) {
        let s = self.interner.resolve(n.sym);
        self.saida.push_str(s);
    }

    fn trecho(&mut self, s: crate::Span) {
        let t = self.fonte.get(s.start..s.end).unwrap_or("");
        self.saida.push_str(t);
    }

    /// `_visitNodeList(metadata, separator: ' ', suffix: ' ')`.
    fn metadata(&mut self, ms: &[Annotation]) {
        for m in ms {
            self.anotacao(m);
            self.w(" ");
        }
    }

    /// A metadata de uma declaração local (`Ast::metadados_locais`).
    fn metadata_local(&mut self, s: StmtId) {
        let a = self.a;
        if let Some((_, ms)) = a.metadados_locais.iter().find(|(x, _)| *x == s) {
            self.metadata(ms);
        }
    }

    // -----------------------------------------------------------------
    // Tipos
    // -----------------------------------------------------------------

    fn argumentos_de_tipo(&mut self, ts: &[TypeId]) {
        if ts.is_empty() {
            return;
        }
        self.w("<");
        for (k, &t) in ts.iter().enumerate() {
            if k > 0 {
                self.w(", ");
            }
            self.tipo(t);
        }
        self.w(">");
    }

    fn parametros_de_tipo(&mut self, ps: &[TypeParameter]) {
        if ps.is_empty() {
            return;
        }
        self.w("<");
        for (k, p) in ps.iter().enumerate() {
            if k > 0 {
                self.w(", ");
            }
            self.metadata(&p.metadata);
            if let Some((_, s)) = p.variance {
                self.trecho(s);
                self.w(" ");
            }
            self.nome(p.name);
            if let Some(b) = p.bound {
                self.w(" extends ");
                self.tipo(b);
            }
        }
        self.w(">");
    }

    fn tipo(&mut self, t: TypeId) {
        let a = self.a;
        let ty = a.ty(t);
        match &ty.kind {
            TypeKind::Named { name, args } => {
                for (k, n) in name.iter().enumerate() {
                    if k > 0 {
                        self.w(".");
                    }
                    self.nome(*n);
                }
                self.argumentos_de_tipo(args);
            }
            TypeKind::Void => self.w("void"),
            TypeKind::Function { return_type, type_params, parameters } => {
                if let Some(r) = return_type {
                    self.tipo(*r);
                }
                self.w(" Function");
                self.parametros_de_tipo(type_params);
                self.parametros(parameters);
            }
            TypeKind::Record { positional, named } => {
                self.w("(");
                for (k, &p) in positional.iter().enumerate() {
                    if k > 0 {
                        self.w(", ");
                    }
                    self.tipo(p);
                    // O nome do campo posicional: o identificador que o
                    // parser consumiu logo depois do tipo.
                    let b = self.fonte.as_bytes();
                    let i = pular_brancos(b, a.ty(p).span.end);
                    if b.get(i).is_some_and(|&c| parte_de_identificador(c) && !c.is_ascii_digit()) {
                        let mut j = i;
                        while j < b.len() && parte_de_identificador(b[j]) {
                            j += 1;
                        }
                        self.w(" ");
                        let nome = &self.fonte[i..j];
                        self.saida.push_str(nome);
                    }
                }
                if !named.is_empty() {
                    if !positional.is_empty() {
                        self.w(", ");
                    }
                    self.w("{");
                    for (k, (n, t)) in named.iter().enumerate() {
                        if k > 0 {
                            self.w(", ");
                        }
                        self.tipo(*t);
                        self.w(" ");
                        self.nome(*n);
                    }
                    self.w("}");
                }
                self.w(")");
            }
        }
        if ty.nullable {
            self.w("?");
        }
    }

    // -----------------------------------------------------------------
    // Parâmetros
    // -----------------------------------------------------------------

    /// `visitFormalParameterList`.
    fn parametros(&mut self, ps: &[Parameter]) {
        self.w("(");
        let mut fim_do_grupo: Option<&'static str> = None;
        for (k, p) in ps.iter().enumerate() {
            if k > 0 {
                self.w(", ");
            }
            if fim_do_grupo.is_none() && p.kind != ParameterKind::Required {
                if p.kind == ParameterKind::Named {
                    fim_do_grupo = Some("}");
                    self.w("{");
                } else {
                    fim_do_grupo = Some("]");
                    self.w("[");
                }
            }
            self.parametro(p);
        }
        if let Some(g) = fim_do_grupo {
            self.w(g);
        }
        self.w(")");
    }

    fn parametro(&mut self, p: &Parameter) {
        self.metadata(&p.metadata);
        if p.required {
            self.w("required ");
        }
        if p.covariant {
            self.w("covariant ");
        }
        let funcao = p.function_parameters.is_some();
        if p.this_ || p.super_ || !funcao {
            // `keyword`: o `FunctionTypedFormalParameter` não o tem.
            if p.final_ {
                self.w("final ");
            } else if p.const_ {
                self.w("const ");
            } else if p.var_ {
                self.w("var ");
            }
        }
        if p.this_ || p.super_ {
            if let Some(t) = p.ty {
                self.tipo(t);
                self.w(" ");
            }
            self.w(if p.this_ { "this." } else { "super." });
            if let Some(n) = p.name {
                self.nome(n);
            }
            self.parametros_de_tipo(&p.function_type_params);
            if let Some(fs) = &p.function_parameters {
                self.parametros(fs);
            }
        } else if let Some(fs) = &p.function_parameters {
            if let Some(t) = p.ty {
                self.tipo(t);
                self.w(" ");
            }
            if let Some(n) = p.name {
                self.nome(n);
            }
            self.parametros_de_tipo(&p.function_type_params);
            self.parametros(fs);
            if p.function_nullable {
                self.w("?");
            }
        } else {
            if let Some(t) = p.ty {
                self.tipo(t);
            }
            if p.ty.is_some() && p.name.is_some() {
                self.w(" ");
            }
            if let Some(n) = p.name {
                self.nome(n);
            }
        }
        // `visitDefaultFormalParameter`: o separador escrito.
        if let Some(v) = p.default_value {
            let inicio = self.a.expr(v).span.start;
            let b = self.fonte.as_bytes();
            // O separador é o último `=` ou `:` antes do valor.
            let mut i = inicio;
            let mut sep = b'=';
            while i > p.span.start {
                i -= 1;
                if b[i] == b'=' || b[i] == b':' {
                    sep = b[i];
                    break;
                }
            }
            if sep == b':' {
                self.w(":");
            } else {
                self.w(" =");
            }
            self.w(" ");
            self.expr(v);
        }
    }

    /// `visitAnnotation`: `@`, o nome (com o prefixo), os argumentos de
    /// tipo, `.construtor` e os argumentos.
    fn anotacao(&mut self, m: &Annotation) {
        self.w("@");
        // Os argumentos de tipo vêm depois das partes escritas antes deles.
        let antes = match m.type_args.first() {
            Some(&t) => {
                let ini = self.a.ty(t).span.start;
                m.name.iter().filter(|n| n.span.end <= ini).count()
            }
            None => m.name.len(),
        };
        for (k, n) in m.name.iter().enumerate() {
            if k == antes {
                self.argumentos_de_tipo(&m.type_args);
            }
            if k > 0 {
                self.w(".");
            }
            self.nome(*n);
        }
        if antes >= m.name.len() {
            self.argumentos_de_tipo(&m.type_args);
        }
        if let Some(args) = &m.arguments {
            self.argumentos(args, false);
        }
    }

    /// `visitArgumentList`; `com_tipos` imprime antes os argumentos de tipo
    /// da invocação.
    fn argumentos(&mut self, args: &Arguments, com_tipos: bool) {
        if com_tipos {
            self.argumentos_de_tipo(&args.type_args);
        }
        self.w("(");
        for (k, x) in args.args.iter().enumerate() {
            if k > 0 {
                self.w(", ");
            }
            if let Some(n) = x.name {
                // `visitNamedExpression`: o rótulo `nome:` e ` valor`.
                self.nome(n);
                self.w(": ");
            }
            self.expr(x.value);
        }
        self.w(")");
    }

    // -----------------------------------------------------------------
    // Expressões
    // -----------------------------------------------------------------

    fn expr(&mut self, e: ExprId) {
        let a = self.a;
        let x = a.expr(e);
        match &x.kind {
            ExprKind::Int(s) | ExprKind::Double(s) => self.trecho(*s),
            ExprKind::Bool(v) => self.w(if *v { "true" } else { "false" }),
            ExprKind::Null => self.w("null"),
            ExprKind::String(s) => self.string(s),
            ExprKind::Symbol(partes) => {
                self.w("#");
                for (k, n) in partes.iter().enumerate() {
                    if k > 0 {
                        self.w(".");
                    }
                    self.nome(*n);
                }
            }
            ExprKind::Identifier(n) => self.nome(*n),
            ExprKind::This => self.w("this"),
            ExprKind::Super => self.w("super"),
            ExprKind::Parenthesized(i) => {
                self.w("(");
                self.expr(*i);
                self.w(")");
            }
            ExprKind::List { const_, type_args, elements } => {
                if *const_ {
                    self.w("const ");
                }
                self.argumentos_de_tipo(type_args);
                self.w("[");
                self.elementos(elements);
                self.w("]");
            }
            ExprKind::SetOrMap { const_, type_args, elements } => {
                if *const_ {
                    self.w("const ");
                }
                self.argumentos_de_tipo(type_args);
                self.w("{");
                self.elementos(elements);
                self.w("}");
            }
            ExprKind::Record { positional, named, .. } => {
                // `visitRecordLiteral`: os campos na ordem escrita, sem o
                // `const` e sem a vírgula final.
                let mut campos: Vec<(usize, Option<crate::ast::Name>, ExprId)> = positional.iter().map(|&p| (a.expr(p).span.start, None, p)).collect();
                campos.extend(named.iter().map(|&(n, v)| (n.span.start, Some(n), v)));
                campos.sort_by_key(|c| c.0);
                self.w("(");
                for (k, (_, n, v)) in campos.into_iter().enumerate() {
                    if k > 0 {
                        self.w(", ");
                    }
                    if let Some(n) = n {
                        self.nome(n);
                        self.w(": ");
                    }
                    self.expr(v);
                }
                self.w(")");
            }
            ExprKind::InstanceCreation { keyword, ty, constructor, arguments } => {
                match keyword {
                    Some(CreationKeyword::New) => self.w("new "),
                    Some(CreationKeyword::Const) => self.w("const "),
                    None => {}
                }
                self.tipo(*ty);
                if let Some(c) = constructor {
                    self.w(".");
                    self.nome(*c);
                }
                self.argumentos(arguments, false);
            }
            ExprKind::FunctionExpression(f) => self.funcao(*f, false),
            ExprKind::Property { target, name, null_aware } => {
                if matches!(a.expr(*target).kind, ExprKind::CascadeTarget) {
                    self.w(self.cascata);
                } else {
                    self.expr(*target);
                    self.w(if *null_aware { "?." } else { "." });
                }
                self.nome(*name);
            }
            ExprKind::Index { target, index, null_aware } => {
                if matches!(a.expr(*target).kind, ExprKind::CascadeTarget) {
                    self.w(self.cascata);
                } else {
                    self.expr(*target);
                    if *null_aware {
                        self.w("?");
                    }
                }
                self.w("[");
                self.expr(*index);
                self.w("]");
            }
            ExprKind::Call { target, arguments } => {
                self.expr(*target);
                self.argumentos(arguments, true);
            }
            ExprKind::TypeArguments { target, type_args } => {
                self.expr(*target);
                self.argumentos_de_tipo(type_args);
            }
            ExprKind::Unary { op, operand } => {
                let (pre, lexema) = match op {
                    UnaryOp::Neg => (true, "-"),
                    UnaryOp::Not => (true, "!"),
                    UnaryOp::BitNot => (true, "~"),
                    UnaryOp::PrefixInc => (true, "++"),
                    UnaryOp::PrefixDec => (true, "--"),
                    UnaryOp::PostfixInc => (false, "++"),
                    UnaryOp::PostfixDec => (false, "--"),
                    UnaryOp::NullAssert => (false, "!"),
                };
                // Os operandos escritos já respeitam a precedência (os
                // parênteses estão na árvore): o `_writeOperand` não acrescenta.
                if pre {
                    self.w(lexema);
                    self.expr(*operand);
                } else {
                    self.expr(*operand);
                    self.w(lexema);
                }
            }
            ExprKind::Binary { op, left, right } => {
                self.expr(*left);
                self.w(" ");
                self.w(lexema_binario(*op));
                self.w(" ");
                self.expr(*right);
            }
            ExprKind::Conditional { condition, then, else_ } => {
                self.expr(*condition);
                self.w(" ? ");
                self.expr(*then);
                self.w(" : ");
                self.expr(*else_);
            }
            ExprKind::Is { value, ty, negated } => {
                self.expr(*value);
                self.w(if *negated { " is! " } else { " is " });
                self.tipo(*ty);
            }
            ExprKind::As { value, ty } => {
                self.expr(*value);
                self.w(" as ");
                self.tipo(*ty);
            }
            ExprKind::Assign { op, target, value } => {
                self.expr(*target);
                self.w(" ");
                match op {
                    AssignOp::Assign => self.w("="),
                    AssignOp::Compound(b) => {
                        self.w(lexema_binario(*b));
                        self.w("=");
                    }
                }
                self.w(" ");
                self.expr(*value);
            }
            ExprKind::PatternAssign { pattern, value } => {
                self.padrao(*pattern);
                self.w(" = ");
                self.expr(*value);
            }
            ExprKind::Cascade { target, sections, null_aware } => {
                self.expr(*target);
                let antes = self.cascata;
                for (k, &s) in sections.iter().enumerate() {
                    self.cascata = if k == 0 && *null_aware { "?.." } else { ".." };
                    self.expr(s);
                }
                self.cascata = antes;
            }
            ExprKind::CascadeTarget => self.w(self.cascata),
            ExprKind::Await(i) => {
                self.w("await ");
                self.expr(*i);
            }
            ExprKind::Throw(i) => {
                self.w("throw ");
                self.expr(*i);
            }
            ExprKind::Rethrow => self.w("rethrow"),
            ExprKind::Switch { value, cases } => {
                self.w("switch (");
                self.expr(*value);
                self.w(") {");
                for (k, c) in cases.iter().enumerate() {
                    if k > 0 {
                        self.w(", ");
                    }
                    self.padrao(c.pattern);
                    if let Some(g) = c.guard {
                        self.w(" when ");
                        self.expr(g);
                    }
                    self.w(" => ");
                    self.expr(c.body);
                }
                self.w("}");
            }
            ExprKind::DotShorthand { name, const_ } => {
                if *const_ {
                    self.w("const ");
                }
                self.w(".");
                self.nome(*name);
            }
        }
    }

    /// A string, possivelmente adjacente e interpolada, pela fonte: cada
    /// literal com o lexema (`visitSimpleStringLiteral`,
    /// `visitInterpolationString`), os adjacentes separados por um espaço
    /// (`visitAdjacentStrings`), cada interpolação `${e}` recomposta
    /// (`visitInterpolationExpression`).
    fn string(&mut self, s: &StringLit) {
        let a = self.a;
        let b = self.fonte.as_bytes();
        let fim = s.span.end.min(b.len());
        let mut interpolacoes = s.parts.iter().filter_map(|p| match p {
            crate::ast::StringPart::Interpolation(e) => Some(*e),
            crate::ast::StringPart::Text(_) => None,
        });
        let mut i = s.span.start;
        let mut primeiro = true;
        while i < fim {
            let j = pular_brancos(b, i);
            if j >= fim {
                break;
            }
            i = j;
            if !primeiro {
                self.w(" ");
            }
            primeiro = false;
            let cru = b[i] == b'r';
            let mut ini = i;
            if cru {
                i += 1;
            }
            let q = b[i];
            let tripla = b[i..].starts_with(&[q, q, q]);
            let n = if tripla { 3 } else { 1 };
            i += n;
            loop {
                if i >= fim {
                    self.saida.push_str(&self.fonte[ini..fim]);
                    return;
                }
                if b[i] == q && (!tripla || b[i..].starts_with(&[q, q, q])) {
                    i += n;
                    self.saida.push_str(&self.fonte[ini..i]);
                    break;
                }
                if !cru && b[i] == b'\\' {
                    i += 2;
                    continue;
                }
                if !cru && b[i] == b'$' && i + 1 < fim {
                    if b[i + 1] == b'{' {
                        self.saida.push_str(&self.fonte[ini..i]);
                        self.w("${");
                        if let Some(e) = interpolacoes.next() {
                            self.expr(e);
                            // Até o `}` que fecha, depois da expressão.
                            let k = pular_brancos(b, a.expr(e).span.end);
                            i = if b.get(k) == Some(&b'}') { k + 1 } else { k };
                        } else {
                            i += 2;
                        }
                        self.w("}");
                        ini = i;
                        continue;
                    }
                    if parte_de_identificador(b[i + 1]) && !b[i + 1].is_ascii_digit() && b[i + 1] != b'$' {
                        self.saida.push_str(&self.fonte[ini..i]);
                        self.w("$");
                        if let Some(e) = interpolacoes.next() {
                            self.expr(e);
                            i = a.expr(e).span.end;
                        } else {
                            i += 1;
                        }
                        ini = i;
                        continue;
                    }
                }
                i += 1;
            }
        }
    }

    fn elementos(&mut self, es: &[CollectionElement]) {
        for (k, e) in es.iter().enumerate() {
            if k > 0 {
                self.w(", ");
            }
            self.elemento(e);
        }
    }

    fn elemento(&mut self, e: &CollectionElement) {
        match e {
            CollectionElement::Expression(x) => self.expr(*x),
            CollectionElement::NullAwareExpression(x) => {
                self.w("?");
                self.expr(*x);
            }
            // `visitMapLiteralEntry`: a 3.6.2 não imprime os `?`.
            CollectionElement::MapEntry { key, value, .. } => {
                self.expr(*key);
                self.w(" : ");
                self.expr(*value);
            }
            CollectionElement::Spread { value, null_aware } => {
                self.w(if *null_aware { "...?" } else { "..." });
                self.expr(*value);
            }
            CollectionElement::If { condition, case_pattern, guard, then, else_ } => {
                self.w("if (");
                self.expr(*condition);
                if let Some(p) = case_pattern {
                    self.w(" case ");
                    self.padrao(*p);
                    if let Some(g) = guard {
                        self.w(" when ");
                        self.expr(*g);
                    }
                }
                self.w(") ");
                self.elemento(then);
                if let Some(x) = else_ {
                    self.w(" else ");
                    self.elemento(x);
                }
            }
            CollectionElement::For { await_, init, condition, updates, body } => {
                if *await_ {
                    self.w("await ");
                }
                self.w("for (");
                self.partes_do_for(init.as_ref(), *condition, updates);
                self.w(") ");
                self.elemento(body);
            }
            CollectionElement::ForIn { await_, target, iterable, body } => {
                if *await_ {
                    self.w("await ");
                }
                self.w("for (");
                self.partes_do_for_in(target, *iterable);
                self.w(") ");
                self.elemento(body);
            }
        }
    }

    fn partes_do_for(&mut self, init: Option<&ForInit>, condition: Option<ExprId>, updates: &[ExprId]) {
        if let Some(ForInit::Pattern { final_, pattern, value }) = init {
            // `visitForPartsWithPattern`.
            self.w(if *final_ { "final " } else { "var " });
            self.padrao(*pattern);
            self.w(" = ");
            self.expr(*value);
            self.w("; ");
            if let Some(c) = condition {
                self.expr(c);
            }
            self.w("; ");
            for (k, &u) in updates.iter().enumerate() {
                if k > 0 {
                    self.w(", ");
                }
                self.expr(u);
            }
            return;
        }
        match init {
            Some(ForInit::Variables(l)) => self.lista_de_variaveis(l),
            Some(ForInit::Expression(e)) => self.expr(*e),
            _ => {}
        }
        self.w(";");
        if let Some(c) = condition {
            self.w(" ");
            self.expr(c);
        }
        self.w(";");
        for (k, &u) in updates.iter().enumerate() {
            self.w(if k == 0 { " " } else { ", " });
            self.expr(u);
        }
    }

    fn partes_do_for_in(&mut self, target: &ForInTarget, iterable: ExprId) {
        match target {
            ForInTarget::Declared { metadata, final_, var_, ty, name } => {
                self.metadata(metadata);
                if *final_ {
                    self.w("final ");
                } else if *var_ {
                    self.w("var ");
                }
                if let Some(t) = ty {
                    self.tipo(*t);
                    self.w(" ");
                }
                self.nome(*name);
            }
            ForInTarget::Pattern { final_, pattern } => {
                self.w(if *final_ { "final " } else { "var " });
                self.padrao(*pattern);
            }
            ForInTarget::Expression(e) => self.expr(*e),
        }
        self.w(" in ");
        self.expr(iterable);
    }

    /// `visitVariableDeclarationList` (sem a metadata, que o chamador põe).
    fn lista_de_variaveis(&mut self, l: &VariableList) {
        if l.late {
            self.w("late ");
        }
        if l.final_ {
            self.w("final ");
        } else if l.const_ {
            self.w("const ");
        } else if l.var_ {
            self.w("var ");
        }
        if let Some(t) = l.ty {
            self.tipo(t);
            self.w(" ");
        }
        for (k, v) in l.variables.iter().enumerate() {
            if k > 0 {
                self.w(", ");
            }
            self.nome(v.name);
            if let Some(i) = v.initializer {
                self.w(" = ");
                self.expr(i);
            }
        }
    }

    /// `visitFunctionExpression` (`declarada`: o corpo `=> e` de uma
    /// declaração termina em `;`).
    fn funcao(&mut self, f: FunctionId, declarada: bool) {
        let a = self.a;
        let g = a.function(f);
        self.parametros_de_tipo(&g.type_params);
        if let Some(ps) = &g.parameters {
            self.parametros(ps);
        }
        // `_visitFunctionBody`.
        if !matches!(g.body, FunctionBody::Empty) {
            self.w(" ");
        }
        let modificador = match g.modifier {
            AsyncModifier::None => "",
            AsyncModifier::Async => "async ",
            AsyncModifier::AsyncStar => "async* ",
            AsyncModifier::SyncStar => "sync* ",
        };
        match &g.body {
            FunctionBody::Block(s) => {
                self.w(modificador);
                self.stmt(*s);
            }
            FunctionBody::Expression(e) => {
                self.w(modificador);
                self.w("=> ");
                self.expr(*e);
                if declarada {
                    self.w(";");
                }
            }
            FunctionBody::Empty => self.w(";"),
            FunctionBody::Native(lit) => {
                self.w("native ");
                if let Some(l) = lit {
                    self.string(l);
                }
                self.w(";");
            }
        }
    }

    // -----------------------------------------------------------------
    // Instruções
    // -----------------------------------------------------------------

    fn stmt(&mut self, s: StmtId) {
        let a = self.a;
        let st = a.stmt(s);
        match &st.kind {
            StmtKind::Block(ss) => {
                self.w("{");
                for (k, &x) in ss.iter().enumerate() {
                    if k > 0 {
                        self.w(" ");
                    }
                    self.stmt(x);
                }
                self.w("}");
            }
            StmtKind::Variables(l) => {
                self.metadata_local(s);
                self.lista_de_variaveis(l);
                self.w(";");
            }
            StmtKind::PatternVariables { final_, pattern, value } => {
                self.metadata_local(s);
                self.w(if *final_ { "final " } else { "var " });
                self.padrao(*pattern);
                self.w(" = ");
                self.expr(*value);
                self.w(";");
            }
            StmtKind::Function(f) => {
                // `visitFunctionDeclaration` de uma função local.
                self.metadata_local(s);
                let g = a.function(*f);
                if g.external {
                    self.w("external ");
                }
                if let Some(r) = g.return_type {
                    self.tipo(r);
                    self.w(" ");
                }
                if let Some(n) = g.name {
                    self.nome(n);
                }
                self.funcao(*f, true);
            }
            StmtKind::Expression(e) => {
                self.expr(*e);
                self.w(";");
            }
            StmtKind::If { condition, case_pattern, guard, then, else_ } => {
                self.w("if (");
                self.expr(*condition);
                if let Some(p) = case_pattern {
                    self.w(" case ");
                    self.padrao(*p);
                    if let Some(g) = guard {
                        self.w(" when ");
                        self.expr(*g);
                    }
                }
                self.w(") ");
                self.stmt(*then);
                if let Some(x) = else_ {
                    self.w(" else ");
                    self.stmt(*x);
                }
            }
            StmtKind::For { await_, init, condition, updates, body } => {
                if *await_ {
                    self.w("await ");
                }
                self.w("for (");
                self.partes_do_for(init.as_ref(), *condition, updates);
                self.w(") ");
                self.stmt(*body);
            }
            StmtKind::ForIn { await_, target, iterable, body } => {
                if *await_ {
                    self.w("await ");
                }
                self.w("for (");
                self.partes_do_for_in(target, *iterable);
                self.w(") ");
                self.stmt(*body);
            }
            StmtKind::While { condition, body } => {
                self.w("while (");
                self.expr(*condition);
                self.w(") ");
                self.stmt(*body);
            }
            StmtKind::DoWhile { body, condition } => {
                self.w("do ");
                self.stmt(*body);
                self.w(" while (");
                self.expr(*condition);
                self.w(");");
            }
            StmtKind::Switch { value, cases } => {
                self.w("switch (");
                self.expr(*value);
                self.w(") {");
                for (k, c) in cases.iter().enumerate() {
                    if k > 0 {
                        self.w(" ");
                    }
                    for l in c.labels.iter() {
                        self.nome(*l);
                        self.w(": ");
                    }
                    match c.pattern {
                        Some(p) => {
                            self.w("case ");
                            self.padrao(p);
                            if let Some(g) = c.guard {
                                self.w(" when ");
                                self.expr(g);
                            }
                            self.w(": ");
                        }
                        None => self.w("default: "),
                    }
                    for (j, &x) in c.body.iter().enumerate() {
                        if j > 0 {
                            self.w(" ");
                        }
                        self.stmt(x);
                    }
                }
                self.w("}");
            }
            StmtKind::Break(l) | StmtKind::Continue(l) => {
                self.w(if matches!(st.kind, StmtKind::Break(_)) { "break" } else { "continue" });
                if let Some(n) = l {
                    self.w(" ");
                    self.nome(*n);
                }
                self.w(";");
            }
            StmtKind::Return(v) => match v {
                None => self.w("return;"),
                Some(e) => {
                    self.w("return ");
                    self.expr(*e);
                    self.w(";");
                }
            },
            StmtKind::Yield { star, value } => {
                self.w(if *star { "yield* " } else { "yield " });
                self.expr(*value);
                self.w(";");
            }
            StmtKind::Try { body, catches, finally_ } => {
                self.w("try ");
                self.stmt(*body);
                for c in catches.iter() {
                    self.w(" ");
                    self.catch(c);
                }
                if let Some(f) = finally_ {
                    self.w(" finally ");
                    self.stmt(*f);
                }
            }
            StmtKind::Labeled { labels, body } => {
                for l in labels.iter() {
                    self.nome(*l);
                    self.w(": ");
                }
                self.stmt(*body);
            }
            StmtKind::Assert { condition, message } => {
                self.w("assert (");
                self.expr(*condition);
                if let Some(m) = message {
                    self.w(", ");
                    self.expr(*m);
                }
                self.w(");");
            }
            StmtKind::Empty => self.w(";"),
        }
    }

    fn catch(&mut self, c: &CatchClause) {
        if let Some(t) = c.on_type {
            self.w("on ");
            self.tipo(t);
        }
        if c.exception.is_some() {
            if c.on_type.is_some() {
                self.w(" ");
            }
            self.w("catch (");
            if let Some(e) = c.exception {
                self.nome(e);
            }
            if let Some(s) = c.stack_trace {
                self.w(", ");
                self.nome(s);
            }
            self.w(") ");
        } else {
            self.w(" ");
        }
        self.stmt(c.body);
    }

    // -----------------------------------------------------------------
    // Padrões
    // -----------------------------------------------------------------

    fn padrao(&mut self, p: PatternId) {
        let a = self.a;
        let pt = a.pattern(p);
        match &pt.kind {
            PatternKind::Wildcard { ty } => {
                // `visitWildcardPattern`: o `var`/`final` escrito.
                if palavra_em(self.fonte, pt.span.start, "var") {
                    self.w("var ");
                } else if palavra_em(self.fonte, pt.span.start, "final") {
                    self.w("final ");
                }
                if let Some(t) = ty {
                    self.tipo(*t);
                    self.w(" ");
                }
                self.w("_");
            }
            PatternKind::Variable { final_, var_, ty, name } => {
                if *final_ {
                    self.w("final ");
                } else if *var_ {
                    self.w("var ");
                }
                if let Some(t) = ty {
                    self.tipo(*t);
                    self.w(" ");
                }
                self.nome(*name);
            }
            PatternKind::Constant(e) => {
                // `visitConstantPattern`: o `const` do padrão, quando a
                // expressão não o imprime ela mesma.
                let antes = self.saida.len();
                let escrito = palavra_em(self.fonte, pt.span.start, "const");
                self.expr(*e);
                if escrito && !self.saida[antes..].starts_with("const ") {
                    self.saida.insert_str(antes, "const ");
                }
            }
            PatternKind::Relational { op, value } => {
                self.w(lexema_binario(*op));
                self.w(" ");
                self.expr(*value);
            }
            PatternKind::Or(l, r) => {
                self.padrao(*l);
                self.w(" || ");
                self.padrao(*r);
            }
            PatternKind::And(l, r) => {
                self.padrao(*l);
                self.w(" && ");
                self.padrao(*r);
            }
            PatternKind::NullCheck(i) => {
                self.padrao(*i);
                self.w("?");
            }
            PatternKind::NullAssert(i) => {
                self.padrao(*i);
                self.w("!");
            }
            PatternKind::Cast { pattern, ty } => {
                self.padrao(*pattern);
                self.w(" as ");
                self.tipo(*ty);
            }
            PatternKind::Parenthesized(i) => {
                self.w("(");
                self.padrao(*i);
                self.w(")");
            }
            PatternKind::List { type_args, elements } => {
                self.argumentos_de_tipo(type_args);
                self.w("[");
                for (k, e) in elements.iter().enumerate() {
                    if k > 0 {
                        self.w(", ");
                    }
                    match e {
                        ListPatternElement::Pattern(x) => self.padrao(*x),
                        ListPatternElement::Rest(x) => {
                            self.w("...");
                            if let Some(x) = x {
                                self.padrao(*x);
                            }
                        }
                    }
                }
                self.w("]");
            }
            PatternKind::Map { type_args, entries, rest } => {
                self.argumentos_de_tipo(type_args);
                self.w("{");
                for (k, e) in entries.iter().enumerate() {
                    if k > 0 {
                        self.w(", ");
                    }
                    self.expr(e.key);
                    self.w(": ");
                    self.padrao(e.value);
                }
                if *rest {
                    if !entries.is_empty() {
                        self.w(", ");
                    }
                    self.w("...");
                }
                self.w("}");
            }
            PatternKind::Record { fields } => {
                self.w("(");
                self.campos(fields);
                if fields.len() == 1 {
                    self.w(",");
                }
                self.w(")");
            }
            PatternKind::Object { ty, fields } => {
                self.tipo(*ty);
                self.w("(");
                self.campos(fields);
                self.w(")");
            }
        }
    }

    /// `visitPatternField`: `nome: p`, `: p` (o nome inferido do padrão)
    /// ou só `p`.
    fn campos(&mut self, fields: &[crate::ast::PatternField]) {
        let a = self.a;
        for (k, f) in fields.iter().enumerate() {
            if k > 0 {
                self.w(", ");
            }
            if let Some(n) = f.name {
                let sp = a.pattern(f.pattern).span;
                let inferido = n.span.start >= sp.start && n.span.end <= sp.end;
                if !inferido {
                    self.nome(n);
                }
                self.w(": ");
            }
            self.padrao(f.pattern);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brancos_e_comentarios() {
        let b = b"  /* a /* b */ c */ // x\n  y";
        assert_eq!(pular_brancos(b, 0), b.len() - 1);
    }

    #[test]
    fn palavra_inteira() {
        assert!(palavra_em("var x", 0, "var"));
        assert!(!palavra_em("variavel", 0, "var"));
    }
}

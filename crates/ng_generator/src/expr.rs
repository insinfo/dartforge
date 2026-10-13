//! Conversion of template expressions to Dart.
//!
//! `ngcompiler` has an expression parser of its own (1,620 lines in
//! `compiler/expression_parser`). We do not need it: a template expression is a
//! Dart expression, and the new track's parser already parses it. The work here
//! is only to rewrite the tree with the right receiver — `name` becomes
//! `_ctx.name` — and with the parentheses the official emitter adds.
//!
//! What `expression_converter.dart` writes and this module reproduces:
//! `_ctx.item.nome`, `_ctx.titulo()`, `(!_ctx.item.ativo)`, `'fixo'`.
//!
//! Three properties of the expression decide the shape of the generated code,
//! and each one follows the official rule (`analyzed_class.dart`):
//! - `isImmutable`: only a literal, `a ?? b` and a binary of immutables, and a
//!   `final`/`const` field read with the implicit receiver. An explicit getter
//!   is **mutable** (its `variable` is synthetic); `a.b`, `!x`, a call and a
//!   ternary too.
//! - `canBeNull`: only a literal is not null (and `a ?? b` when one side is
//!   not).
//! - the type (`_TypeResolver`): of the member or of the getter; the rest is
//!   `dynamic`.
//!
//! Event handlers (`parseAction`) are the same language with two more shapes:
//! `$event` and assignment (`x = y`, `a.b = y`).
use crate::component::Membro;
use crate::resolucao::Resolucao;
use crate::view::{Motivo, Recusa, recusa};
use dartforge_frontend::ast;
use dartforge_intern::Interner;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Start and end of the mark of a pipe call in the converted text:
/// `\u{3}name/arguments\u{4}`. The converter knows neither the view nor the
/// order of the calls; the view replaces the mark with the proxy.
pub const MARCA_DE_PIPE: char = '\u{3}';

/// Prefix of [`Convertida::estatica`] for an `exports:` receiver without an
/// `AnalyzedClass` ([`crate::resolucao::Exportado::TipoSemClasse`]).
const SEM_CLASSE: char = '\u{7}';
pub const FIM_DE_PIPE: char = '\u{4}';

/// A local of an embedded view: the name in Dart and the type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Local {
    /// How it comes out in the generated code: `local_item`.
    pub dart: String,
    /// Static type, for choosing the interpolation shape.
    pub tipo: String,
    /// File in whose scope the type text was written (the type of a member of
    /// another class is written in that class's library). `None`: the
    /// component's own.
    pub escopo: Option<PathBuf>,
}

/// A template expression already converted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Convertida {
    /// The Dart that goes into the generated file.
    pub texto: String,
    /// ngcompiler's `isImmutable`: a value that does not change needs no
    /// `checkBinding` and is written once, on the first check.
    pub imutavel: bool,
    /// ngcompiler's `canBeNull`: decides the `if (x != null)` around the
    /// constant binding and `setAttribute` × `updateAttribute`.
    pub pode_ser_nulo: bool,
    /// Static type, when it can be known without leaving the component class.
    /// It is what chooses between `interpolateString`, `interpolate` and
    /// `updateTextWithPrimitive`. `None`: unknown.
    pub tipo: Option<String>,
    /// File in whose scope `tipo` was written, when it is not the component's.
    pub escopo: Option<PathBuf>,
    /// The shape of the expression (`call`, `ternary`…), so the scoreboard says
    /// what is missing when it is refused in context.
    pub forma: &'static str,
    /// It is a primitive literal (`'x'`, `1`, `true`, `null`): the value is the
    /// text itself.
    pub literal: bool,
    /// The view locals the expression reads, in the order the official
    /// converter asks the `ViewNameResolver` for them — the order in which the
    /// `final local_x = …` declarations come out in the method.
    pub locais: Vec<String>,
    /// A class of `exports:` (the simple name): only valid as the receiver of a
    /// static member (`Class.name`).
    pub estatica: Option<String>,
}

impl Convertida {
    fn nova(texto: String, forma: &'static str) -> Self {
        Convertida {
            texto,
            imutavel: false,
            pode_ser_nulo: true,
            tipo: None,
            escopo: None,
            forma,
            literal: false,
            locais: Vec::new(),
            estatica: None,
        }
    }
}

/// Line break of the `DartFormatter` (dart_style 2.3.8, page of 1,000,000
/// columns) that the ngdart builder runs over the output: with an infinite
/// page, only the trailing comma of an argument list breaks it, and the emitter
/// writes a comma after each named argument (`visitAllNamedExpressions`). The
/// mark is [`QUEBRA`] followed by one [`RECUO`] per space beyond the
/// indentation of the line where the expression is, which is only known when
/// the file is assembled ([`resolver_quebras`]).
pub const QUEBRA: char = '\u{10}';
pub const RECUO: char = '\u{11}';

/// Adds `n` spaces to each [`QUEBRA`] of `t`.
pub fn recuar(t: &str, n: usize) -> String {
    let mais: String = std::iter::repeat_n(RECUO, n).collect();
    t.replace(QUEBRA, &format!("{QUEBRA}{mais}"))
}

/// Replaces each [`QUEBRA`] with the line break with the indentation of the
/// line it is on plus its [`RECUO`]s.
pub fn resolver_quebras(texto: &str) -> String {
    if !texto.contains(QUEBRA) {
        return texto.to_string();
    }
    let mut saida = String::with_capacity(texto.len() + texto.len() / 8);
    for (k, linha) in texto.split('\n').enumerate() {
        if k > 0 {
            saida.push('\n');
        }
        let base = linha.len() - linha.trim_start_matches(' ').len();
        let mut chars = linha.chars().peekable();
        while let Some(c) = chars.next() {
            if c != QUEBRA {
                saida.push(c);
                continue;
            }
            let mut n = base;
            while chars.next_if_eq(&RECUO).is_some() {
                n += 1;
            }
            saida.push('\n');
            saida.extend(std::iter::repeat_n(' ', n));
        }
    }
    saida
}

/// Joins lists of locals keeping the first occurrence of each.
fn juntar(partes: &[&[String]]) -> Vec<String> {
    let mut saida: Vec<String> = Vec::new();
    for p in partes {
        for l in p.iter() {
            if !saida.contains(l) {
                saida.push(l.clone());
            }
        }
    }
    saida
}

/// What a component's expressions see: members, methods, the view locals and
/// the semantic database.
#[derive(Clone, Copy)]
pub struct Escopo<'a> {
    pub membros: &'a HashMap<String, Membro>,
    pub metodos: &'a HashMap<String, String>,
    /// Positional parameters of each method (`rewriteTearOff`).
    pub aridades: &'a HashMap<String, usize>,
    pub locais: &'a HashMap<String, Local>,
    pub tipos: Option<(&'a dyn Resolucao, &'a Path)>,
    /// The component class as the generated file writes it (`import1.Class`),
    /// for static members; `None` when unknown, and the static is refused.
    pub classe: Option<&'a str>,
    /// The names of `exports:` (`_matchExport`), with the deferred qualifier.
    pub exportados: Option<&'a crate::view::Exportados>,
    /// The component's type parameters (`T` of `Class<T>`): in scope in all its
    /// views, they go in as they are when substituting the inherited types.
    pub genericos: &'a [String],
}

/// Converts an expression written in the template.
///
/// `membros` are the fields and getters of the component: a name that is not
/// there does not belong to `_ctx` and the expression is refused, because
/// translating it wrong would give a file that compiles and does something
/// else.
pub fn converter(
    expressao: &str,
    membros: &HashMap<String, Membro>,
    interner: &mut Interner,
) -> Result<Convertida, Recusa> {
    converter_com_metodos(expressao, membros, &HashMap::new(), interner, None)
}

/// Like [`converter`], also knowing the class's methods — which are only valid
/// as call targets.
pub fn converter_com_metodos(
    expressao: &str,
    membros: &HashMap<String, Membro>,
    metodos: &HashMap<String, String>,
    interner: &mut Interner,
    tipos: Option<(&dyn Resolucao, &Path)>,
) -> Result<Convertida, Recusa> {
    converter_com_locais(
        expressao,
        membros,
        metodos,
        &HashMap::new(),
        interner,
        tipos,
    )
}

/// Like [`converter_com_metodos`], with the locals of an embedded view (`let
/// item of itens` puts `item` in scope). A local wins over the component
/// member, as the template scope dictates.
pub fn converter_com_locais(
    expressao: &str,
    membros: &HashMap<String, Membro>,
    metodos: &HashMap<String, String>,
    locais: &HashMap<String, Local>,
    interner: &mut Interner,
    tipos: Option<(&dyn Resolucao, &Path)>,
) -> Result<Convertida, Recusa> {
    let aridades = HashMap::new();
    let escopo = Escopo {
        membros,
        metodos,
        aridades: &aridades,
        locais,
        tipos,
        classe: None,
        exportados: None,
        genericos: &[],
    };
    converter_no_escopo(expressao, &escopo, interner)
}

/// Converts a binding expression (`parseBinding`/`parseInterpolation`).
pub fn converter_no_escopo(
    expressao: &str,
    escopo: &Escopo,
    interner: &mut Interner,
) -> Result<Convertida, Recusa> {
    let (analisada, fonte, raiz) = analisar(expressao, interner)?;
    let c = Conversor {
        ast: &analisada.ast,
        fonte: &fonte,
        interner,
        escopo,
        acao: false,
    };
    c.expr(raiz, true)
}

/// A converted event handler: `parseAction` followed by
/// `handlerTypeFromExpression` (`parse_utils.dart`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Acao {
    /// `m()` or `m($event)` with the implicit receiver (or the tear-off `m`
    /// rewritten into one of these by `rewriteTearOff`): comes out as a
    /// tear-off, `this.eventHandlerN(_ctx.m)`.
    Simples {
        metodo: String,
        aridade: u8,
        /// The same call as a statement (`_ctx.m($event)`), for when this
        /// handler is merged with another one of the same event.
        instrucao: String,
    },
    /// Any other shape: a statement of the `_handleEvent_N` method.
    Complexa(Convertida),
}

/// Converts the text of an `(event)="..."`.
pub fn converter_acao(
    expressao: &str,
    escopo: &Escopo,
    interner: &mut Interner,
) -> Result<Acao, Recusa> {
    let (analisada, fonte, raiz) = analisar(expressao, interner)?;
    let c = Conversor {
        ast: &analisada.ast,
        fonte: &fonte,
        interner,
        escopo,
        acao: true,
    };
    c.acao(raiz)
}

/// Parses the text as a Dart expression: a top-level `var` is the smallest
/// context in which the parser accepts any expression.
fn analisar(
    expressao: &str,
    interner: &mut Interner,
) -> Result<(dartforge_frontend::parser::Parsed, String, ast::ExprId), Recusa> {
    let fonte = format!("var _e = {expressao};");
    let analisada = dartforge_frontend::parser::parse(&fonte, interner);
    let invalida = || recusa(Motivo::Expressao, "expression the Dart parser does not accept");
    if !analisada.diagnostics.is_empty() {
        return Err(invalida());
    }
    let Some(&id) = analisada.unit.declarations.first() else {
        return Err(invalida());
    };
    let ast::DeclKind::Variables(lista) = &analisada.ast.decl(id).kind else {
        return Err(invalida());
    };
    let Some(v) = lista.variables.first() else {
        return Err(invalida());
    };
    let Some(inicial) = v.initializer else {
        return Err(invalida());
    };
    Ok((analisada, fonte, inicial))
}

struct Conversor<'a> {
    ast: &'a ast::Ast,
    fonte: &'a str,
    interner: &'a Interner,
    escopo: &'a Escopo<'a>,
    /// Event handler: `$event` and assignment are allowed.
    acao: bool,
}

fn fora(forma: &'static str) -> Recusa {
    recusa(Motivo::Expressao, forma)
}

impl Conversor<'_> {
    /// An instance field or getter that the component class inherits (the
    /// analyzer's `lookUpGetter` walks up the hierarchy): the type and the
    /// scope come from whoever declares it, and the immutability is that of the
    /// field there. Without a written type, or with a type that mentions a type
    /// parameter or a private of another library: `None`, and the name is
    /// refused.
    fn membro_herdado(&self, nome: &str) -> Option<Convertida> {
        if nome.starts_with('_') {
            return None;
        }
        let (r, arquivo) = self.escopo.tipos?;
        let classe = self.escopo.classe?.rsplit('.').next()?;
        let (tipo, escopo) =
            r.tipo_do_membro_livre(arquivo, &self.receptor(classe), nome, self.escopo.genericos)?;
        let imutavel = r.membro_final(arquivo, classe, nome)?;
        Some(Convertida {
            imutavel,
            tipo: Some(tipo),
            escopo: Some(escopo),
            ..Convertida::nova(format!("_ctx.{nome}"), "membro herdado")
        })
    }

    /// An instance setter (`set name(..)`) of the class or above it: a write
    /// target that is neither a field nor a getter ([`Resolucao::tem_setter`]).
    fn setter(&self, nome: &str) -> bool {
        if nome.starts_with('_') {
            return false;
        }
        let (Some((r, arquivo)), Some(classe)) = (self.escopo.tipos, self.escopo.classe) else {
            return false;
        };
        r.tem_setter(arquivo, classe.rsplit('.').next().unwrap_or(classe), nome)
    }

    /// An inherited instance method ([`Resolucao::metodo`]), looked up only
    /// when the name is neither the class's own nor private.
    fn metodo_herdado(&self, nome: &str) -> Option<crate::resolucao::Metodo> {
        if nome.starts_with('_') {
            return None;
        }
        let (r, arquivo) = self.escopo.tipos?;
        let classe = self.escopo.classe?.rsplit('.').next()?;
        r.metodo_livre(arquivo, &self.receptor(classe), nome, self.escopo.genericos)
    }

    /// The implicit receiver as a type: the class with its own parameters
    /// (`Class<T>`), which the hierarchy lookup substitutes as free.
    fn receptor(&self, classe: &str) -> String {
        if self.escopo.genericos.is_empty() {
            classe.to_string()
        } else {
            format!("{classe}<{}>", self.escopo.genericos.join(", "))
        }
    }

    /// O handler inteiro: classifica (simples × complexo) e converte.
    fn acao(&self, id: ast::ExprId) -> Result<Acao, Recusa> {
        let id = self.sem_parenteses(id);
        let e = self.ast.expr(id);
        // Tear-off (`(click)="m"`): `rewriteTearOff` replaces it with `m()` or
        // `m($event)`, according to the method's positional parameters.
        if let ast::ExprKind::Identifier(n) = &e.kind {
            let nome = self.interner.resolve(n.sym);
            if !self.escopo.locais.contains_key(nome) && nome != "$event" {
                if let Some(&posicionais) = self.escopo.aridades.get(nome) {
                    let aridade = u8::from(posicionais > 0);
                    return self.simples(nome, aridade);
                }
                if self.escopo.metodos.contains_key(nome) {
                    return Err(fora("tear-off of a method without a known arity"));
                }
                if !self.escopo.membros.contains_key(nome)
                    && let Some(m) = self.metodo_herdado(nome)
                {
                    return self.simples(nome, u8::from(m.posicionais > 0));
                }
            }
        }
        // `handlerTypeFromExpression`: `m()` or `m($event)`, implicit receiver,
        // no named argument.
        if let ast::ExprKind::Call { target, arguments } = &e.kind
            && let ast::ExprKind::Identifier(n) = &self.ast.expr(*target).kind
            && arguments.type_args.is_empty()
            && arguments.args.iter().all(|a| a.name.is_none())
        {
            let nome = self.interner.resolve(n.sym);
            let aridade = match &arguments.args[..] {
                [] => Some(0),
                [a] if self.e_event(a.value) => Some(1),
                _ => None,
            };
            if let Some(aridade) = aridade {
                // A local with the method's name: the official compiler calls
                // the local's function (`InvokeFunctionExpr`) and refuses the
                // tear-off.
                if self.escopo.locais.contains_key(nome) {
                    return Err(fora("handler that calls a local"));
                }
                return self.simples(nome, aridade);
            }
        }
        let c = match &e.kind {
            // An assignment at the top of the statement comes out without
            // parentheses (`lineWasEmpty` in `visitWritePropExpr`).
            ast::ExprKind::Assign { .. } => self.atribuicao(id)?,
            _ => self.expr(id, true)?,
        };
        Ok(Acao::Complexa(c))
    }

    /// A simple handler: the method must belong to the component (or be a
    /// field, which the tear-off reads the same way).
    fn simples(&self, nome: &str, aridade: u8) -> Result<Acao, Recusa> {
        if !self.escopo.metodos.contains_key(nome)
            && !self.escopo.membros.contains_key(nome)
            && self.metodo_herdado(nome).is_none()
            && self.membro_herdado(nome).is_none()
        {
            return Err(fora("handler that is not a member of the component"));
        }
        if self.escopo.membros.get(nome).is_some_and(|m| m.estatico) {
            return Err(fora("handler on a static member"));
        }
        let arg = if aridade == 1 { "$event" } else { "" };
        Ok(Acao::Simples {
            metodo: nome.to_string(),
            aridade,
            instrucao: format!("_ctx.{nome}({arg})"),
        })
    }

    /// `$pipe.name(input, args)`: the official parser's `BindingPipe`
    /// (`_createPipeOrThrow`). The call comes out as `MARK(input, args)`, and
    /// whoever knows the view replaces the [`MARCA_DE_PIPE`] with the
    /// `pureProxyN` of that point (`CompilePipe._call`). Type `dynamic`
    /// (`visitPipe` of `_TypeResolver`), mutable and possibly null.
    fn pipe(&self, nome: &str, arguments: &ast::Arguments) -> Result<Convertida, Recusa> {
        if self.acao {
            return Err(recusa(Motivo::PipesUsados, "pipe in an event"));
        }
        if arguments.args.iter().any(|a| a.name.is_some()) {
            return Err(recusa(Motivo::PipesUsados, "pipe com argumento nomeado"));
        }
        if arguments.args.is_empty() {
            return Err(recusa(Motivo::PipesUsados, "pipe sem argumento"));
        }
        // `visitPipe` converts the input and then the arguments: that is the
        // order in which the locals are requested.
        let mut args = Vec::new();
        let mut locais = Vec::new();
        // The inner pipe gets its proxy first (it is converted earlier), but
        // the outer mark comes first in the text: whoever replaces the marks
        // sorts them by the end of the call.
        for a in arguments.args.iter() {
            let v = self.expr(a.value, true)?;
            locais.extend(v.locais);
            args.push(v.texto);
        }
        Ok(Convertida {
            tipo: Some("dynamic".into()),
            locais: juntar(&[&locais]),
            ..Convertida::nova(
                format!(
                    "{MARCA_DE_PIPE}{nome}/{}{FIM_DE_PIPE}({})",
                    args.len(),
                    args.join(", ")
                ),
                "pipe",
            )
        })
    }

    fn e_event(&self, id: ast::ExprId) -> bool {
        matches!(&self.ast.expr(self.sem_parenteses(id)).kind,
            ast::ExprKind::Identifier(n) if self.interner.resolve(n.sym) == "$event")
    }

    fn sem_parenteses(&self, mut id: ast::ExprId) -> ast::ExprId {
        while let ast::ExprKind::Parenthesized(i) = &self.ast.expr(id).kind {
            id = *i;
        }
        id
    }

    /// `x = y`, `a.b = y`, `a[i] = y` (`PropertyWrite`/`KeyedWrite`), without
    /// the parentheses — whoever is inside another expression adds them.
    fn atribuicao(&self, id: ast::ExprId) -> Result<Convertida, Recusa> {
        let ast::ExprKind::Assign { op, target, value } = &self.ast.expr(id).kind else {
            return Err(fora("assignment"));
        };
        if !self.acao {
            return Err(fora("assignment outside an event"));
        }
        // The official compiler ignores the operator of a compound assignment
        // and writes `x = y`: a shape deliberately not reproduced.
        if *op != ast::AssignOp::Assign {
            return Err(fora("compound assignment (`+=`)"));
        }
        let (alvo, locais_alvo) = match &self.ast.expr(*target).kind {
            ast::ExprKind::Identifier(n) => {
                let nome = self.interner.resolve(n.sym);
                if self.escopo.locais.contains_key(nome) || nome == "$event" {
                    return Err(fora("assignment to a local"));
                }
                match self.escopo.membros.get(nome) {
                    // Inherited field: the write also walks up the hierarchy
                    // (`PropertyWrite` with the implicit receiver becomes
                    // `_ctx.x`).
                    None if self.membro_herdado(nome).is_some() || self.setter(nome) => {
                        (format!("_ctx.{nome}"), Vec::new())
                    }
                    None => return Err(fora("assignment to a name outside the component")),
                    Some(m) if m.estatico => match self.escopo.classe {
                        Some(classe) => (format!("{classe}.{nome}"), Vec::new()),
                        None => return Err(fora("static member without the qualified class")),
                    },
                    Some(_) => (format!("_ctx.{nome}"), Vec::new()),
                }
            }
            ast::ExprKind::Property {
                target: t,
                name,
                null_aware: false,
            } => {
                let r = self.expr(*t, true)?;
                (
                    format!("{}.{}", r.texto, self.interner.resolve(name.sym)),
                    r.locais,
                )
            }
            ast::ExprKind::Index { .. } => return Err(fora("assignment to an index `a[i] = x`")),
            _ => return Err(fora("assignment to an unknown target")),
        };
        let v = self.expr(*value, true)?;
        Ok(Convertida {
            locais: juntar(&[&locais_alvo, &v.locais]),
            ..Convertida::nova(format!("{alvo} = {}", v.texto), "assignment")
        })
    }

    /// `raiz` says whether this node is the implicit receiver — only then does
    /// an identifier become `_ctx.name`; in `a.b`, `b` is a member of `a`.
    fn expr(&self, id: ast::ExprId, raiz: bool) -> Result<Convertida, Recusa> {
        let v = self.expr_do_no(id, raiz)?;
        // A call that breaks ([`QUEBRA`]) only as the whole expression or as an
        // argument of another call: inside an operator, an access or a
        // conditional the formatter indents it differently, no case.
        if v.texto.contains(QUEBRA) && !matches!(self.ast.expr(id).kind, ast::ExprKind::Call { .. })
        {
            return Err(fora("call with a named argument inside an expression"));
        }
        Ok(v)
    }

    fn expr_do_no(&self, id: ast::ExprId, raiz: bool) -> Result<Convertida, Recusa> {
        let e = self.ast.expr(id);
        match &e.kind {
            // `LiteralPrimitive`: immutable and never null (`canBeNull`). The
            // type is `String` for a string and `dynamic` for the rest
            // (`visitLiteralPrimitive` of `_TypeResolver`). The text is what
            // `o.literal(value)` writes, not what the template wrote.
            ast::ExprKind::Int(_) => {
                let t = self.texto(id);
                let Ok(v) = t.parse::<u64>() else {
                    return Err(fora("integer literal outside the decimal form"));
                };
                Ok(Convertida {
                    imutavel: true,
                    pode_ser_nulo: false,
                    tipo: Some("dynamic".into()),
                    literal: true,
                    ..Convertida::nova(v.to_string(), "literal")
                })
            }
            ast::ExprKind::Double(_) => {
                let t = self.texto(id);
                // Dart's `double.toString()` and Rust's `{:?}` agree on the
                // short form (`1.5`, `2.0`); on exponents, they do not.
                match t.parse::<f64>() {
                    Ok(v) if format!("{v:?}") == t && !t.contains(['e', 'E']) => Ok(Convertida {
                        imutavel: true,
                        pode_ser_nulo: false,
                        tipo: Some("dynamic".into()),
                        literal: true,
                        ..Convertida::nova(t, "literal")
                    }),
                    _ => Err(fora("double literal outside the canonical form")),
                }
            }
            ast::ExprKind::Bool(b) => Ok(Convertida {
                imutavel: true,
                pode_ser_nulo: false,
                tipo: Some("dynamic".into()),
                literal: true,
                ..Convertida::nova(b.to_string(), "literal")
            }),
            ast::ExprKind::Null => Ok(Convertida {
                imutavel: true,
                pode_ser_nulo: false,
                tipo: Some("dynamic".into()),
                literal: true,
                ..Convertida::nova("null".into(), "literal nulo")
            }),
            ast::ExprKind::String(lit) => {
                // Only a literal without interpolation: `'a$b'` inside the
                // template is something else and does not appear in the owner's
                // projects.
                let Some(valor) = lit.constant_value() else {
                    return Err(fora("string with Dart interpolation"));
                };
                Ok(Convertida {
                    imutavel: true,
                    pode_ser_nulo: false,
                    tipo: Some("String".into()),
                    literal: true,
                    ..Convertida::nova(crate::view::literal(&valor.to_string_lossy()), "literal")
                })
            }
            ast::ExprKind::Identifier(n) => {
                let nome = self.interner.resolve(n.sym);
                if !raiz {
                    return Ok(Convertida::nova(nome.to_string(), "name"));
                }
                // `getLocal('$event')` returns the handler's variable.
                if nome == "$event" {
                    if !self.acao {
                        return Err(fora("`$event` outside an event"));
                    }
                    return Ok(Convertida {
                        tipo: Some("dynamic".into()),
                        ..Convertida::nova("$event".into(), "`$event`")
                    });
                }
                // An `exports:` name (`_matchExport`, at parse time: before the
                // locals and the members, `analyzer_parser.dart:435-455`):
                // through the import of the library that declares it, type
                // `dynamic`. A `StaticRead` is always immutable
                // (`analyzed_class.dart:116`), even of a mutable variable or a
                // getter.
                if let Some((q, o_que)) = self.escopo.exportados.and_then(|e| e.get(nome)) {
                    use crate::resolucao::Exportado;
                    let texto = format!("{q}{nome}");
                    return match o_que {
                        Exportado::Classe => Ok(Convertida {
                            tipo: Some("dynamic".into()),
                            estatica: Some(nome.to_string()),
                            ..Convertida::nova(texto, "classe de exports:")
                        }),
                        // Without an `AnalyzedClass` (enum, mixin): `X.y` is a
                        // plain `PropertyRead`, mutable and `dynamic`.
                        Exportado::TipoSemClasse => Ok(Convertida {
                            tipo: Some("dynamic".into()),
                            estatica: Some(format!("{SEM_CLASSE}{nome}")),
                            ..Convertida::nova(texto, "enum or mixin from exports:")
                        }),
                        Exportado::Variavel { .. } => Ok(Convertida {
                            imutavel: true,
                            tipo: Some("dynamic".into()),
                            ..Convertida::nova(texto, "variable from exports:")
                        }),
                        Exportado::Getter => Ok(Convertida {
                            imutavel: true,
                            tipo: Some("dynamic".into()),
                            ..Convertida::nova(texto, "getter de exports:")
                        }),
                        Exportado::Funcao => Err(fora("function from exports: as a value")),
                    };
                }
                // The loop local shadows the component member (but not the
                // export, resolved earlier, at parse time). `isImmutable` does
                // not know about locals: the component field with the same name
                // counts (`lookUpGetter`), or the method ("methods are
                // immutable").
                if let Some(l) = self.escopo.locais.get(nome) {
                    let imutavel = match self.escopo.membros.get(nome) {
                        Some(m) => m.imutavel,
                        None => {
                            self.escopo.metodos.contains_key(nome)
                                || self.membro_herdado(nome).is_some_and(|c| c.imutavel)
                                || self.metodo_herdado(nome).is_some()
                        }
                    };
                    return Ok(Convertida {
                        imutavel,
                        tipo: Some(l.tipo.clone()),
                        escopo: l.escopo.clone(),
                        locais: vec![nome.to_string()],
                        ..Convertida::nova(l.dart.clone(), "local")
                    });
                }
                // A method read as a value (the `trackBy: rastrear` of
                // `*ngFor`, a callback passed to a child): `isImmutable` says
                // "methods are immutable"; `canBeNull` does not exempt it; and
                // the `_TypeResolver` looks for a *getter* with the name —
                // there is none, so it is `dynamic`.
                if !self.escopo.membros.contains_key(nome) && self.escopo.metodos.contains_key(nome)
                {
                    return Ok(Convertida {
                        imutavel: true,
                        tipo: Some("dynamic".into()),
                        ..Convertida::nova(format!("_ctx.{nome}"), "method as a value")
                    });
                }
                let Some(m) = self.escopo.membros.get(nome) else {
                    return self
                        .membro_herdado(nome)
                        .ok_or_else(|| fora("name outside the component"));
                };
                // Static: the class's `importExpr` and the name; `isImmutable`
                // by the field (`final`/`const`), type `dynamic` and, outside a
                // literal, possibly null (case j25).
                if m.estatico {
                    let Some(classe) = self.escopo.classe else {
                        return Err(fora("static member without the qualified class"));
                    };
                    return Ok(Convertida {
                        imutavel: m.imutavel,
                        tipo: Some("dynamic".into()),
                        ..Convertida::nova(format!("{classe}.{nome}"), "static member")
                    });
                }
                // `isImmutable` of a `PropertyRead` with the implicit receiver:
                // a `final`/`const` field (a getter already arrives here
                // mutable). Without a written type, what the semantic database
                // infers from the declaration (`Enum.values`,
                // `Class<A>.named()`…), with its scope.
                let (tipo, escopo) = if m.tipo.is_empty() {
                    match self.membro_herdado(nome) {
                        Some(c) => (c.tipo, c.escopo),
                        None => (None, None),
                    }
                } else {
                    (Some(m.tipo.clone()), None)
                };
                Ok(Convertida {
                    imutavel: m.imutavel,
                    tipo,
                    escopo,
                    ..Convertida::nova(format!("_ctx.{nome}"), "member")
                })
            }
            ast::ExprKind::Property {
                target,
                name,
                null_aware,
            } => {
                let alvo = self.expr(*target, raiz)?;
                let nome = self.interner.resolve(name.sym);
                let ponto = if *null_aware { "?." } else { "." };
                // `Class.name` of an `exports:` class: a `const`/`final` field
                // (and an enum value) is immutable, a getter is not, and a
                // method is ("methods are immutable",
                // `analyzed_class.dart:140-150`). Type `dynamic` (case j51).
                if alvo
                    .estatica
                    .as_deref()
                    .is_some_and(|c| c.starts_with(SEM_CLASSE))
                {
                    return Ok(Convertida {
                        tipo: Some("dynamic".into()),
                        locais: alvo.locais,
                        ..Convertida::nova(
                            format!("{}{ponto}{nome}", alvo.texto),
                            "member of an enum or mixin",
                        )
                    });
                }
                if let Some(classe) = &alvo.estatica {
                    use crate::resolucao::Estatico;
                    let Some((r, arquivo)) = self.escopo.tipos else {
                        return Err(fora("static member from exports: without the semantic database"));
                    };
                    let imutavel = match r.membro_estatico(arquivo, classe, nome) {
                        Some(Estatico::Campo { imutavel }) => imutavel,
                        Some(Estatico::Getter) => false,
                        Some(Estatico::Metodo) => true,
                        None => return Err(fora("unknown static member from exports:")),
                    };
                    return Ok(Convertida {
                        imutavel,
                        tipo: Some("dynamic".into()),
                        locais: alvo.locais,
                        ..Convertida::nova(
                            format!("{}{ponto}{nome}", alvo.texto),
                            "static member",
                        )
                    });
                }
                // The type at the end of the chain is in another class: the
                // semantic database answers, as the analyzer answers the
                // official compiler. A `dynamic` receiver (the `#ref`,
                // `$event`): `_lookupGetterReturnType` only looks at
                // `InterfaceType`, and the rest gives `dynamic`.
                let achado = match (&alvo.tipo, self.escopo.tipos) {
                    (Some(t), Some((r, arquivo))) if t != "dynamic" => r.tipo_do_membro_livre(
                        alvo.escopo.as_deref().unwrap_or(arquivo),
                        t,
                        nome,
                        self.escopo.genericos,
                    ),
                    _ => None,
                };
                // A receiver of a type that does not exist in scope
                // (`InvalidType`: the class of a file the ngdart phase does not
                // see yet) also gives `dynamic`.
                let inexistente = match (&alvo.tipo, self.escopo.tipos) {
                    (Some(t), Some((r, arquivo))) => {
                        r.tipo_inexistente(alvo.escopo.as_deref().unwrap_or(arquivo), t)
                    }
                    _ => false,
                };
                let (tipo, escopo) = match achado {
                    Some((t, e)) => (Some(t), Some(e)),
                    None if alvo.tipo.as_deref() == Some("dynamic") || inexistente => {
                        (Some("dynamic".to_string()), None)
                    }
                    None => (None, None),
                };
                // The receiver is not implicit: `a.b` is always mutable.
                Ok(Convertida {
                    tipo,
                    escopo,
                    locais: alvo.locais,
                    ..Convertida::nova(
                        format!("{}{ponto}{nome}", alvo.texto),
                        if *null_aware { "`?.`" } else { "propriedade" },
                    )
                })
            }
            ast::ExprKind::Call { target, arguments } => {
                if !arguments.type_args.is_empty() {
                    return Err(fora("call with a type argument"));
                }
                if let ast::ExprKind::Property {
                    target: t,
                    name,
                    null_aware: false,
                } = &self.ast.expr(*target).kind
                    && raiz
                    && matches!(&self.ast.expr(*t).kind,
                        ast::ExprKind::Identifier(n) if self.interner.resolve(n.sym) == "$pipe")
                {
                    return self.pipe(self.interner.resolve(name.sym), arguments);
                }
                // `visitMethodCall` converts the arguments **before** the
                // receiver: that is the order in which the locals are
                // requested.
                let mut args = Vec::new();
                let mut locais_args = Vec::new();
                for a in arguments.args.iter() {
                    let v = self.expr(a.value, true)?;
                    locais_args.extend(v.locais);
                    match &a.name {
                        Some(n) => {
                            args.push(format!("{}: {}", self.interner.resolve(n.sym), v.texto))
                        }
                        None => args.push(v.texto),
                    }
                }
                // With a named argument the list ends in a comma and the
                // formatter puts one argument per line, two spaces in (and
                // whatever already broke inside them, along with it). In a list
                // without a named argument, an argument that breaks would get
                // the continuation indentation — a shape without a case in the
                // corpus.
                let nomeado = arguments.args.iter().any(|a| a.name.is_some());
                if !nomeado && args.iter().any(|a| a.contains(QUEBRA)) {
                    return Err(fora("call with a named argument inside an argument"));
                }
                let lista = if nomeado {
                    let mut l: String = args
                        .iter()
                        .map(|a| format!("{QUEBRA}{RECUO}{RECUO}{},", recuar(a, 2)))
                        .collect();
                    l.push(QUEBRA);
                    l
                } else {
                    args.join(", ")
                };
                // The target of a call may be a class method, which is not
                // valid as a bare value. The type is that of `_TypeResolver`:
                // the method's return (`visitMethodCall`); a field or local
                // called as a function is not a method, and gives `dynamic`.
                let mut escopo_do_retorno = None;
                let (alvo, retorno, locais_alvo) = match &self.ast.expr(*target).kind {
                    ast::ExprKind::Identifier(n)
                        if raiz
                            && !self
                                .escopo
                                .locais
                                .contains_key(self.interner.resolve(n.sym)) =>
                    {
                        let nome = self.interner.resolve(n.sym);
                        let exportada = self
                            .escopo
                            .exportados
                            .and_then(|e| e.get(nome))
                            .filter(|(_, o)| *o == crate::resolucao::Exportado::Funcao);
                        match self.escopo.metodos.get(nome) {
                            // An `exports:` function: through the import,
                            // `dynamic`.
                            _ if exportada.is_some() => (
                                format!(
                                    "{}{nome}",
                                    exportada.map(|(q, _)| q.as_str()).unwrap_or_default()
                                ),
                                Some("dynamic".to_string()),
                                Vec::new(),
                            ),
                            Some(t) => (format!("_ctx.{nome}"), Some(t.clone()), Vec::new()),
                            // Inherited method: the return type in the scope of
                            // whoever declares it; without a written type,
                            // unknown.
                            None if !self.escopo.membros.contains_key(nome)
                                && let Some(m) = self.metodo_herdado(nome) =>
                            {
                                let (t, e) = m.retorno.unzip();
                                escopo_do_retorno = e;
                                (format!("_ctx.{nome}"), t, Vec::new())
                            }
                            None => {
                                let t = self.expr(*target, raiz)?;
                                (t.texto, Some("dynamic".to_string()), t.locais)
                            }
                        }
                    }
                    // A local called as a function: the type is still looked up
                    // as a class method (`visitMethodCall` looks at the name).
                    ast::ExprKind::Identifier(n) => {
                        let t = self.expr(*target, raiz)?;
                        let tipo = self
                            .escopo
                            .metodos
                            .get(self.interner.resolve(n.sym))
                            .cloned()
                            .unwrap_or_else(|| "dynamic".to_string());
                        (t.texto, Some(tipo), t.locais)
                    }
                    // `a.m()`: the semantic database answers the member `m` of
                    // the type of `a` — the return type, when it is a method.
                    _ => {
                        let t = self.expr(*target, raiz)?;
                        (t.texto, t.tipo, t.locais)
                    }
                };
                Ok(Convertida {
                    tipo: retorno,
                    escopo: escopo_do_retorno,
                    locais: juntar(&[&locais_args, &locais_alvo]),
                    ..Convertida::nova(format!("{alvo}({lista})"), "call")
                })
            }
            ast::ExprKind::Unary { op, operand } => {
                // `!`, `-x` and `x!`. ngcompiler's expression parser reads `-x`
                // as `0 - x` (output `(0 - _ctx.x)`) and `x!` as `(x!)`; `~`
                // does not even exist there.
                // `x!` is `PostfixNotNull`: `.notNull()`, which the emitter
                // writes as `(x!)`; type `dynamic`, mutable.
                if *op == ast::UnaryOp::NullAssert {
                    let v = self.expr(*operand, raiz)?;
                    return Ok(Convertida {
                        tipo: Some("dynamic".into()),
                        locais: v.locais,
                        ..Convertida::nova(format!("({}!)", v.texto), "`x!`")
                    });
                }
                // `-x` is `Binary('-', 0, x)` (`analyzer_parser.dart:607-609`):
                // output `(0 - x)`, immutable if `x` is, "may be null" and
                // `dynamic`, like any binary.
                if *op == ast::UnaryOp::Neg {
                    let v = self.expr(*operand, raiz)?;
                    return Ok(Convertida {
                        imutavel: v.imutavel,
                        tipo: Some("dynamic".into()),
                        locais: v.locais,
                        ..Convertida::nova(format!("(0 - {})", v.texto), "`-x`")
                    });
                }
                if *op != ast::UnaryOp::Not {
                    return Err(fora("unary operator outside the template"));
                }
                let v = self.expr(*operand, raiz)?;
                let op = self.operador_unario(id);
                // `PrefixNot` is not part of `isImmutable`: it is mutable.
                Ok(Convertida {
                    tipo: Some("dynamic".into()),
                    locais: v.locais,
                    ..Convertida::nova(format!("({op}{})", v.texto), "`!`")
                })
            }
            ast::ExprKind::Binary { op, .. } if !operador_do_template(*op) => {
                // `a | b` in the template is a pipe, not a bitwise OR:
                // translating it as Dart would give `(_ctx.a | _ctx.b)`, which
                // compiles and does something else. The others (`&`, `^`, `<<`,
                // `~/`…) do not exist in ngdart's expression language.
                Err(if *op == ast::BinaryOp::BitOr {
                    recusa(Motivo::PipesUsados, "pipe `x | nome`")
                } else {
                    fora("binary operator outside the template")
                })
            }
            ast::ExprKind::Binary { op, left, right } => {
                let a = self.expr(*left, raiz)?;
                let b = self.expr(*right, raiz)?;
                let texto_op = self.operador_binario(id);
                let se_nulo = *op == ast::BinaryOp::IfNull;
                // `canBeNull(IfNull)`: not null if the left one is not;
                // otherwise, the right one decides. A plain binary is always
                // "may be null" for the official compiler.
                let pode_ser_nulo = if se_nulo {
                    a.pode_ser_nulo && b.pode_ser_nulo
                } else {
                    true
                };
                // `visitBinary`: `String + String` is `String`; the rest,
                // `dynamic`. `??` is `dynamic`.
                let tipo = if *op == ast::BinaryOp::Add
                    && a.tipo.as_deref() == Some("String")
                    && b.tipo.as_deref() == Some("String")
                {
                    "String"
                } else {
                    "dynamic"
                };
                Ok(Convertida {
                    // `isImmutable` of `Binary` and `IfNull`: both sides.
                    imutavel: a.imutavel && b.imutavel,
                    pode_ser_nulo,
                    tipo: Some(tipo.into()),
                    locais: juntar(&[&a.locais, &b.locais]),
                    ..Convertida::nova(
                        format!("({} {texto_op} {})", a.texto, b.texto),
                        if se_nulo { "`??`" } else { "binary" },
                    )
                })
            }
            ast::ExprKind::Conditional {
                condition,
                then,
                else_,
            } => {
                let c = self.expr(*condition, raiz)?;
                let t = self.expr(*then, raiz)?;
                let f = self.expr(*else_, raiz)?;
                Ok(Convertida {
                    tipo: Some("dynamic".into()),
                    locais: juntar(&[&c.locais, &t.locais, &f.locais]),
                    ..Convertida::nova(
                        format!("({} ? {} : {})", c.texto, t.texto, f.texto),
                        "ternary",
                    )
                })
            }
            ast::ExprKind::Parenthesized(inner) => self.expr(*inner, raiz),
            // Inside another expression the assignment gets parentheses.
            ast::ExprKind::Assign { .. } => {
                let a = self.atribuicao(id)?;
                Ok(Convertida {
                    texto: format!("({})", a.texto),
                    ..a
                })
            }
            // `KeyedRead`: `receiver.key(key)`, without parentheses; `dynamic`.
            ast::ExprKind::Index {
                target,
                index,
                null_aware: false,
            } => {
                let r = self.expr(*target, raiz)?;
                let k = self.expr(*index, true)?;
                Ok(Convertida {
                    tipo: Some("dynamic".into()),
                    locais: juntar(&[&r.locais, &k.locais]),
                    ..Convertida::nova(format!("{}[{}]", r.texto, k.texto), "index")
                })
            }
            ast::ExprKind::Index { .. } => Err(fora("index `a?[i]`")),
            ast::ExprKind::List { .. } | ast::ExprKind::SetOrMap { .. } => {
                Err(fora("collection literal"))
            }
            ast::ExprKind::FunctionExpression(_) => Err(fora("anonymous function")),
            ast::ExprKind::Is { .. } | ast::ExprKind::As { .. } => Err(fora("`is`/`as`")),
            _ => Err(fora("expression shape outside the template")),
        }
    }

    fn texto(&self, id: ast::ExprId) -> String {
        let s = self.ast.expr(id).span;
        self.fonte.get(s.start..s.end).unwrap_or("").to_string()
    }

    /// O operador como escrito na fonte (o primeiro caractere do trecho).
    fn operador_unario(&self, id: ast::ExprId) -> String {
        let t = self.texto(id);
        match t.chars().next() {
            Some('!') => "!".into(),
            Some('-') => "-".into(),
            Some('~') => "~".into(),
            _ => String::new(),
        }
    }

    /// The binary operator as written, extracted from between the two operands.
    fn operador_binario(&self, id: ast::ExprId) -> String {
        let ast::ExprKind::Binary { left, right, .. } = &self.ast.expr(id).kind else {
            return String::new();
        };
        let fim_esq = self.ast.expr(*left).span.end;
        let ini_dir = self.ast.expr(*right).span.start;
        self.fonte
            .get(fim_esq..ini_dir)
            .unwrap_or("")
            .trim()
            .to_string()
    }
}

/// Binary operators of ngdart's expression language
/// (`expression_parser/parser.dart`): arithmetic, comparison, logical and `??`.
/// What Dart has in addition does not pass.
fn operador_do_template(op: ast::BinaryOp) -> bool {
    use ast::BinaryOp as B;
    matches!(
        op,
        B::Add
            | B::Sub
            | B::Mul
            | B::Div
            | B::Rem
            | B::Eq
            | B::NotEq
            | B::Lt
            | B::Gt
            | B::LtEq
            | B::GtEq
            | B::And
            | B::Or
            | B::IfNull
    )
}

#[cfg(test)]
mod testes {
    use super::*;

    fn membros() -> HashMap<String, Membro> {
        HashMap::from([
            (
                "item".to_string(),
                Membro {
                    tipo: "Item".into(),
                    imutavel: false,
                    estatico: false,
                },
            ),
            (
                "nome".to_string(),
                Membro {
                    tipo: "String".into(),
                    imutavel: false,
                    estatico: false,
                },
            ),
            (
                "fixo".to_string(),
                Membro {
                    tipo: "int".into(),
                    imutavel: true,
                    estatico: false,
                },
            ),
            (
                "titulo".to_string(),
                Membro {
                    tipo: "String".into(),
                    imutavel: true,
                    estatico: false,
                },
            ),
        ])
    }

    fn conv(e: &str) -> Convertida {
        converter(e, &membros(), &mut Interner::new()).expect("converte")
    }

    /// The shapes that corpus case c05 showed coming out of the official
    /// compiler.
    #[test]
    fn formas_do_oficial() {
        assert_eq!(conv("item.nome").texto, "_ctx.item.nome");
        assert_eq!(conv("!item.ativo").texto, "(!_ctx.item.ativo)");
        assert_eq!(conv("'fixo'").texto, "'fixo'");
        assert_eq!(conv("\"fixo\"").texto, "'fixo'");
        assert!(conv("'fixo'").imutavel);
    }

    #[test]
    fn tipo_do_membro_direto() {
        assert_eq!(conv("nome").tipo.as_deref(), Some("String"));
        assert!(!conv("nome").imutavel);
        assert!(conv("fixo").imutavel);
    }

    /// The official `isImmutable`: `a.b`, `!x` and a ternary are mutable even
    /// with everything `final`; `a ?? b` and a binary of immutables are not.
    #[test]
    fn imutabilidade_como_no_oficial() {
        assert!(!conv("titulo.length").imutavel);
        assert!(!conv("!fixo").imutavel);
        assert!(!conv("fixo > 1 ? 1 : 2").imutavel);
        assert!(conv("fixo ?? 1").imutavel);
        assert!(conv("fixo + 1").imutavel);
    }

    /// `canBeNull`: only a literal escapes, and `a ?? b` with a non-null side.
    #[test]
    fn nulidade_como_no_oficial() {
        assert!(!conv("'x'").pode_ser_nulo);
        assert!(!conv("1").pode_ser_nulo);
        assert!(conv("fixo").pode_ser_nulo);
        assert!(conv("fixo + 1").pode_ser_nulo);
        assert!(!conv("fixo ?? 1").pode_ser_nulo);
    }

    #[test]
    fn binario_e_condicional_com_parenteses() {
        assert_eq!(conv("nome == 'x'").texto, "(_ctx.nome == 'x')");
        assert_eq!(
            conv("fixo > 1 ? nome : 'y'").texto,
            "((_ctx.fixo > 1) ? _ctx.nome : 'y')"
        );
    }

    /// `|` in the template is a pipe: refused at any depth, and `||` is still
    /// the logical OR.
    #[test]
    fn pipe_e_recusado_e_ou_logico_passa() {
        let m = &membros();
        let mut i = Interner::new();
        assert_eq!(
            converter("nome | fixo", m, &mut i).map_err(|r| r.motivo),
            Err(Motivo::PipesUsados)
        );
        assert_eq!(
            converter("(nome | fixo) == 'x'", m, &mut i).map_err(|r| r.motivo),
            Err(Motivo::PipesUsados)
        );
        assert_eq!(
            conv("fixo > 1 || fixo < 0").texto,
            "((_ctx.fixo > 1) || (_ctx.fixo < 0))"
        );
    }

    /// `$pipe.name(input, args)` becomes the mark with the name and the number
    /// of arguments, type `dynamic`, also inside another pipe; with a named
    /// argument, it refuses.
    #[test]
    fn pipe_vira_marca() {
        let c = conv("$pipe.date(fixo, 'dd/MM')");
        assert_eq!(c.texto, "\u{3}date/2\u{4}(_ctx.fixo, 'dd/MM')");
        assert_eq!(c.tipo.as_deref(), Some("dynamic"));
        assert!(!c.imutavel);
        assert_eq!(
            conv("$pipe.a($pipe.b(fixo))").texto,
            "\u{3}a/1\u{4}(\u{3}b/1\u{4}(_ctx.fixo))"
        );
        let m = &membros();
        let mut i = Interner::new();
        assert_eq!(
            converter("$pipe.a(fixo, x: 1)", m, &mut i).map_err(|r| r.motivo),
            Err(Motivo::PipesUsados)
        );
    }

    /// What ngdart's parser reads differently from Dart: `-x` is `0 - x` there,
    /// `&` does not exist. `x!` comes out as `(x!)` and the index without
    /// parentheses.
    #[test]
    fn operadores_fora_do_template_sao_recusados() {
        let m = &membros();
        let mut i = Interner::new();
        assert_eq!(conv("-fixo").texto, "(0 - _ctx.fixo)");
        assert!(converter("fixo & 1", m, &mut i).is_err());
        assert_eq!(conv("nome!").texto, "(_ctx.nome!)");
        assert_eq!(conv("item[fixo]").texto, "_ctx.item[_ctx.fixo]");
    }

    /// A name that does not belong to the component does not become `_ctx.name`
    /// by mistake.
    #[test]
    fn nome_desconhecido_e_recusado() {
        assert!(converter("outro", &membros(), &mut Interner::new()).is_err());
    }

    fn acao(e: &str) -> Acao {
        let m = membros();
        let metodos = HashMap::from([
            ("salvar".to_string(), "void".to_string()),
            ("ir".to_string(), "void".to_string()),
        ]);
        let aridades = HashMap::from([("salvar".to_string(), 0), ("ir".to_string(), 1)]);
        let locais = HashMap::from([(
            "x".to_string(),
            Local {
                dart: "local_x".into(),
                tipo: "int".into(),
                escopo: None,
            },
        )]);
        let escopo = Escopo {
            membros: &m,
            metodos: &metodos,
            aridades: &aridades,
            locais: &locais,
            tipos: None,
            classe: None,
            exportados: None,
            genericos: &[],
        };
        converter_acao(e, &escopo, &mut Interner::new()).expect("converte")
    }

    /// `handlerTypeFromExpression` e `rewriteTearOff`.
    #[test]
    fn handlers_simples_e_tear_off() {
        assert!(matches!(acao("salvar()"), Acao::Simples { aridade: 0, .. }));
        assert!(matches!(
            acao("ir($event)"),
            Acao::Simples { aridade: 1, .. }
        ));
        assert!(matches!(acao("salvar"), Acao::Simples { aridade: 0, .. }));
        assert!(matches!(acao("ir"), Acao::Simples { aridade: 1, .. }));
    }

    /// Complex ones: top-level assignment without parentheses, arguments,
    /// locals in the order the official compiler asks for them (arguments
    /// before the receiver).
    #[test]
    fn handlers_complexos() {
        let Acao::Complexa(c) = acao("nome = $event") else {
            panic!()
        };
        assert_eq!(c.texto, "_ctx.nome = $event");
        let Acao::Complexa(c) = acao("ir(x)") else {
            panic!()
        };
        assert_eq!(c.texto, "_ctx.ir(local_x)");
        assert_eq!(c.locais, vec!["x".to_string()]);
        let Acao::Complexa(c) = acao("$event.stopPropagation()") else {
            panic!()
        };
        assert_eq!(c.texto, "$event.stopPropagation()");
    }
}

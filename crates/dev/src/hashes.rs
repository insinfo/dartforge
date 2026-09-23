//! Dois hashes por biblioteca: conteúdo e API pública.
//!
//! O PLANO fixa a regra de invalidação: corpo mudou e API não → reanalisa e
//! reemite **só** aquela biblioteca; API mudou → ela e quem a importa.
//!
//! O hash de API é **estrutural**: percorre a árvore sintática e mistura
//! nomes, modificadores e tipos escritos das declarações e membros públicos —
//! nunca faixas de texto. Um `print("x")` a mais dentro de um método desloca
//! todos os spans seguintes; se o hash olhasse posições ou trechos, cada tecla
//! num corpo invalidaria os dependentes, que é justamente o que este item
//! existe para evitar. Comentários, espaços e a ordem dos corpos não aparecem.
//!
//! Exceção deliberada: o **valor** de uma declaração `const` entra no hash,
//! porque um dependente pode usá-lo em contexto constante (`case`, argumento
//! de anotação, tamanho de literal). Entra como o texto do próprio
//! inicializador — um intervalo que só muda quando o valor muda.
//!
//! Membros privados (`_x`) não entram: nenhum dependente os enxerga.
use dartforge_elements::model::{LibraryId, Program};
use dartforge_frontend::ast::{
    self, Ast, DeclKind, FunctionBody, MemberKind, Parameter, TypeKind, TypeParameter, VariableList,
};

/// FNV-1a de 64 bits: barato e estável entre execuções (o `DefaultHasher` do
/// Rust não promete estabilidade entre versões).
#[derive(Clone, Copy)]
pub struct Fnv(u64);

impl Default for Fnv {
    fn default() -> Self {
        Fnv(0xcbf2_9ce4_8422_2325)
    }
}

impl Fnv {
    pub fn bytes(&mut self, b: &[u8]) {
        for x in b {
            self.0 ^= u64::from(*x);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    /// Marca de separação, para que `ab` + `c` não colida com `a` + `bc`.
    pub fn sep(&mut self) {
        self.bytes(&[0xff]);
    }
    pub fn bool(&mut self, b: bool) {
        self.bytes(&[u8::from(b)]);
    }
    pub fn num(&mut self, n: u64) {
        self.bytes(&n.to_le_bytes());
    }
    pub fn valor(self) -> u64 {
        self.0
    }
}

/// Os dois hashes de uma biblioteca.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HashesBiblioteca {
    pub conteudo: u64,
    pub api: u64,
}

/// Calcula os dois hashes de uma biblioteca (unidade principal, partes e
/// patches, na ordem em que estão no `Program`).
pub fn hashes_da_biblioteca(program: &Program, lib: LibraryId) -> HashesBiblioteca {
    let mut conteudo = Fnv::default();
    let mut api = Fnv::default();
    // A versão de linguagem muda o significado do texto (curingas, atalhos de
    // ponto, construtores primários) e pode mudar sem tocar na fonte, pelo
    // `languageVersion` do `package_config`: entra nos dois hashes.
    let versao = program.library(lib).features.versao();
    for h in [&mut conteudo, &mut api] {
        h.num(u64::from(versao.major) << 16 | u64::from(versao.minor));
        h.sep();
    }
    for &uid in &program.library(lib).units {
        let u = program.unit(uid);
        conteudo.bytes(u.source.as_bytes());
        conteudo.sep();
        let mut h = Hasher { fonte: &u.source, ast: &u.ast, f: &mut api };
        // Diretivas fazem parte da API: trocar um import muda o que a
        // biblioteca vê e o que ela reexporta. São linhas curtas e sem corpo,
        // então o texto da própria diretiva serve de forma canônica.
        for d in &u.unit.directives {
            let t = h.trecho(d.span.start, d.span.end).to_string();
            h.f.bytes(t.as_bytes());
            h.f.sep();
        }
        for &decl in &u.unit.declarations {
            h.decl(decl);
        }
    }
    HashesBiblioteca { conteudo: conteudo.valor(), api: api.valor() }
}

/// Percorre a árvore misturando só o que é visível de fora.
struct Hasher<'a> {
    fonte: &'a str,
    ast: &'a Ast,
    f: &'a mut Fnv,
}

impl<'a> Hasher<'a> {
    /// Trecho seguro da fonte (spans vêm do lexer e podem passar do fim em
    /// recuperação de erro).
    fn trecho(&self, inicio: usize, fim: usize) -> &'a str {
        let fim = fim.min(self.fonte.len());
        let inicio = inicio.min(fim);
        self.fonte.get(inicio..fim).unwrap_or("")
    }

    fn nome(&mut self, n: &ast::Name) {
        let t = self.trecho(n.span.start, n.span.end);
        self.f.bytes(t.as_bytes());
        self.f.sep();
    }

    fn texto(&mut self, s: &str) {
        self.f.bytes(s.as_bytes());
        self.f.sep();
    }

    /// Nome de uma declaração, para decidir se é pública.
    fn texto_do_nome(&self, n: &ast::Name) -> &'a str {
        self.trecho(n.span.start, n.span.end)
    }

    fn privado(&self, n: &ast::Name) -> bool {
        self.texto_do_nome(n).starts_with('_')
    }

    /// Tipo escrito, na forma como aparece na fonte (nome, argumentos,
    /// nulabilidade), sem depender de posição.
    fn tipo(&mut self, id: ast::TypeId) {
        let t = self.ast.ty(id);
        self.f.bool(t.nullable);
        match &t.kind {
            TypeKind::Named { name, args } => {
                self.texto("t:nome");
                for n in name.iter() {
                    self.nome(n);
                }
                self.f.num(args.len() as u64);
                for &a in args.iter() {
                    self.tipo(a);
                }
            }
            TypeKind::Void => self.texto("t:void"),
            TypeKind::Function { return_type, type_params, parameters } => {
                self.texto("t:fn");
                self.tipo_opcional(*return_type);
                self.parametros_de_tipo(type_params);
                self.parametros(parameters);
            }
            TypeKind::Record { positional, named } => {
                self.texto("t:record");
                self.f.num(positional.len() as u64);
                for &p in positional.iter() {
                    self.tipo(p);
                }
                for (n, ty) in named.iter() {
                    self.nome(n);
                    self.tipo(*ty);
                }
            }
        }
    }

    fn tipo_opcional(&mut self, id: Option<ast::TypeId>) {
        match id {
            Some(t) => self.tipo(t),
            None => self.texto("t:ausente"),
        }
    }

    fn parametros_de_tipo(&mut self, ps: &[TypeParameter]) {
        self.f.num(ps.len() as u64);
        for p in ps {
            self.nome(&p.name);
            self.tipo_opcional(p.bound);
        }
    }

    /// Parâmetros formais: o que um chamador precisa saber. O valor padrão
    /// não entra — o Dart o avalia no próprio corpo chamado, então mudá-lo
    /// recompila a biblioteca que o declara, não quem chama.
    fn parametros(&mut self, ps: &[Parameter]) {
        self.f.num(ps.len() as u64);
        for p in ps {
            self.f.num(match p.kind {
                ast::ParameterKind::Required => 0,
                ast::ParameterKind::Optional => 1,
                ast::ParameterKind::Named => 2,
            });
            self.f.bool(p.required);
            self.f.bool(p.covariant);
            self.f.bool(p.this_);
            self.f.bool(p.super_);
            self.f.bool(p.default_value.is_some());
            match &p.name {
                Some(n) => self.nome(n),
                None => self.texto("p:sem-nome"),
            }
            self.tipo_opcional(p.ty);
            self.parametros_de_tipo(&p.function_type_params);
            if let Some(fp) = &p.function_parameters {
                self.texto("p:fn");
                self.parametros(fp);
            }
        }
    }

    /// Assinatura de uma função/método/getter/setter/operador.
    fn funcao(&mut self, fid: ast::FunctionId) {
        let f = self.ast.function(fid);
        self.f.num(match f.kind {
            ast::FunctionKind::Function => 0,
            ast::FunctionKind::Getter => 1,
            ast::FunctionKind::Setter => 2,
            ast::FunctionKind::Operator => 3,
        });
        self.f.bool(f.external);
        self.f.bool(f.static_);
        // O modificador `async`/`sync*` muda o tipo de retorno visto de fora
        // (`Future`, `Stream`, `Iterable`), então conta como assinatura.
        self.f.num(match f.modifier {
            ast::AsyncModifier::None => 0,
            ast::AsyncModifier::Async => 1,
            ast::AsyncModifier::AsyncStar => 2,
            ast::AsyncModifier::SyncStar => 3,
        });
        // Ter corpo ou não é visível (abstrato/externo vs concreto).
        self.f.bool(matches!(f.body, FunctionBody::Empty));
        self.tipo_opcional(f.return_type);
        self.parametros_de_tipo(&f.type_params);
        match &f.parameters {
            Some(ps) => self.parametros(ps),
            None => self.texto("f:sem-lista"),
        }
    }

    /// Declarações de variável/campo: modificadores, tipo e nomes públicos.
    /// Em `const`, o valor entra pelo texto do próprio inicializador.
    fn variaveis(&mut self, lista: &VariableList, prefixo: &str) {
        let mut algum = false;
        for v in lista.variables.iter() {
            if self.privado(&v.name) {
                continue;
            }
            if !algum {
                self.texto(prefixo);
                self.f.bool(lista.external);
                self.f.bool(lista.static_);
                self.f.bool(lista.abstract_);
                self.f.bool(lista.covariant);
                self.f.bool(lista.late);
                self.f.bool(lista.final_);
                self.f.bool(lista.const_);
                self.f.bool(lista.var_);
                self.tipo_opcional(lista.ty);
                algum = true;
            }
            self.nome(&v.name);
            self.f.bool(v.initializer.is_some());
            if lista.const_ {
                if let Some(e) = v.initializer {
                    let s = self.ast.expr(e).span;
                    let t = self.trecho(s.start, s.end).to_string();
                    self.f.bytes(t.as_bytes());
                    self.f.sep();
                }
            }
        }
    }

    /// Declaração de topo.
    fn decl(&mut self, decl: ast::DeclId) {
        let d = self.ast.decl(decl);
        match &d.kind {
            DeclKind::Function(fid) => {
                let f = self.ast.function(*fid);
                let Some(nome) = f.name else { return };
                if self.privado(&nome) {
                    return;
                }
                self.texto("d:fn");
                self.nome(&nome);
                self.funcao(*fid);
            }
            DeclKind::Variables(lista) => self.variaveis(lista, "d:var"),
            DeclKind::Class(c) => {
                if self.privado(&c.name) {
                    return;
                }
                self.texto("d:classe");
                self.nome(&c.name);
                let m = c.modifiers;
                for b in [m.abstract_, m.base, m.interface, m.final_, m.sealed, m.mixin, c.mixin_application] {
                    self.f.bool(b);
                }
                self.parametros_de_tipo(&c.type_params);
                self.tipo_opcional(c.extends);
                self.tipos(&c.with);
                self.tipos(&c.implements);
                self.membros(&c.members);
            }
            DeclKind::Mixin(mx) => {
                if self.privado(&mx.name) {
                    return;
                }
                self.texto("d:mixin");
                self.nome(&mx.name);
                self.f.bool(mx.base);
                self.parametros_de_tipo(&mx.type_params);
                self.tipos(&mx.on);
                self.tipos(&mx.implements);
                self.membros(&mx.members);
            }
            DeclKind::Enum(e) => {
                if self.privado(&e.name) {
                    return;
                }
                self.texto("d:enum");
                self.nome(&e.name);
                self.parametros_de_tipo(&e.type_params);
                self.tipos(&e.with);
                self.tipos(&e.implements);
                // Nome e ordem das constantes são API (o índice é observável).
                self.f.num(e.constants.len() as u64);
                for c in &e.constants {
                    self.nome(&c.name);
                }
                self.membros(&e.members);
            }
            DeclKind::Extension(e) => {
                if e.name.is_some_and(|n| self.privado(&n)) {
                    return;
                }
                self.texto("d:extension");
                if let Some(n) = e.name {
                    self.nome(&n);
                }
                self.parametros_de_tipo(&e.type_params);
                self.tipo(e.on);
                self.membros(&e.members);
            }
            DeclKind::ExtensionType(e) => {
                if self.privado(&e.name) {
                    return;
                }
                self.texto("d:extension-type");
                self.nome(&e.name);
                self.f.bool(e.const_);
                if let Some(c) = e.constructor {
                    self.nome(&c);
                }
                self.parametros_de_tipo(&e.type_params);
                self.tipo(e.representation_type);
                self.nome(&e.representation_name);
                self.tipos(&e.implements);
                self.membros(&e.members);
            }
            DeclKind::Typedef(t) => {
                if self.privado(&t.name) {
                    return;
                }
                self.texto("d:typedef");
                self.nome(&t.name);
                self.parametros_de_tipo(&t.type_params);
                match &t.kind {
                    ast::TypedefKind::Alias(ty) => self.tipo(*ty),
                    ast::TypedefKind::Legacy { return_type, parameters } => {
                        self.tipo_opcional(*return_type);
                        self.parametros(parameters);
                    }
                }
            }
        }
    }

    fn tipos(&mut self, ts: &[ast::TypeId]) {
        self.f.num(ts.len() as u64);
        for &t in ts {
            self.tipo(t);
        }
    }

    /// Membros públicos de uma classe, mixin, enum, extension ou extension type.
    fn membros(&mut self, membros: &[ast::MemberId]) {
        for &mid in membros {
            let m = self.ast.member(mid);
            match &m.kind {
                MemberKind::Method(fid) => {
                    let f = self.ast.function(*fid);
                    let Some(nome) = f.name else { continue };
                    if self.privado(&nome) {
                        continue;
                    }
                    self.texto("m:metodo");
                    self.nome(&nome);
                    self.funcao(*fid);
                }
                MemberKind::Constructor(c) => {
                    if c.name.is_some_and(|n| self.privado(&n)) {
                        continue;
                    }
                    self.texto("m:ctor");
                    match &c.name {
                        Some(n) => self.nome(n),
                        None => self.texto("m:sem-nome"),
                    }
                    self.f.bool(c.external);
                    self.f.bool(c.const_);
                    self.f.bool(c.factory);
                    self.f.bool(c.redirect.is_some());
                    if let Some(r) = &c.redirect {
                        self.tipo(r.ty);
                        if let Some(n) = r.constructor {
                            self.nome(&n);
                        }
                    }
                    self.parametros(&c.parameters);
                    // Um construtor `const` é avaliado por quem o chama: a
                    // lista de inicializadores entra pelo texto de cada um.
                    if c.const_ {
                        for i in c.initializers.iter() {
                            let s = match i {
                                ast::Initializer::Field { span, .. }
                                | ast::Initializer::Super { span, .. }
                                | ast::Initializer::Redirect { span, .. }
                                | ast::Initializer::Assert { span, .. } => *span,
                            };
                            let t = self.trecho(s.start, s.end).to_string();
                            self.f.bytes(t.as_bytes());
                            self.f.sep();
                        }
                    }
                }
                MemberKind::Field(lista) => self.variaveis(lista, "m:campo"),
            }
        }
    }
}

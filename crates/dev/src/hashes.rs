//! Dois hashes por biblioteca: conteúdo e API pública.
//!
//! O PLANO fixa a regra de invalidação: corpo mudou e API não → reanalisa e
//! reemite **só** aquela biblioteca; API mudou → ela e quem a importa. Os
//! dois hashes saem da árvore sintática, não dos ids do `Program`: ids mudam
//! quando uma declaração é inserida antes de outra, e um hash que muda por
//! isso invalidaria o projeto inteiro a cada edição.
//!
//! A API pública é o texto de cada declaração de topo **sem os corpos** e sem
//! os membros privados — a mesma ideia das interfaces `.resi` do ReScript.
use dartforge_elements::model::{LibraryId, Program};
use dartforge_frontend::ast::{self, Ast, DeclKind, FunctionBody, MemberKind};

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
    pub fn sep(&mut self) {
        self.bytes(&[0xff]);
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
    for &uid in &program.library(lib).units {
        let u = program.unit(uid);
        conteudo.bytes(u.source.as_bytes());
        conteudo.sep();
        // Diretivas fazem parte da API: trocar um import muda o que a
        // biblioteca vê e o que ela reexporta.
        for d in &u.unit.directives {
            api.bytes(trecho(&u.source, d.span.start, d.span.end).as_bytes());
            api.sep();
        }
        for &decl in &u.unit.declarations {
            hash_decl(&u.source, &u.ast, decl, &mut api);
        }
    }
    HashesBiblioteca { conteudo: conteudo.valor(), api: api.valor() }
}

/// Trecho seguro da fonte (spans vêm do lexer e podem passar do fim em
/// recuperação de erro).
fn trecho(fonte: &str, inicio: usize, fim: usize) -> &str {
    let fim = fim.min(fonte.len());
    let inicio = inicio.min(fim);
    fonte.get(inicio..fim).unwrap_or("")
}

/// Nome privado (`_x`): não entra na API pública.
fn privado(nome: &str) -> bool {
    nome.starts_with('_')
}

/// Início do corpo de uma função, para cortar o texto na assinatura.
fn inicio_do_corpo(ast: &Ast, body: &FunctionBody) -> Option<usize> {
    match body {
        FunctionBody::Block(s) => Some(ast.stmt(*s).span.start),
        FunctionBody::Expression(e) => Some(ast.expr(*e).span.start),
        _ => None,
    }
}

/// Acrescenta ao hash da API o cabeçalho de uma declaração de topo.
fn hash_decl(fonte: &str, ast: &Ast, decl: ast::DeclId, api: &mut Fnv) {
    let d = ast.decl(decl);
    let (inicio, fim) = (d.span.start, d.span.end);
    match &d.kind {
        DeclKind::Function(fid) => {
            let f = ast.function(*fid);
            let nome = f.name.map(|n| n.span).map(|s| trecho(fonte, s.start, s.end)).unwrap_or("");
            if privado(nome) {
                return;
            }
            let corte = inicio_do_corpo(ast, &f.body).unwrap_or(fim);
            api.bytes(trecho(fonte, inicio, corte).as_bytes());
            api.sep();
        }
        DeclKind::Variables(lista) => {
            // Variável de topo: tipo e nome são API; o inicializador só quando
            // é `const` (o valor entra na compilação de quem usa).
            for v in lista.variables.iter() {
                let nome = trecho(fonte, v.name.span.start, v.name.span.end);
                if privado(nome) {
                    continue;
                }
                let corte = if lista.const_ {
                    fim
                } else {
                    v.initializer.map(|e| ast.expr(e).span.start).unwrap_or(fim)
                };
                api.bytes(trecho(fonte, inicio, corte).as_bytes());
                api.sep();
            }
        }
        DeclKind::Class(c) => hash_membros(fonte, ast, inicio, c.name.span.start, &c.members, api),
        DeclKind::Mixin(m) => hash_membros(fonte, ast, inicio, m.name.span.start, &m.members, api),
        DeclKind::Enum(e) => {
            // Constantes de enum são API (nome e ordem).
            api.bytes(trecho(fonte, inicio, e.constants.last().map(|c| c.span.end).unwrap_or(fim)).as_bytes());
            api.sep();
            hash_membros(fonte, ast, inicio, e.name.span.start, &e.members, api);
        }
        DeclKind::Extension(e) => hash_membros(fonte, ast, inicio, inicio, &e.members, api),
        DeclKind::ExtensionType(e) => hash_membros(fonte, ast, inicio, e.name.span.start, &e.members, api),
        DeclKind::Typedef(_) => {
            api.bytes(trecho(fonte, inicio, fim).as_bytes());
            api.sep();
        }
    }
}

/// Cabeçalho da classe (do início até o primeiro membro) e assinatura de cada
/// membro público.
fn hash_membros(
    fonte: &str,
    ast: &Ast,
    inicio_decl: usize,
    _inicio_nome: usize,
    membros: &[ast::MemberId],
    api: &mut Fnv,
) {
    let fim_cabecalho = membros
        .first()
        .map(|&m| ast.member(m).span.start)
        .unwrap_or(inicio_decl);
    api.bytes(trecho(fonte, inicio_decl, fim_cabecalho).as_bytes());
    api.sep();
    for &mid in membros {
        let m = ast.member(mid);
        match &m.kind {
            MemberKind::Method(fid) => {
                let f = ast.function(*fid);
                let nome = f.name.map(|n| n.span).map(|s| trecho(fonte, s.start, s.end)).unwrap_or("");
                if privado(nome) {
                    continue;
                }
                let corte = inicio_do_corpo(ast, &f.body).unwrap_or(m.span.end);
                api.bytes(trecho(fonte, m.span.start, corte).as_bytes());
                api.sep();
            }
            MemberKind::Constructor(c) => {
                let nome = c.name.map(|n| trecho(fonte, n.span.start, n.span.end)).unwrap_or("");
                if privado(nome) {
                    continue;
                }
                // Inicializadores e redirecionamento fazem parte do que um
                // `const` de outra biblioteca precisa; o corpo não.
                let corte = inicio_do_corpo(ast, &c.body).unwrap_or(m.span.end);
                api.bytes(trecho(fonte, m.span.start, corte).as_bytes());
                api.sep();
            }
            MemberKind::Field(lista) => {
                for v in lista.variables.iter() {
                    let nome = trecho(fonte, v.name.span.start, v.name.span.end);
                    if privado(nome) {
                        continue;
                    }
                    let corte = if lista.const_ {
                        m.span.end
                    } else {
                        v.initializer.map(|e| ast.expr(e).span.start).unwrap_or(m.span.end)
                    };
                    api.bytes(trecho(fonte, m.span.start, corte).as_bytes());
                    api.sep();
                }
            }
        }
    }
}

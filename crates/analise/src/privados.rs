//! Declarações privadas nunca referenciadas: a parte de biblioteca do
//! `UnusedLocalElementsVerifier` do analyzer 6.11.0
//! (`src/error/unused_local_elements_verifier.dart`) —
//! `UNUSED_ELEMENT` (classes, mixins, enums, tipos de extensão, typedefs,
//! funções, variáveis de topo, métodos, getters, setters e construtores
//! nomeados que não são acessíveis de fora da biblioteca) e `UNUSED_FIELD`
//! (campo privado, ou estático público de tipo privado, que nunca é lido).
//!
//! O analyzer liga cada referência ao seu elemento; esta camada não tem os
//! tipos dos receptores (`a._x`), então a ligação é **pelo nome**: uma
//! declaração só é relatada quando nenhum identificador com o nome dela
//! aparece na biblioteca fora das declarações (as de topo e de membros de
//! todas as unidades). Qualquer ocorrência conta como uso, mesmo que o
//! analyzer a ligasse a outro elemento: o erro possível é deixar de relatar
//! (falso negativo), nunca relatar um uso que existe. As exceções seguem
//! o analyzer:
//!
//! * campo e variável de topo: só **leitura** conta — `x = e`, `this.x` num
//!   construtor e `x = e` na lista de inicialização escrevem (`x += 1;` e
//!   `x++;` contam como leitura aqui, uma aproximação conservadora);
//! * classe (`class`, não mixin/enum): referências dentro da própria
//!   declaração não contam (`_enclosingClass`);
//! * comentários não contam (o lexer não os devolve);
//! * `values` em qualquer lugar lê todas as constantes de enum;
//! * construtor só é relatado se a classe tem mais de um;
//! * `@pragma(...)` e `@JS(...)` tornam a declaração usada.
//!
//! Bibliotecas com erro de sintaxe ou com `augment` ficam de fora.

use crate::Unidade;
use dartforge_diagnostics::codigos::warning as w;
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_frontend::ast::{self, DeclKind, Initializer, MemberKind};
use dartforge_frontend::token::{Kind, Op};
use dartforge_intern::Interner;
use std::collections::{HashMap, HashSet};

/// Um identificador na fonte.
struct Ocorrencia {
    unidade: usize,
    span: Span,
    escrita: bool,
}

/// O que se relata e como se decide o uso.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Regra {
    /// `UNUSED_ELEMENT`: qualquer referência usa.
    Elemento,
    /// `UNUSED_ELEMENT` de variável de topo: só leitura usa.
    VariavelDeTopo,
    /// `UNUSED_FIELD`: só leitura usa.
    Campo,
    /// `UNUSED_FIELD` de constante de enum: leitura ou `values`.
    ConstanteDeEnum,
}

struct Candidato {
    unidade: usize,
    /// O nome como aparece nos identificadores.
    nome: String,
    /// O que a mensagem mostra (`A.named` para construtor).
    exibido: String,
    span: Span,
    regra: Regra,
    /// Referências dentro deste intervalo não contam (classe).
    proprio: Option<Span>,
}

fn privado(nome: &str) -> bool {
    nome.starts_with('_')
}

/// Anotação que o analyzer trata como uso (`@pragma('vm:entry-point')`,
/// `@JS`): a declaração com ela fica de fora.
fn anotada(metadata: &[ast::Annotation], nomes: &Interner) -> bool {
    metadata.iter().any(|a| {
        a.name
            .iter()
            .any(|n| matches!(nomes.resolve(n.sym), "pragma" | "JS" | "js"))
    })
}

/// Coleta os candidatos e os intervalos de todas as declarações nomeadas.
struct Coleta<'a> {
    nomes: &'a Interner,
    candidatos: Vec<Candidato>,
    /// `(unidade, início)` dos nomes declarados (não são referências).
    declaracoes: HashSet<(usize, usize)>,
    /// `(unidade, início)` de identificadores que só escrevem.
    escritas: HashSet<(usize, usize)>,
    /// Algum membro `augment`: a biblioteca fica de fora.
    augment: bool,
    /// Tipos nomeados por um `typedef` público (`typedef P = _C;`).
    expostos: HashSet<String>,
    /// `(unidade, início)` dos campos declarados por parâmetro de construtor
    /// primário (3.13): o analyzer 3.13 os relata com o código próprio
    /// (`UNUSED_FIELD_FROM_PRIMARY_CONSTRUCTOR`), que o 3.6 não tem.
    de_primario: HashSet<(usize, usize)>,
}

impl Coleta<'_> {
    fn texto(&self, n: ast::Name) -> String {
        self.nomes.resolve(n.sym).to_string()
    }

    fn declarar(&mut self, u: usize, n: ast::Name) {
        self.declaracoes.insert((u, n.span.start));
    }

    fn candidato(
        &mut self,
        u: usize,
        n: ast::Name,
        exibido: String,
        regra: Regra,
        proprio: Option<Span>,
    ) {
        let nome = self.texto(n);
        self.candidatos.push(Candidato {
            unidade: u,
            nome,
            exibido,
            span: n.span,
            regra,
            proprio,
        });
    }

    /// Os membros de um tipo. `tipo_privado`: o tipo (ou a extensão) tem nome
    /// privado. `extensao`: membros de extensão (públicos só se ela for).
    #[allow(clippy::too_many_arguments)]
    fn membros(
        &mut self,
        u: usize,
        ast_: &ast::Ast,
        nome_tipo: &str,
        membros: &[ast::MemberId],
        tipo_privado: bool,
        enum_: bool,
        extensao: bool,
    ) {
        let construtores = membros
            .iter()
            .filter(|&&m| matches!(ast_.member(m).kind, MemberKind::Constructor(_)))
            .count();
        for &mid in membros {
            let m = ast_.member(mid);
            self.augment |= m.augment;
            let anot = anotada(&m.metadata, self.nomes);
            match &m.kind {
                MemberKind::Method(f) => {
                    let func = ast_.function(*f);
                    let Some(n) = func.name else { continue };
                    self.declarar(u, n);
                    let nome = self.texto(n);
                    // `_isPubliclyAccessible`.
                    let acessivel =
                        !privado(&nome) && !(tipo_privado && (func.static_ || extensao));
                    // Operadores e `call` se usam sem o nome (`-x`, `x[i]`,
                    // `x()`): a ligação pelo nome não os vê.
                    let sem_nome = func.kind == ast::FunctionKind::Operator || nome == "call";
                    if !acessivel && !anot && !sem_nome {
                        self.candidato(u, n, nome, Regra::Elemento, None);
                    }
                }
                MemberKind::Field(lista) => {
                    for v in lista.variables.iter() {
                        self.declarar(u, v.name);
                        let nome = self.texto(v.name);
                        // `_isReadMember`: público só conta se for estático de
                        // tipo privado.
                        let candidato = privado(&nome) || (lista.static_ && tipo_privado);
                        if candidato && !anot && !self.de_primario.contains(&(u, v.name.span.start)) {
                            self.candidato(u, v.name, nome, Regra::Campo, None);
                        }
                    }
                }
                MemberKind::Constructor(k) => {
                    for p in k.parameters.iter() {
                        if p.this_
                            && let Some(n) = p.name
                        {
                            self.escritas.insert((u, n.span.start));
                        }
                    }
                    for i in k.initializers.iter() {
                        if let Initializer::Field { name, .. } = i {
                            self.escritas.insert((u, name.span.start));
                        }
                    }
                    let Some(n) = k.name else { continue };
                    self.declarar(u, n);
                    let nome = self.texto(n);
                    // `C.new(…)` é o construtor sem nome (nunca relatado).
                    if nome == "new" {
                        continue;
                    }
                    // Um `typedef` público para o tipo usa os construtores
                    // públicos dele (`visitGenericTypeAlias`).
                    if !privado(&nome) && self.expostos.contains(nome_tipo) {
                        continue;
                    }
                    let acessivel = !privado(&nome) && !tipo_privado && !(enum_ && !k.factory);
                    if construtores > 1 && !acessivel && !anot && !k.parte_primaria {
                        self.candidato(u, n, format!("{nome_tipo}.{nome}"), Regra::Elemento, None);
                    }
                }
            }
        }
    }
}

/// Os diagnósticos de declarações privadas não usadas da biblioteca
/// formada por `unidades` (na ordem da biblioteca), com o índice da unidade.
/// `com_erro_de_sintaxe`: alguma unidade teve erro de sintaxe (a biblioteca
/// fica de fora).
/// Como [`nao_usados`], com a regra do T5 no lugar da porta por biblioteca:
/// uma declaração privada só fica de fora quando o nome dela é citado em
/// algum trecho que o parser pulou em qualquer unidade da biblioteca
/// (`pulados`, um por unidade, na mesma ordem).
pub fn nao_usados_com_pulados(
    unidades: &[Unidade<'_>],
    nomes: &Interner,
    pulados: &[crate::Pulados<'_>],
) -> Vec<(usize, Diagnostic)> {
    let mut saida = nao_usados(unidades, nomes, false);
    if pulados.iter().any(|p| !p.vazio()) {
        saida.retain(|(i, d)| {
            let nome = unidades.get(*i).and_then(|u| u.fonte.get(d.span.start..d.span.end)).unwrap_or("");
            !pulados.iter().any(|p| p.cita(nome))
        });
    }
    saida
}

pub fn nao_usados(
    unidades: &[Unidade<'_>],
    nomes: &Interner,
    com_erro_de_sintaxe: bool,
) -> Vec<(usize, Diagnostic)> {
    if com_erro_de_sintaxe {
        return Vec::new();
    }
    // `visitGenericTypeAlias`: o tipo de um `typedef` público, pelo nome.
    let mut expostos = HashSet::new();
    for un in unidades {
        for &d in un.unit.declarations.iter() {
            if let DeclKind::Typedef(t) = &un.ast.decl(d).kind
                && !privado(nomes.resolve(t.name.sym))
                && let ast::TypedefKind::Alias(corpo) = t.kind
                && let ast::TypeKind::Named { name, .. } = &un.ast.ty(corpo).kind
                && let Some(ultimo) = name.last()
            {
                expostos.insert(nomes.resolve(ultimo.sym).to_string());
            }
        }
    }
    let mut c = Coleta {
        nomes,
        candidatos: Vec::new(),
        declaracoes: HashSet::new(),
        escritas: HashSet::new(),
        augment: false,
        expostos,
        de_primario: HashSet::new(),
    };
    for (u, un) in unidades.iter().enumerate() {
        let ast_ = un.ast;
        for &d in un.unit.declarations.iter() {
            let decl = ast_.decl(d);
            if decl.augment {
                return Vec::new();
            }
            let anot = anotada(&decl.metadata, nomes);
            match &decl.kind {
                DeclKind::Class(k) => {
                    c.declarar(u, k.name);
                    let nome = c.texto(k.name);
                    let p = privado(&nome);
                    // O alias de classe (`class _A = S with M;`) não é
                    // visitado pelo verificador do analyzer: nunca é relatado.
                    if p && !anot && !k.mixin_application {
                        c.candidato(u, k.name, nome.clone(), Regra::Elemento, Some(decl.span));
                    }
                    if let Some(k2) = k.primary_constructor
                        && let MemberKind::Constructor(k2) = &ast_.member(k2).kind
                    {
                        c.de_primario.extend(k2.parameters.iter().filter_map(|p| p.name).map(|n| (u, n.span.start)));
                    }
                    c.membros(u, ast_, &nome, &k.members, p, false, false);
                }
                DeclKind::Mixin(k) => {
                    c.declarar(u, k.name);
                    let nome = c.texto(k.name);
                    let p = privado(&nome);
                    if p && !anot {
                        c.candidato(u, k.name, nome.clone(), Regra::Elemento, None);
                    }
                    c.membros(u, ast_, &nome, &k.members, p, false, false);
                }
                DeclKind::Enum(k) => {
                    c.declarar(u, k.name);
                    let nome = c.texto(k.name);
                    let p = privado(&nome);
                    if p && !anot {
                        c.candidato(u, k.name, nome.clone(), Regra::Elemento, None);
                    }
                    for k2 in &k.constants {
                        c.declarar(u, k2.name);
                        let n2 = c.texto(k2.name);
                        if (p || privado(&n2)) && !anotada(&k2.metadata, nomes) {
                            c.candidato(u, k2.name, n2, Regra::ConstanteDeEnum, None);
                        }
                    }
                    if k.primary_constructor.is_some() {
                        // Construtor primário (3.13): contagem de construtores
                        // e acessibilidade dependem da forma nova; não se decide.
                        continue;
                    }
                    c.membros(u, ast_, &nome, &k.members, p, true, false);
                }
                DeclKind::ExtensionType(k) => {
                    c.declarar(u, k.name);
                    c.declarar(u, k.representation_name);
                    if let Some(n) = k.constructor {
                        c.declarar(u, n);
                    }
                    let nome = c.texto(k.name);
                    let p = privado(&nome);
                    if p && !anot {
                        c.candidato(u, k.name, nome.clone(), Regra::Elemento, None);
                    }
                    // O construtor primário também conta para o analyzer;
                    // aqui só os escritos são contados, o que no máximo deixa
                    // de relatar um construtor (com um só escrito).
                    c.membros(u, ast_, &nome, &k.members, p, false, false);
                }
                DeclKind::Extension(k) => {
                    let (nome, p) = match k.name {
                        Some(n) => {
                            c.declarar(u, n);
                            let t = c.texto(n);
                            let p = privado(&t);
                            (t, p)
                        }
                        // Extensão sem nome: a acessibilidade dos membros
                        // públicos depende do nome vazio; só os privados.
                        None => (String::new(), false),
                    };
                    c.membros(u, ast_, &nome, &k.members, p, false, true);
                }
                DeclKind::Typedef(t) => {
                    c.declarar(u, t.name);
                    let nome = c.texto(t.name);
                    if privado(&nome) && !anot {
                        c.candidato(u, t.name, nome, Regra::Elemento, None);
                    }
                }
                DeclKind::Function(f) => {
                    let func = ast_.function(*f);
                    let Some(n) = func.name else { continue };
                    c.declarar(u, n);
                    let nome = c.texto(n);
                    if privado(&nome) && !anot {
                        c.candidato(u, n, nome, Regra::Elemento, None);
                    }
                }
                DeclKind::Variables(lista) => {
                    for v in lista.variables.iter() {
                        c.declarar(u, v.name);
                        let nome = c.texto(v.name);
                        if privado(&nome) && !anot {
                            c.candidato(u, v.name, nome, Regra::VariavelDeTopo, None);
                        }
                    }
                }
            }
        }
    }
    if c.candidatos.is_empty() || c.augment {
        return Vec::new();
    }

    // Os identificadores de todas as unidades.
    let mut ocorrencias: HashMap<&str, Vec<Ocorrencia>> = HashMap::new();
    for (u, un) in unidades.iter().enumerate() {
        let Ok(tokens) = dartforge_frontend::lexer::lex(un.fonte) else {
            return Vec::new();
        };
        for (i, t) in tokens.iter().enumerate() {
            if t.kind != Kind::Ident || c.declaracoes.contains(&(u, t.span.start)) {
                continue;
            }
            let atribuido = tokens
                .get(i + 1)
                .is_some_and(|p| p.kind == Kind::Op(Op::Assign));
            let escrita = atribuido || c.escritas.contains(&(u, t.span.start));
            ocorrencias
                .entry(t.text(un.fonte))
                .or_default()
                .push(Ocorrencia {
                    unidade: u,
                    span: t.span,
                    escrita,
                });
        }
    }
    let values_lido = ocorrencias.contains_key("values");

    let mut saida = Vec::new();
    for k in &c.candidatos {
        let usado = ocorrencias.get(k.nome.as_str()).is_some_and(|v| {
            v.iter().any(|o| {
                let dentro = k.proprio.is_some_and(|p| {
                    o.unidade == k.unidade && p.start <= o.span.start && o.span.end <= p.end
                });
                let conta = match k.regra {
                    Regra::Elemento => true,
                    Regra::VariavelDeTopo | Regra::Campo | Regra::ConstanteDeEnum => !o.escrita,
                };
                !dentro && conta
            })
        }) || (k.regra == Regra::ConstanteDeEnum && values_lido);
        if usado {
            continue;
        }
        let codigo = match k.regra {
            Regra::Elemento | Regra::VariavelDeTopo => w::UNUSED_ELEMENT,
            Regra::Campo | Regra::ConstanteDeEnum => w::UNUSED_FIELD,
        };
        saida.push((
            k.unidade,
            Diagnostic::com_codigo(codigo, k.span, [k.exibido.as_str()]),
        ));
    }
    saida
}

#[cfg(test)]
mod testes {
    use super::*;

    fn rodar(fonte: &str) -> Vec<(String, String, String)> {
        let mut interner = Interner::new();
        let p = dartforge_frontend::parser::parse(fonte, &mut interner);
        assert!(p.diagnostics.is_empty(), "{:?}", p.diagnostics);
        let u = Unidade {
            ast: &p.ast,
            unit: &p.unit,
            fonte,
        };
        let mut v: Vec<_> = nao_usados(&[u], &interner, false)
            .into_iter()
            .map(|(_, d)| {
                (
                    d.code.unwrap().info().nome.to_string(),
                    fonte[d.span.start..d.span.end].to_string(),
                    d.message,
                )
            })
            .collect();
        v.sort();
        v
    }

    /// Conferido com o `dart analyze` 3.6.2: tudo o que ele relata neste
    /// arquivo, exceto o operador da extensão privada (usado sem o nome;
    /// aqui fica de fora, um falso negativo).
    #[test]
    fn igual_ao_oraculo_menos_operador() {
        let fonte = "class _A {\n  static _A criar() => _A();\n}\n\nclass _Usada {}\n\nclass C {\n  int _lido = 0;\n  \
            int _escrito = 0;\n  int _formal;\n  int _inicializado;\n  static int _estatico = 1;\n  \
            C(this._formal) : _inicializado = 2;\n  C._um();\n  C._outro();\n  void _metodo() {}\n  void _chamado() {}\n  \
            int get _getter => 1;\n  set _setter(int v) {}\n  void f() {\n    _escrito = 3;\n    print(_lido);\n    \
            _chamado();\n    print(_Usada);\n    print(C._um);\n  }\n}\n\nenum _E { a, b }\n\nenum E2 { x, _y }\n\n\
            typedef _F = int Function();\n\nint _topo = 0;\nint _topoLido = 0;\n\nvoid _funcao() {}\n\n\
            void main() {\n  _topo = 1;\n  print(_topoLido);\n}\n\nextension _X on int {\n  int operator -() => 0;\n  \
            int get dobro => 2;\n}\n\nmixin class _M {}\n\nclass _Alias = Object with _M;\n\n\
            /// [_comentado] só no comentário.\nint _comentado = 0;\n";
        let v = rodar(fonte);
        let elemento = |n: &str| ("unused_element".to_string(), n.to_string());
        let campo = |n: &str| ("unused_field".to_string(), n.to_string());
        let mut esperado = vec![
            elemento("_A"),
            elemento("criar"),
            campo("_escrito"),
            campo("_formal"),
            campo("_inicializado"),
            campo("_estatico"),
            elemento("_outro"),
            elemento("_metodo"),
            elemento("_getter"),
            elemento("_setter"),
            elemento("_E"),
            campo("a"),
            campo("b"),
            campo("_y"),
            elemento("_F"),
            elemento("_topo"),
            elemento("_funcao"),
            elemento("dobro"),
            elemento("_comentado"),
        ];
        esperado.sort();
        let obtido: Vec<(String, String)> =
            v.iter().map(|(a, b, _)| (a.clone(), b.clone())).collect();
        assert_eq!(obtido, esperado);
        let outro = v.iter().find(|x| x.1 == "_outro").unwrap();
        assert_eq!(outro.2, "The declaration 'C._outro' isn't referenced.");
        let e = v.iter().find(|x| x.1 == "_escrito").unwrap();
        assert_eq!(e.2, "The value of the field '_escrito' isn't used.");
    }

    /// `values` lê todas as constantes; um `typedef` público usa os
    /// construtores públicos do tipo; construtor único não se relata.
    #[test]
    fn values_typedef_publico_e_construtor_unico() {
        let fonte = "enum _E { a, b }\nvoid f() { print(_E.values); }\n\
            class _C { _C.um(); _C.dois(); }\ntypedef P = _C;\n\
            class D { D._so(); }\n";
        assert_eq!(rodar(fonte), vec![]);
    }

    /// Com `@pragma` a declaração conta como usada.
    #[test]
    fn pragma_torna_usado() {
        assert_eq!(rodar("@pragma('vm:entry-point')\nvoid _f() {}\n"), vec![]);
    }
}

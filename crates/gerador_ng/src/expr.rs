//! Conversão das expressões do template para Dart.
//!
//! O `ngcompiler` tem um parser de expressões só dele (1.620 linhas em
//! `compiler/expression_parser`). Nós não precisamos: expressão de template é
//! expressão Dart, e o parser da trilha nova já a analisa. O trabalho aqui é
//! só reescrever a árvore com o receptor certo — `nome` vira `_ctx.nome` — e
//! nos parênteses que o emissor oficial põe.
//!
//! O que o `expression_converter.dart` escreve e este módulo reproduz:
//! `_ctx.item.nome`, `_ctx.titulo()`, `(!_ctx.item.ativo)`, `'fixo'`.
//!
//! Três propriedades da expressão decidem a forma do código gerado, e cada
//! uma segue a regra do oficial (`analyzed_class.dart`):
//! - `isImmutable`: só literal, `a ?? b` e binário de imutáveis, e o campo
//!   `final`/`const` lido com receptor implícito. Getter explícito é
//!   **mutável** (a `variable` dele é sintética); `a.b`, `!x`, chamada e
//!   ternário também.
//! - `canBeNull`: só o literal não é nulo (e `a ?? b` quando um lado não é).
//! - o tipo (`_TypeResolver`): do membro ou do getter; o resto é `dynamic`.
//!
//! Os handlers de evento (`parseAction`) são a mesma linguagem com duas
//! formas a mais: `$event` e a atribuição (`x = y`, `a.b = y`).
use crate::componente::Membro;
use crate::resolucao::Resolucao;
use crate::visao::{Motivo, Recusa, recusa};
use dartforge_frontend::ast;
use dartforge_intern::Interner;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Início e fim da marca de uma chamada de pipe no texto convertido:
/// `\u{3}nome/argumentos\u{4}`. Quem converte não sabe a visão nem a ordem
/// das chamadas; a visão troca a marca pelo proxy.
pub const MARCA_DE_PIPE: char = '\u{3}';
pub const FIM_DE_PIPE: char = '\u{4}';

/// Um local de visão embutida: o nome em Dart e o tipo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Local {
    /// Como sai no código gerado: `local_item`.
    pub dart: String,
    /// Tipo estático, para a escolha da forma de interpolação.
    pub tipo: String,
    /// Arquivo em cujo escopo o texto do tipo foi escrito (o tipo de um
    /// membro de outra classe é escrito na biblioteca dela). `None`: o do
    /// próprio componente.
    pub escopo: Option<PathBuf>,
}

/// Uma expressão de template já convertida.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Convertida {
    /// O Dart que sai no arquivo gerado.
    pub texto: String,
    /// `isImmutable` do ngcompiler: valor que não muda dispensa
    /// `checkBinding` e é escrito uma vez, na primeira checagem.
    pub imutavel: bool,
    /// `canBeNull` do ngcompiler: decide o `if (x != null)` em volta da
    /// ligação constante e o `setAttribute` × `updateAttribute`.
    pub pode_ser_nulo: bool,
    /// Tipo estático, quando dá para saber sem sair da classe do componente.
    /// É o que escolhe entre `interpolateString`, `interpolate` e
    /// `updateTextWithPrimitive`. `None`: não se sabe.
    pub tipo: Option<String>,
    /// Arquivo em cujo escopo `tipo` foi escrito, quando não é o do
    /// componente.
    pub escopo: Option<PathBuf>,
    /// A forma da expressão (`chamada`, `ternário`…), para o placar dizer
    /// o que falta quando ela é recusada no contexto.
    pub forma: &'static str,
    /// É um literal primitivo (`'x'`, `1`, `true`, `null`): o valor é o
    /// próprio texto.
    pub literal: bool,
    /// Os locais da visão que a expressão lê, na ordem em que o conversor
    /// oficial os pede ao `ViewNameResolver` — é a ordem em que as
    /// declarações `final local_x = …` saem no método.
    pub locais: Vec<String>,
    /// Uma classe de `exports:` (o nome simples): só vale como receptor de
    /// membro estático (`Classe.nome`).
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

/// Quebra de linha do `DartFormatter` (dart_style 2.3.8, página de
/// 1.000.000 colunas) que o builder do ngdart roda sobre a saída: com a
/// página infinita, só a vírgula final de uma lista de argumentos a quebra,
/// e o emissor escreve vírgula depois de cada argumento nomeado
/// (`visitAllNamedExpressions`). A marca é [`QUEBRA`] seguida de um
/// [`RECUO`] por espaço além da indentação da linha onde a expressão está,
/// que só se conhece quando o arquivo está montado
/// ([`resolver_quebras`]).
pub const QUEBRA: char = '\u{10}';
pub const RECUO: char = '\u{11}';

/// Soma `n` espaços a cada [`QUEBRA`] de `t`.
pub fn recuar(t: &str, n: usize) -> String {
    let mais: String = std::iter::repeat_n(RECUO, n).collect();
    t.replace(QUEBRA, &format!("{QUEBRA}{mais}"))
}

/// Troca cada [`QUEBRA`] pela quebra de linha com a indentação da linha
/// em que está mais os [`RECUO`]s dela.
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

/// Junta listas de locais mantendo a primeira ocorrência de cada um.
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

/// O que as expressões de um componente enxergam: membros, métodos, os
/// locais da visão e o banco semântico.
#[derive(Clone, Copy)]
pub struct Escopo<'a> {
    pub membros: &'a HashMap<String, Membro>,
    pub metodos: &'a HashMap<String, String>,
    /// Parâmetros posicionais de cada método (`rewriteTearOff`).
    pub aridades: &'a HashMap<String, usize>,
    pub locais: &'a HashMap<String, Local>,
    pub tipos: Option<(&'a dyn Resolucao, &'a Path)>,
    /// A classe do componente como o arquivo gerado a escreve
    /// (`import1.Classe`), para os membros estáticos; `None` quando não se
    /// sabe, e o estático é recusado.
    pub classe: Option<&'a str>,
    /// Os nomes de `exports:` (`_matchExport`), com o qualificador tardio.
    pub exportados: Option<&'a crate::visao::Exportados>,
}

/// Converte uma expressão escrita no template.
///
/// `membros` são os campos e getters do componente: um nome que não está lá
/// não é do `_ctx` e a expressão é recusada, porque traduzi-la errado daria
/// um arquivo que compila e faz outra coisa.
pub fn converter(
    expressao: &str,
    membros: &HashMap<String, Membro>,
    interner: &mut Interner,
) -> Result<Convertida, Recusa> {
    converter_com_metodos(expressao, membros, &HashMap::new(), interner, None)
}

/// Como [`converter`], sabendo também os métodos da classe — que só valem
/// como alvo de chamada.
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

/// Como [`converter_com_metodos`], com os locais de uma visão embutida
/// (`let item of itens` põe `item` no escopo). Um local vence o membro do
/// componente, como manda o escopo do template.
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
    };
    converter_no_escopo(expressao, &escopo, interner)
}

/// Converte uma expressão de ligação (`parseBinding`/`parseInterpolation`).
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

/// Um handler de evento convertido: `parseAction` seguido de
/// `handlerTypeFromExpression` (`parse_utils.dart`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Acao {
    /// `m()` ou `m($event)` com receptor implícito (ou o tear-off `m`
    /// reescrito para uma dessas por `rewriteTearOff`): sai como tear-off,
    /// `this.eventHandlerN(_ctx.m)`.
    Simples {
        metodo: String,
        aridade: u8,
        /// A mesma chamada como instrução (`_ctx.m($event)`), para quando
        /// este handler é fundido com outro do mesmo evento.
        instrucao: String,
    },
    /// Qualquer outra forma: uma instrução do método `_handleEvent_N`.
    Complexa(Convertida),
}

/// Converte o texto de um `(evento)="..."`.
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

/// Analisa o texto como expressão Dart: um `var` de topo é o menor contexto
/// em que o parser aceita uma expressão qualquer.
fn analisar(
    expressao: &str,
    interner: &mut Interner,
) -> Result<(dartforge_frontend::parser::Parsed, String, ast::ExprId), Recusa> {
    let fonte = format!("var _e = {expressao};");
    let analisada = dartforge_frontend::parser::parse(&fonte, interner);
    let invalida = || recusa(Motivo::Expressao, "expressão que o parser Dart não aceita");
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
    /// Handler de evento: `$event` e atribuição valem.
    acao: bool,
}

fn fora(forma: &'static str) -> Recusa {
    recusa(Motivo::Expressao, forma)
}

impl Conversor<'_> {
    /// Um campo ou getter de instância que a classe do componente herda
    /// (`lookUpGetter` do analyzer sobe a hierarquia): o tipo e o escopo
    /// vêm de quem o declara, e a imutabilidade é a do campo de lá. Sem
    /// tipo escrito, com tipo que cita parâmetro de tipo ou privado de outra
    /// biblioteca: `None`, e o nome é recusado.
    fn membro_herdado(&self, nome: &str) -> Option<Convertida> {
        if nome.starts_with('_') {
            return None;
        }
        let (r, arquivo) = self.escopo.tipos?;
        let classe = self.escopo.classe?.rsplit('.').next()?;
        let (tipo, escopo) = r.tipo_do_membro(arquivo, classe, nome)?;
        let imutavel = r.membro_final(arquivo, classe, nome)?;
        Some(Convertida {
            imutavel,
            tipo: Some(tipo),
            escopo: Some(escopo),
            ..Convertida::nova(format!("_ctx.{nome}"), "membro herdado")
        })
    }

    /// Um método de instância herdado ([`Resolucao::metodo`]), procurado só
    /// quando o nome não é da própria classe nem privado.
    fn metodo_herdado(&self, nome: &str) -> Option<crate::resolucao::Metodo> {
        if nome.starts_with('_') {
            return None;
        }
        let (r, arquivo) = self.escopo.tipos?;
        let classe = self.escopo.classe?.rsplit('.').next()?;
        r.metodo(arquivo, classe, nome)
    }

    /// O handler inteiro: classifica (simples × complexo) e converte.
    fn acao(&self, id: ast::ExprId) -> Result<Acao, Recusa> {
        let id = self.sem_parenteses(id);
        let e = self.ast.expr(id);
        // Tear-off (`(click)="m"`): `rewriteTearOff` troca por `m()` ou
        // `m($event)`, pelos parâmetros posicionais do método.
        if let ast::ExprKind::Identifier(n) = &e.kind {
            let nome = self.interner.resolve(n.sym);
            if !self.escopo.locais.contains_key(nome) && nome != "$event" {
                if let Some(&posicionais) = self.escopo.aridades.get(nome) {
                    let aridade = u8::from(posicionais > 0);
                    return self.simples(nome, aridade);
                }
                if self.escopo.metodos.contains_key(nome) {
                    return Err(fora("tear-off de método sem aridade conhecida"));
                }
                if !self.escopo.membros.contains_key(nome)
                    && let Some(m) = self.metodo_herdado(nome)
                {
                    return self.simples(nome, u8::from(m.posicionais > 0));
                }
            }
        }
        // `handlerTypeFromExpression`: `m()` ou `m($event)`, receptor
        // implícito, sem argumento nomeado.
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
                // Local com o nome do método: o oficial chama a função do
                // local (`InvokeFunctionExpr`) e recusa o tear-off.
                if self.escopo.locais.contains_key(nome) {
                    return Err(fora("handler que chama um local"));
                }
                return self.simples(nome, aridade);
            }
        }
        let c = match &e.kind {
            // Atribuição no topo da instrução sai sem parênteses
            // (`lineWasEmpty` em `visitWritePropExpr`).
            ast::ExprKind::Assign { .. } => self.atribuicao(id)?,
            _ => self.expr(id, true)?,
        };
        Ok(Acao::Complexa(c))
    }

    /// Um handler simples: o método tem de ser do componente (ou um campo,
    /// que o tear-off lê do mesmo jeito).
    fn simples(&self, nome: &str, aridade: u8) -> Result<Acao, Recusa> {
        if !self.escopo.metodos.contains_key(nome)
            && !self.escopo.membros.contains_key(nome)
            && self.metodo_herdado(nome).is_none()
            && self.membro_herdado(nome).is_none()
        {
            return Err(fora("handler que não é membro do componente"));
        }
        if self.escopo.membros.get(nome).is_some_and(|m| m.estatico) {
            return Err(fora("handler em membro estático"));
        }
        let arg = if aridade == 1 { "$event" } else { "" };
        Ok(Acao::Simples {
            metodo: nome.to_string(),
            aridade,
            instrucao: format!("_ctx.{nome}({arg})"),
        })
    }

    /// `$pipe.nome(entrada, args)`: o `BindingPipe` do parser oficial
    /// (`_createPipeOrThrow`). A chamada sai como `MARCA(entrada, args)`, e
    /// quem conhece a visão troca a [`MARCA_DE_PIPE`] pelo `pureProxyN`
    /// daquele ponto (`CompilePipe._call`). Tipo `dynamic` (`visitPipe` do
    /// `_TypeResolver`), mutável e pode ser nulo.
    fn pipe(&self, nome: &str, arguments: &ast::Arguments) -> Result<Convertida, Recusa> {
        if self.acao {
            return Err(recusa(Motivo::PipesUsados, "pipe em evento"));
        }
        if arguments.args.iter().any(|a| a.name.is_some()) {
            return Err(recusa(Motivo::PipesUsados, "pipe com argumento nomeado"));
        }
        if arguments.args.is_empty() {
            return Err(recusa(Motivo::PipesUsados, "pipe sem argumento"));
        }
        // `visitPipe` converte a entrada e depois os argumentos: é a ordem
        // em que os locais são pedidos.
        let mut args = Vec::new();
        let mut locais = Vec::new();
        // O pipe de dentro ganha o proxy primeiro (é convertido antes), mas
        // a marca de fora vem antes no texto: quem troca as marcas as ordena
        // pelo fim da chamada.
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

    /// `x = y`, `a.b = y`, `a[i] = y` (`PropertyWrite`/`KeyedWrite`), sem os
    /// parênteses — quem está dentro de outra expressão os põe.
    fn atribuicao(&self, id: ast::ExprId) -> Result<Convertida, Recusa> {
        let ast::ExprKind::Assign { op, target, value } = &self.ast.expr(id).kind else {
            return Err(fora("atribuição"));
        };
        if !self.acao {
            return Err(fora("atribuição fora de evento"));
        }
        // O oficial ignora o operador de uma atribuição composta e escreve
        // `x = y`: forma que não se reproduz de propósito.
        if *op != ast::AssignOp::Assign {
            return Err(fora("atribuição composta (`+=`)"));
        }
        let (alvo, locais_alvo) = match &self.ast.expr(*target).kind {
            ast::ExprKind::Identifier(n) => {
                let nome = self.interner.resolve(n.sym);
                if self.escopo.locais.contains_key(nome) || nome == "$event" {
                    return Err(fora("atribuição a local"));
                }
                match self.escopo.membros.get(nome) {
                    None => return Err(fora("atribuição a nome fora do componente")),
                    Some(m) if m.estatico => match self.escopo.classe {
                        Some(classe) => (format!("{classe}.{nome}"), Vec::new()),
                        None => return Err(fora("membro estático sem a classe qualificada")),
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
            ast::ExprKind::Index { .. } => return Err(fora("atribuição a índice `a[i] = x`")),
            _ => return Err(fora("atribuição a alvo desconhecido")),
        };
        let v = self.expr(*value, true)?;
        Ok(Convertida {
            locais: juntar(&[&locais_alvo, &v.locais]),
            ..Convertida::nova(format!("{alvo} = {}", v.texto), "atribuição")
        })
    }

    /// `raiz` diz se este nó é o receptor implícito — só aí um identificador
    /// vira `_ctx.nome`; em `a.b`, o `b` é membro de `a`.
    fn expr(&self, id: ast::ExprId, raiz: bool) -> Result<Convertida, Recusa> {
        let v = self.expr_do_no(id, raiz)?;
        // A chamada que quebra ([`QUEBRA`]) só como a expressão inteira ou
        // argumento de outra chamada: dentro de operador, acesso ou
        // condicional o formatador a indenta de outro jeito, sem caso.
        if v.texto.contains(QUEBRA) && !matches!(self.ast.expr(id).kind, ast::ExprKind::Call { .. })
        {
            return Err(fora("chamada com argumento nomeado dentro de expressão"));
        }
        Ok(v)
    }

    fn expr_do_no(&self, id: ast::ExprId, raiz: bool) -> Result<Convertida, Recusa> {
        let e = self.ast.expr(id);
        match &e.kind {
            // `LiteralPrimitive`: imutável e nunca nulo (`canBeNull`). O tipo
            // é `String` para texto e `dynamic` para o resto
            // (`visitLiteralPrimitive` do `_TypeResolver`). O texto é o que
            // `o.literal(valor)` escreve, não o que o template escreveu.
            ast::ExprKind::Int(_) => {
                let t = self.texto(id);
                let Ok(v) = t.parse::<u64>() else {
                    return Err(fora("literal inteiro fora da forma decimal"));
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
                // `double.toString()` do Dart e o `{:?}` do Rust concordam
                // na forma curta (`1.5`, `2.0`); expoente, não.
                match t.parse::<f64>() {
                    Ok(v) if format!("{v:?}") == t && !t.contains(['e', 'E']) => Ok(Convertida {
                        imutavel: true,
                        pode_ser_nulo: false,
                        tipo: Some("dynamic".into()),
                        literal: true,
                        ..Convertida::nova(t, "literal")
                    }),
                    _ => Err(fora("literal double fora da forma canônica")),
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
                // Só literal sem interpolação: `'a$b'` dentro do template é
                // outra coisa e não aparece nos projetos do proprietário.
                let Some(valor) = lit.constant_value() else {
                    return Err(fora("texto com interpolação Dart"));
                };
                Ok(Convertida {
                    imutavel: true,
                    pode_ser_nulo: false,
                    tipo: Some("String".into()),
                    literal: true,
                    ..Convertida::nova(crate::visao::literal(&valor.to_string_lossy()), "literal")
                })
            }
            ast::ExprKind::Identifier(n) => {
                let nome = self.interner.resolve(n.sym);
                if !raiz {
                    return Ok(Convertida::nova(nome.to_string(), "nome"));
                }
                // `getLocal('$event')` devolve a variável do handler.
                if nome == "$event" {
                    if !self.acao {
                        return Err(fora("`$event` fora de evento"));
                    }
                    return Ok(Convertida {
                        tipo: Some("dynamic".into()),
                        ..Convertida::nova("$event".into(), "`$event`")
                    });
                }
                // O local do laço sombreia o membro do componente.
                if let Some(l) = self.escopo.locais.get(nome) {
                    return Ok(Convertida {
                        tipo: Some(l.tipo.clone()),
                        escopo: l.escopo.clone(),
                        locais: vec![nome.to_string()],
                        ..Convertida::nova(l.dart.clone(), "local")
                    });
                }
                // Nome de `exports:` (`_matchExport`, antes dos membros): pelo
                // import da biblioteca que o declara, tipo `dynamic`; a
                // variável `const`/`final` é imutável (caso j51).
                if let Some((q, o_que)) = self.escopo.exportados.and_then(|e| e.get(nome)) {
                    use crate::resolucao::Exportado;
                    let texto = format!("{q}{nome}");
                    return match o_que {
                        Exportado::Classe => Ok(Convertida {
                            tipo: Some("dynamic".into()),
                            estatica: Some(nome.to_string()),
                            ..Convertida::nova(texto, "classe de exports:")
                        }),
                        Exportado::Variavel { imutavel } => Ok(Convertida {
                            imutavel: *imutavel,
                            tipo: Some("dynamic".into()),
                            ..Convertida::nova(texto, "variável de exports:")
                        }),
                        Exportado::Getter => Ok(Convertida {
                            tipo: Some("dynamic".into()),
                            ..Convertida::nova(texto, "getter de exports:")
                        }),
                        Exportado::Funcao => Err(fora("função de exports: como valor")),
                    };
                }
                // Método lido como valor (o `trackBy: rastrear` do `*ngFor`,
                // um callback passado a um filho): `isImmutable` diz que
                // "methods are immutable"; `canBeNull` não o isenta; e o
                // `_TypeResolver` procura um *getter* com o nome — não há,
                // então é `dynamic`.
                if !self.escopo.membros.contains_key(nome) && self.escopo.metodos.contains_key(nome)
                {
                    return Ok(Convertida {
                        imutavel: true,
                        tipo: Some("dynamic".into()),
                        ..Convertida::nova(format!("_ctx.{nome}"), "método como valor")
                    });
                }
                let Some(m) = self.escopo.membros.get(nome) else {
                    return self
                        .membro_herdado(nome)
                        .ok_or_else(|| fora("nome fora do componente"));
                };
                // Estático: `importExpr` da classe e o nome; `isImmutable`
                // pelo campo (`final`/`const`), tipo `dynamic` e, fora do
                // literal, pode ser nulo (caso j25).
                if m.estatico {
                    let Some(classe) = self.escopo.classe else {
                        return Err(fora("membro estático sem a classe qualificada"));
                    };
                    return Ok(Convertida {
                        imutavel: m.imutavel,
                        tipo: Some("dynamic".into()),
                        ..Convertida::nova(format!("{classe}.{nome}"), "membro estático")
                    });
                }
                // `isImmutable` de `PropertyRead` com receptor implícito:
                // campo `final`/`const` (o getter já chega aqui mutável).
                Ok(Convertida {
                    imutavel: m.imutavel,
                    tipo: (!m.tipo.is_empty()).then(|| m.tipo.clone()),
                    ..Convertida::nova(format!("_ctx.{nome}"), "membro")
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
                // `Classe.nome` de uma classe de `exports:`: o campo
                // `const`/`final` (e o valor de enum) é imutável; getter e
                // método, não. Tipo `dynamic` (caso j51).
                if let Some(classe) = &alvo.estatica {
                    use crate::resolucao::Estatico;
                    let Some((r, arquivo)) = self.escopo.tipos else {
                        return Err(fora("membro estático de exports: sem o banco semântico"));
                    };
                    let imutavel = match r.membro_estatico(arquivo, classe, nome) {
                        Some(Estatico::Campo { imutavel }) => imutavel,
                        Some(Estatico::Getter | Estatico::Metodo) => false,
                        None => return Err(fora("membro estático de exports: desconhecido")),
                    };
                    return Ok(Convertida {
                        imutavel,
                        tipo: Some("dynamic".into()),
                        locais: alvo.locais,
                        ..Convertida::nova(
                            format!("{}{ponto}{nome}", alvo.texto),
                            "membro estático",
                        )
                    });
                }
                // O tipo do fim da cadeia está noutra classe: é o banco
                // semântico que responde, como o analyzer responde ao oficial.
                // Receptor `dynamic` (o `#ref`, `$event`): o
                // `_lookupGetterReturnType` só olha `InterfaceType`, e o
                // resto dá `dynamic`.
                let achado = match (&alvo.tipo, self.escopo.tipos) {
                    (Some(t), Some((r, arquivo))) if t != "dynamic" => {
                        r.tipo_do_membro(alvo.escopo.as_deref().unwrap_or(arquivo), t, nome)
                    }
                    _ => None,
                };
                // Receptor de tipo que não existe no escopo (`InvalidType`:
                // a classe de um arquivo que a fase do ngdart ainda não vê)
                // também dá `dynamic`.
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
                // O receptor não é implícito: `a.b` é sempre mutável.
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
                    return Err(fora("chamada com argumento de tipo"));
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
                // `visitMethodCall` converte os argumentos **antes** do
                // receptor: é a ordem em que os locais são pedidos.
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
                // Com argumento nomeado a lista termina em vírgula e o
                // formatador põe um argumento por linha, dois espaços para
                // dentro (e o que já quebrava dentro deles, junto). Numa
                // lista sem nomeado, um argumento que quebra ganharia a
                // indentação de continuação — forma sem caso no corpus.
                let nomeado = arguments.args.iter().any(|a| a.name.is_some());
                if !nomeado && args.iter().any(|a| a.contains(QUEBRA)) {
                    return Err(fora("chamada com argumento nomeado dentro de argumento"));
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
                // O alvo de uma chamada pode ser um método da classe, que não
                // vale como valor solto. O tipo é o do `_TypeResolver`: o
                // retorno do método (`visitMethodCall`); campo ou local
                // chamado como função não é método, e dá `dynamic`.
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
                            // Função de `exports:`: pelo import, `dynamic`.
                            _ if exportada.is_some() => (
                                format!(
                                    "{}{nome}",
                                    exportada.map(|(q, _)| q.as_str()).unwrap_or_default()
                                ),
                                Some("dynamic".to_string()),
                                Vec::new(),
                            ),
                            Some(t) => (format!("_ctx.{nome}"), Some(t.clone()), Vec::new()),
                            // Método herdado: o retorno no escopo de quem o
                            // declara; sem tipo escrito, não se sabe.
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
                    // Local chamado como função: o tipo ainda é procurado
                    // como método da classe (`visitMethodCall` olha o nome).
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
                    // `a.m()`: o banco semântico responde o membro `m` do tipo
                    // de `a` — o retorno, quando é método.
                    _ => {
                        let t = self.expr(*target, raiz)?;
                        (t.texto, t.tipo, t.locais)
                    }
                };
                Ok(Convertida {
                    tipo: retorno,
                    escopo: escopo_do_retorno,
                    locais: juntar(&[&locais_args, &locais_alvo]),
                    ..Convertida::nova(format!("{alvo}({lista})"), "chamada")
                })
            }
            ast::ExprKind::Unary { op, operand } => {
                // Só `!`. O parser de expressões do ngcompiler lê `-x` como
                // `0 - x` e sai `(0 - _ctx.x)`; `x!` sai como `(x!)`; `~` nem
                // existe lá. Os dois primeiros ainda não têm caso no corpus.
                // `x!` é `PostfixNotNull`: `.notNull()`, que o emissor escreve
                // `(x!)`; tipo `dynamic`, mutável.
                if *op == ast::UnaryOp::NullAssert {
                    let v = self.expr(*operand, raiz)?;
                    return Ok(Convertida {
                        tipo: Some("dynamic".into()),
                        locais: v.locais,
                        ..Convertida::nova(format!("({}!)", v.texto), "`x!`")
                    });
                }
                if *op != ast::UnaryOp::Not {
                    return Err(fora(match op {
                        ast::UnaryOp::Neg => "`-x`",
                        _ => "operador unário fora do template",
                    }));
                }
                let v = self.expr(*operand, raiz)?;
                let op = self.operador_unario(id);
                // `PrefixNot` não entra no `isImmutable`: é mutável.
                Ok(Convertida {
                    tipo: Some("dynamic".into()),
                    locais: v.locais,
                    ..Convertida::nova(format!("({op}{})", v.texto), "`!`")
                })
            }
            ast::ExprKind::Binary { op, .. } if !operador_do_template(*op) => {
                // `a | b` no template é pipe, não OU bit a bit: traduzir
                // como Dart daria `(_ctx.a | _ctx.b)`, que compila e faz
                // outra coisa. Os demais (`&`, `^`, `<<`, `~/`…) não existem
                // na linguagem de expressões do ngdart.
                Err(if *op == ast::BinaryOp::BitOr {
                    recusa(Motivo::PipesUsados, "pipe `x | nome`")
                } else {
                    fora("operador binário fora do template")
                })
            }
            ast::ExprKind::Binary { op, left, right } => {
                let a = self.expr(*left, raiz)?;
                let b = self.expr(*right, raiz)?;
                let texto_op = self.operador_binario(id);
                let se_nulo = *op == ast::BinaryOp::IfNull;
                // `canBeNull(IfNull)`: não nulo se o da esquerda não for;
                // senão, o da direita decide. Binário comum é sempre
                // "pode ser nulo" para o oficial.
                let pode_ser_nulo = if se_nulo {
                    a.pode_ser_nulo && b.pode_ser_nulo
                } else {
                    true
                };
                // `visitBinary`: `String + String` é `String`; o resto,
                // `dynamic`. `??` é `dynamic`.
                let tipo = if *op == ast::BinaryOp::Add
                    && a.tipo.as_deref() == Some("String")
                    && b.tipo.as_deref() == Some("String")
                {
                    "String"
                } else {
                    "dynamic"
                };
                Ok(Convertida {
                    // `isImmutable` de `Binary` e `IfNull`: os dois lados.
                    imutavel: a.imutavel && b.imutavel,
                    pode_ser_nulo,
                    tipo: Some(tipo.into()),
                    locais: juntar(&[&a.locais, &b.locais]),
                    ..Convertida::nova(
                        format!("({} {texto_op} {})", a.texto, b.texto),
                        if se_nulo { "`??`" } else { "binário" },
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
                        "ternário",
                    )
                })
            }
            ast::ExprKind::Parenthesized(inner) => self.expr(*inner, raiz),
            // Dentro de outra expressão a atribuição ganha parênteses.
            ast::ExprKind::Assign { .. } => {
                let a = self.atribuicao(id)?;
                Ok(Convertida {
                    texto: format!("({})", a.texto),
                    ..a
                })
            }
            // `KeyedRead`: `receiver.key(key)`, sem parênteses; `dynamic`.
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
                    ..Convertida::nova(format!("{}[{}]", r.texto, k.texto), "índice")
                })
            }
            ast::ExprKind::Index { .. } => Err(fora("índice `a?[i]`")),
            ast::ExprKind::List { .. } | ast::ExprKind::SetOrMap { .. } => {
                Err(fora("literal de coleção"))
            }
            ast::ExprKind::FunctionExpression(_) => Err(fora("função anônima")),
            ast::ExprKind::Is { .. } | ast::ExprKind::As { .. } => Err(fora("`is`/`as`")),
            _ => Err(fora("forma de expressão fora do template")),
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

    /// O operador binário como escrito, extraído entre os dois operandos.
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

/// Operadores binários da linguagem de expressões do ngdart
/// (`expression_parser/parser.dart`): aritméticos, comparação, lógicos e
/// `??`. O que o Dart tem a mais não passa.
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

    /// As formas que o caso c05 do corpus mostrou saindo do oficial.
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

    /// `isImmutable` do oficial: `a.b`, `!x` e ternário são mutáveis mesmo
    /// com tudo `final`; `a ?? b` e binário de imutáveis, não.
    #[test]
    fn imutabilidade_como_no_oficial() {
        assert!(!conv("titulo.length").imutavel);
        assert!(!conv("!fixo").imutavel);
        assert!(!conv("fixo > 1 ? 1 : 2").imutavel);
        assert!(conv("fixo ?? 1").imutavel);
        assert!(conv("fixo + 1").imutavel);
    }

    /// `canBeNull`: só o literal escapa, e `a ?? b` com um lado não nulo.
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

    /// `|` no template é pipe: recusado em qualquer profundidade, e o `||`
    /// continua sendo OU lógico.
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

    /// `$pipe.nome(entrada, args)` vira a marca com o nome e o número de
    /// argumentos, tipo `dynamic`, também dentro de outro pipe; com
    /// argumento nomeado, recusa.
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

    /// O que o parser do ngdart lê diferente do Dart fica de fora: `-x` é
    /// `0 - x` lá, `&` não existe. `x!` sai `(x!)` e o índice sem
    /// parênteses.
    #[test]
    fn operadores_fora_do_template_sao_recusados() {
        let m = &membros();
        let mut i = Interner::new();
        assert!(converter("-fixo", m, &mut i).is_err());
        assert!(converter("fixo & 1", m, &mut i).is_err());
        assert_eq!(conv("nome!").texto, "(_ctx.nome!)");
        assert_eq!(conv("item[fixo]").texto, "_ctx.item[_ctx.fixo]");
    }

    /// Nome que não é do componente não vira `_ctx.nome` por engano.
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

    /// Complexos: atribuição no topo sem parênteses, argumentos, locais na
    /// ordem em que o oficial os pede (argumentos antes do receptor).
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

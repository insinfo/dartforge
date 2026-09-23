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
use crate::componente::Membro;
use crate::resolucao::Resolucao;
use crate::visao::{Motivo, Recusa, recusa};
use dartforge_frontend::ast;
use dartforge_intern::Interner;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

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
        }
    }
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
    // Um `var` de topo é o menor contexto em que o parser aceita uma
    // expressão qualquer.
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
    let c = Conversor {
        ast: &analisada.ast,
        fonte: &fonte,
        interner,
        membros,
        metodos,
        locais,
        tipos,
    };
    c.expr(inicial, true)
}

struct Conversor<'a> {
    ast: &'a ast::Ast,
    fonte: &'a str,
    interner: &'a Interner,
    membros: &'a HashMap<String, Membro>,
    metodos: &'a HashMap<String, String>,
    locais: &'a HashMap<String, Local>,
    /// Banco semântico e o arquivo em que a expressão foi escrita, para
    /// perguntar o tipo de um membro que está noutra classe.
    tipos: Option<(&'a dyn Resolucao, &'a Path)>,
}

fn fora(forma: &'static str) -> Recusa {
    recusa(Motivo::Expressao, forma)
}

impl Conversor<'_> {
    /// `raiz` diz se este nó é o receptor implícito — só aí um identificador
    /// vira `_ctx.nome`; em `a.b`, o `b` é membro de `a`.
    fn expr(&self, id: ast::ExprId, raiz: bool) -> Result<Convertida, Recusa> {
        let e = self.ast.expr(id);
        match &e.kind {
            // `LiteralPrimitive`: imutável e nunca nulo (`canBeNull`). O tipo
            // é `String` para texto e `dynamic` para o resto
            // (`visitLiteralPrimitive` do `_TypeResolver`).
            ast::ExprKind::Int(_) | ast::ExprKind::Double(_) | ast::ExprKind::Bool(_) => {
                Ok(Convertida {
                    imutavel: true,
                    pode_ser_nulo: false,
                    tipo: Some("dynamic".into()),
                    literal: true,
                    ..Convertida::nova(self.texto(id), "literal")
                })
            }
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
                if lit.constant_value().is_none() {
                    return Err(fora("texto com interpolação Dart"));
                }
                Ok(Convertida {
                    imutavel: true,
                    pode_ser_nulo: false,
                    tipo: Some("String".into()),
                    literal: true,
                    ..Convertida::nova(self.texto(id), "literal")
                })
            }
            ast::ExprKind::Identifier(n) => {
                let nome = self.interner.resolve(n.sym);
                if !raiz {
                    return Ok(Convertida::nova(nome.to_string(), "nome"));
                }
                // O local do laço sombreia o membro do componente.
                if let Some(l) = self.locais.get(nome) {
                    return Ok(Convertida {
                        tipo: Some(l.tipo.clone()),
                        escopo: l.escopo.clone(),
                        ..Convertida::nova(l.dart.clone(), "local")
                    });
                }
                let Some(m) = self.membros.get(nome) else {
                    return Err(fora(if self.metodos.contains_key(nome) {
                        "método como valor"
                    } else {
                        "nome fora do componente"
                    }));
                };
                // `isImmutable` de `PropertyRead` com receptor implícito:
                // campo `final`/`const` (o getter já chega aqui mutável).
                Ok(Convertida {
                    imutavel: m.imutavel,
                    tipo: Some(m.tipo.clone()),
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
                // O tipo do fim da cadeia está noutra classe: é o banco
                // semântico que responde, como o analyzer responde ao oficial.
                let achado = match (&alvo.tipo, self.tipos) {
                    (Some(t), Some((r, arquivo))) => {
                        r.tipo_do_membro(alvo.escopo.as_deref().unwrap_or(arquivo), t, nome)
                    }
                    _ => None,
                };
                let (tipo, escopo) = match achado {
                    Some((t, e)) => (Some(t), Some(e)),
                    None => (None, None),
                };
                // O receptor não é implícito: `a.b` é sempre mutável.
                Ok(Convertida {
                    tipo,
                    escopo,
                    ..Convertida::nova(
                        format!("{}{ponto}{nome}", alvo.texto),
                        if *null_aware { "`?.`" } else { "propriedade" },
                    )
                })
            }
            ast::ExprKind::Call { target, arguments } => {
                // O alvo de uma chamada pode ser um método da classe, que não
                // vale como valor solto.
                let (alvo, retorno) = match &self.ast.expr(*target).kind {
                    ast::ExprKind::Identifier(n)
                        if raiz && !self.locais.contains_key(self.interner.resolve(n.sym)) =>
                    {
                        let nome = self.interner.resolve(n.sym);
                        match self.metodos.get(nome) {
                            Some(t) => (format!("_ctx.{nome}"), Some(t.clone())),
                            None => (self.expr(*target, raiz)?.texto, None),
                        }
                    }
                    _ => (self.expr(*target, raiz)?.texto, None),
                };
                if !arguments.type_args.is_empty() {
                    return Err(fora("chamada com argumento de tipo"));
                }
                let mut args = Vec::new();
                for a in arguments.args.iter() {
                    let v = self.expr(a.value, true)?;
                    match &a.name {
                        Some(n) => {
                            args.push(format!("{}: {}", self.interner.resolve(n.sym), v.texto))
                        }
                        None => args.push(v.texto),
                    }
                }
                Ok(Convertida {
                    tipo: retorno,
                    ..Convertida::nova(format!("{alvo}({})", args.join(", ")), "chamada")
                })
            }
            ast::ExprKind::Unary { op, operand } => {
                // Só `!`. O parser de expressões do ngcompiler lê `-x` como
                // `0 - x` e sai `(0 - _ctx.x)`; `x!` sai sem parênteses; `~`
                // nem existe lá. Traduzir qualquer um deles daria um arquivo
                // diferente do oficial.
                if *op != ast::UnaryOp::Not {
                    return Err(fora(match op {
                        ast::UnaryOp::NullAssert => "`x!` pós-fixo",
                        ast::UnaryOp::Neg => "`-x`",
                        _ => "operador unário fora do template",
                    }));
                }
                let v = self.expr(*operand, raiz)?;
                let op = self.operador_unario(id);
                // `PrefixNot` não entra no `isImmutable`: é mutável.
                Ok(Convertida {
                    tipo: Some("dynamic".into()),
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
                    ..Convertida::nova(
                        format!("({} ? {} : {})", c.texto, t.texto, f.texto),
                        "ternário",
                    )
                })
            }
            ast::ExprKind::Parenthesized(inner) => self.expr(*inner, raiz),
            ast::ExprKind::Index { .. } => Err(fora("índice `a[i]`")),
            ast::ExprKind::Assign { .. } => Err(fora("atribuição")),
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
                },
            ),
            (
                "nome".to_string(),
                Membro {
                    tipo: "String".into(),
                    imutavel: false,
                },
            ),
            (
                "fixo".to_string(),
                Membro {
                    tipo: "int".into(),
                    imutavel: true,
                },
            ),
            (
                "titulo".to_string(),
                Membro {
                    tipo: "String".into(),
                    imutavel: true,
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

    /// O que o parser do ngdart lê diferente do Dart fica de fora: `-x` é
    /// `0 - x` lá, `x!` sai sem parênteses, `&` não existe.
    #[test]
    fn operadores_fora_do_template_sao_recusados() {
        let m = &membros();
        let mut i = Interner::new();
        assert!(converter("-fixo", m, &mut i).is_err());
        assert!(converter("nome!", m, &mut i).is_err());
        assert!(converter("fixo & 1", m, &mut i).is_err());
    }

    /// Nome que não é do componente não vira `_ctx.nome` por engano.
    #[test]
    fn nome_desconhecido_e_recusado() {
        assert!(converter("outro", &membros(), &mut Interner::new()).is_err());
    }
}

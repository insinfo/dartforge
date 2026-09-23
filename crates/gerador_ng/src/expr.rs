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
use crate::componente::Membro;
use crate::resolucao::Resolucao;
use crate::visao::Motivo;
use dartforge_frontend::ast;
use dartforge_intern::Interner;
use std::collections::HashMap;
use std::path::Path;

/// Um local de visão embutida: o nome em Dart e o tipo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Local {
    /// Como sai no código gerado: `local_item`.
    pub dart: String,
    /// Tipo estático, para a escolha da forma de interpolação.
    pub tipo: String,
}

/// Uma expressão de template já convertida.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Convertida {
    /// O Dart que sai no arquivo gerado.
    pub texto: String,
    /// `isImmutable` do ngcompiler: valor que não muda dispensa
    /// `checkBinding` e é escrito uma vez, na primeira checagem.
    pub imutavel: bool,
    /// Tipo estático, quando dá para saber sem sair da classe do componente.
    /// É o que escolhe entre `interpolateString`, `interpolate` e
    /// `updateTextWithPrimitive`.
    pub tipo: Option<String>,
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
) -> Result<Convertida, Motivo> {
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
) -> Result<Convertida, Motivo> {
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
) -> Result<Convertida, Motivo> {
    // Um `var` de topo é o menor contexto em que o parser aceita uma
    // expressão qualquer.
    let fonte = format!("var _e = {expressao};");
    let analisada = dartforge_frontend::parser::parse(&fonte, interner);
    if !analisada.diagnostics.is_empty() {
        return Err(Motivo::Ligacao);
    }
    let Some(&id) = analisada.unit.declarations.first() else {
        return Err(Motivo::Ligacao);
    };
    let ast::DeclKind::Variables(lista) = &analisada.ast.decl(id).kind else {
        return Err(Motivo::Ligacao);
    };
    let Some(v) = lista.variables.first() else {
        return Err(Motivo::Ligacao);
    };
    let Some(inicial) = v.initializer else {
        return Err(Motivo::Ligacao);
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

impl Conversor<'_> {
    /// `raiz` diz se este nó é o receptor implícito — só aí um identificador
    /// vira `_ctx.nome`; em `a.b`, o `b` é membro de `a`.
    fn expr(&self, id: ast::ExprId, raiz: bool) -> Result<Convertida, Motivo> {
        let e = self.ast.expr(id);
        match &e.kind {
            ast::ExprKind::Int(_) | ast::ExprKind::Double(_) => Ok(Convertida {
                texto: self.texto(id),
                imutavel: true,
                tipo: None,
            }),
            ast::ExprKind::Bool(_) => Ok(Convertida {
                texto: self.texto(id),
                imutavel: true,
                tipo: Some("bool".into()),
            }),
            ast::ExprKind::Null => Ok(Convertida {
                texto: "null".into(),
                imutavel: true,
                tipo: None,
            }),
            ast::ExprKind::String(lit) => {
                // Só literal sem interpolação: `'a$b'` dentro do template é
                // outra coisa e não aparece nos projetos do proprietário.
                if lit.constant_value().is_none() {
                    return Err(Motivo::Ligacao);
                }
                Ok(Convertida {
                    texto: self.texto(id),
                    imutavel: true,
                    tipo: Some("String".into()),
                })
            }
            ast::ExprKind::Identifier(n) => {
                let nome = self.interner.resolve(n.sym);
                if !raiz {
                    return Ok(Convertida {
                        texto: nome.to_string(),
                        imutavel: false,
                        tipo: None,
                    });
                }
                // O local do laço sombreia o membro do componente.
                if let Some(l) = self.locais.get(nome) {
                    return Ok(Convertida {
                        texto: l.dart.clone(),
                        imutavel: false,
                        tipo: Some(l.tipo.clone()),
                    });
                }
                let Some(m) = self.membros.get(nome) else {
                    return Err(Motivo::Ligacao);
                };
                Ok(Convertida {
                    texto: format!("_ctx.{nome}"),
                    imutavel: m.imutavel,
                    tipo: Some(m.tipo.clone()),
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
                    (Some(t), Some((r, arquivo))) => r.tipo_do_membro(arquivo, t, nome),
                    _ => None,
                };
                Ok(Convertida {
                    texto: format!("{}{ponto}{nome}", alvo.texto),
                    imutavel: achado.as_ref().map(|(_, i)| *i).unwrap_or(false),
                    tipo: achado.map(|(t, _)| t),
                })
            }
            ast::ExprKind::Call { target, arguments } => {
                // O alvo de uma chamada pode ser um método da classe, que não
                // vale como valor solto.
                let alvo = match &self.ast.expr(*target).kind {
                    ast::ExprKind::Identifier(n) if raiz => {
                        let nome = self.interner.resolve(n.sym);
                        match self.metodos.get(nome) {
                            Some(_) => Convertida {
                                texto: format!("_ctx.{nome}"),
                                imutavel: false,
                                tipo: None,
                            },
                            None => self.expr(*target, raiz)?,
                        }
                    }
                    _ => self.expr(*target, raiz)?,
                };
                if !arguments.type_args.is_empty() {
                    return Err(Motivo::Ligacao);
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
                    texto: format!("{}({})", alvo.texto, args.join(", ")),
                    imutavel: false,
                    tipo: None,
                })
            }
            ast::ExprKind::Unary { op, operand } => {
                // Só `!`. O parser de expressões do ngcompiler lê `-x` como
                // `0 - x` e sai `(0 - _ctx.x)`; `x!` sai sem parênteses; `~`
                // nem existe lá. Traduzir qualquer um deles daria um arquivo
                // diferente do oficial.
                if *op != ast::UnaryOp::Not {
                    return Err(Motivo::Ligacao);
                }
                let v = self.expr(*operand, raiz)?;
                let op = self.operador_unario(id);
                Ok(Convertida {
                    texto: format!("({op}{})", v.texto),
                    imutavel: v.imutavel,
                    tipo: None,
                })
            }
            ast::ExprKind::Binary { op, .. } if !operador_do_template(*op) => {
                // `a | b` no template é pipe, não OU bit a bit: traduzir
                // como Dart daria `(_ctx.a | _ctx.b)`, que compila e faz
                // outra coisa. Os demais (`&`, `^`, `<<`, `~/`…) não existem
                // na linguagem de expressões do ngdart.
                Err(if *op == ast::BinaryOp::BitOr {
                    Motivo::PipesUsados
                } else {
                    Motivo::Ligacao
                })
            }
            ast::ExprKind::Binary { left, right, .. } => {
                let a = self.expr(*left, raiz)?;
                let b = self.expr(*right, raiz)?;
                let op = self.operador_binario(id);
                Ok(Convertida {
                    texto: format!("({} {op} {})", a.texto, b.texto),
                    // `isImmutable` do ngcompiler para binário: imutável se os
                    // dois lados forem.
                    imutavel: a.imutavel && b.imutavel,
                    tipo: None,
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
                    texto: format!("({} ? {} : {})", c.texto, t.texto, f.texto),
                    imutavel: false,
                    tipo: None,
                })
            }
            ast::ExprKind::Parenthesized(inner) => self.expr(*inner, raiz),
            _ => Err(Motivo::Ligacao),
        }
    }

    fn texto(&self, id: ast::ExprId) -> String {
        let s = self.ast.expr(id).span;
        self.fonte
            .get(s.start as usize..s.end as usize)
            .unwrap_or("")
            .to_string()
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
        let fim_esq = self.ast.expr(*left).span.end as usize;
        let ini_dir = self.ast.expr(*right).span.start as usize;
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
        assert_eq!(conv("titulo()").texto, "_ctx.titulo()");
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
            converter("nome | fixo", m, &mut i),
            Err(Motivo::PipesUsados)
        );
        assert_eq!(
            converter("(nome | fixo) == 'x'", m, &mut i),
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

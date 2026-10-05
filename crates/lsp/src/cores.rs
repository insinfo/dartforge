//! `textDocument/documentColor` e `textDocument/colorPresentation`
//! (docs/LSP-ESPECIFICACAO.md §8.2): o `ColorComputer` do 3.6.2
//! (`AS:src/computer/computer_color.dart`) e o
//! `DocumentColorPresentationHandler`
//! (`handler_document_color_presentation.dart`).
//!
//! Uma expressão cujo tipo estático é o `Color` do `dart:ui` (ou um subtipo)
//! tem cor quando o valor constante dela (ou de um membro dele, ou de um
//! índice de paleta) dá os campos `a`, `r`, `g` e `b`, ou quando é uma
//! chamada conhecida (`Color(0x…)`, `Color.from`, `Color.fromARGB`,
//! `Color.fromRGBO`, `ColorSwatch(0x…)`, `MaterialAccentColor(0x…)`) com
//! literais. As regras do visitante: no índice (`Colors.red[500]`), no
//! identificador prefixado (`A.b`, inteiro, ou o membro `b` do valor de `A`)
//! e no acesso a propriedade (o membro do valor do alvo) uma cor achada
//! encerra a descida; numa criação, os argumentos continuam visitados.
//!
//! Escrito sem compilar nem executar (2026-10-05).

use crate::projeto::Projeto;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, LibraryId, UnitId};
use dartforge_frontend::ast::{self, ExprId, ExprKind};
use dartforge_frontend::pais::{Pai, Pais};
use dartforge_types::constantes::avaliador::{Constante, Ctx, Motor};
use dartforge_types::constantes::valor::{Campo, Estado, Valor};
use dartforge_types::{Resolved, Type};
use std::collections::HashSet;

/// ARGB de 0 a 255.
pub(crate) type Cor = [u8; 4];

/// `getColorForInt`.
fn cor_do_int(v: i64) -> Cor {
    [((v >> 24) & 0xff) as u8, ((v >> 16) & 0xff) as u8, ((v >> 8) & 0xff) as u8, (v & 0xff) as u8]
}

/// `getColorForDoubles`: `(x * 255.0).round() & 0xff`.
fn cor_dos_doubles(a: f64, r: f64, g: f64, b: f64) -> Cor {
    let c = |x: f64| ((x * 255.0).round() as i64 & 0xff) as u8;
    [c(a), c(r), c(g), c(b)]
}

/// O valor de um literal inteiro (decimal ou hexadecimal, com `_`).
fn inteiro(fonte: &str, s: Span) -> Option<i64> {
    let t: String = fonte.get(s.start..s.end)?.chars().filter(|c| *c != '_').collect();
    match t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        Some(h) => u64::from_str_radix(h, 16).ok().map(|v| v as i64),
        None => t.parse().ok(),
    }
}

fn double(fonte: &str, s: Span) -> Option<f64> {
    fonte.get(s.start..s.end)?.replace('_', "").parse().ok()
}

impl Projeto {
    /// A classe é o `Color` do `dart:ui` ou um subtipo dele (`isColor`).
    fn e_cor(&self, c: ClassId) -> bool {
        let p = self.programa();
        let mut pilha = vec![c];
        let mut vistos = HashSet::new();
        while let Some(x) = pilha.pop() {
            if !vistos.insert(x) {
                continue;
            }
            let cl = p.class(x);
            if self.nome(cl.name) == "Color" && p.library(cl.library).uri == "dart:ui" {
                return true;
            }
            pilha.extend(cl.supertype_class);
            pilha.extend(cl.interface_classes.iter().copied());
            pilha.extend(cl.mixin_classes.iter().copied());
        }
        false
    }

    /// O tipo estático de `e` é de cor.
    fn tipo_de_cor(&self, u: UnitId, e: ExprId) -> bool {
        let Some(t) = self.consulta.corpos.units[u.0 as usize].get_type(e) else { return false };
        match self.consulta.tabela.get(t) {
            Type::Interface { class, .. } => self.e_cor(*class),
            _ => false,
        }
    }

    /// As bibliotecas que a avaliação das cores de `u` precisa inferidas,
    /// além das já inferidas: as das variáveis constantes que as expressões
    /// de cor citam e as dos `Color` do `dart:ui` e do Flutter.
    pub(crate) fn bibliotecas_das_cores(&self, u: UnitId) -> Vec<LibraryId> {
        let p = self.programa();
        let mut v: Vec<LibraryId> = Vec::new();
        let corpos = &self.consulta.corpos.units[u.0 as usize];
        for i in 0..p.unit(u).ast.exprs.len() {
            let e = ExprId(i as u32);
            if !self.tipo_de_cor(u, e) {
                continue;
            }
            let var = match corpos.get_resolved(e) {
                Some(Resolved::Element(dartforge_elements::model::Element::Variable(x))) => Some(*x),
                Some(Resolved::Member { member: dartforge_types::MemberRef::Variable(x), .. }) => Some(*x),
                Some(Resolved::Element(dartforge_elements::model::Element::Function(f))) | Some(Resolved::Member { member: dartforge_types::MemberRef::Function(f), .. }) => p.function(*f).variable,
                _ => None,
            };
            if let Some(x) = var {
                v.push(p.variable(x).library);
            }
        }
        for (i, l) in p.libraries.iter().enumerate() {
            if matches!(
                l.uri.as_str(),
                "dart:ui" | "package:flutter/src/painting/colors.dart" | "package:flutter/src/material/colors.dart" | "package:flutter/src/cupertino/colors.dart"
            ) {
                v.push(LibraryId(i as u32));
            }
        }
        v.sort();
        v.dedup();
        v.retain(|l| !self.bibliotecas.contains(l));
        v
    }

    /// As cores de `u` (`ColorComputer.compute`), na ordem do texto.
    pub(crate) fn cores(&mut self, u: UnitId) -> Vec<(Span, Cor)> {
        let lib = self.programa().unit(u).library;
        let antes_de_3 = self.programa().library(lib).features.versao().major < 3;
        let pais = {
            let un = self.programa().unit(u);
            Pais::novo(&un.ast, &un.unit, &un.source, antes_de_3)
        };
        let n = self.programa().unit(u).ast.exprs.len();
        // A tentativa de cada nó (sem os ancestrais), e se ela encerra a
        // descida.
        let mut achadas: Vec<(ExprId, Span, Cor, bool)> = Vec::new();
        let inferidas: HashSet<LibraryId> = self.bibliotecas.iter().copied().collect();
        for i in 0..n {
            let e = ExprId(i as u32);
            if !self.tipo_de_cor(u, e) {
                continue;
            }
            let (span, kind) = {
                let a = &self.programa().unit(u).ast;
                let x = a.expr(e);
                let corpos = &self.consulta.corpos.units[u.0 as usize];
                (x.span, forma_superficial(a, e, matches!(corpos.get_resolved(e), Some(Resolved::Constructor(_)))))
            };
            let em_const = pais.em_contexto_constante(&self.programa().unit(u).ast, e);
            let cor = match kind {
                Superficial::Index { target, index } => {
                    let fonte = &self.programa().unit(u).source;
                    let indice = match &self.programa().unit(u).ast.expr(index).kind {
                        ExprKind::Int(s) => inteiro(fonte, *s),
                        _ => None,
                    };
                    indice.and_then(|k| self.cor_por_valor(u, &inferidas, target, em_const, Membro::Indice(k))).map(|c| (c, true))
                }
                Superficial::Criacao => {
                    let c = self.cor_por_valor(u, &inferidas, e, em_const, Membro::Nenhum).or_else(|| self.cor_de_construtor_conhecido(u, e));
                    c.map(|c| (c, false))
                }
                Superficial::Prefixado { target, nome } => self
                    .cor_por_valor(u, &inferidas, e, em_const, Membro::Nenhum)
                    .or_else(|| self.cor_por_valor(u, &inferidas, target, em_const, Membro::Nome(nome)))
                    .map(|c| (c, true)),
                Superficial::Propriedade { target, nome } => {
                    self.cor_por_valor(u, &inferidas, target, em_const, Membro::Nome(nome)).map(|c| (c, true))
                }
                Superficial::Identificador => self.cor_por_valor(u, &inferidas, e, em_const, Membro::Nenhum).map(|c| (c, false)),
                Superficial::Outra => None,
            };
            if let Some((c, encerra)) = cor {
                achadas.push((e, span, c, encerra));
            }
        }
        // O que fica sob um nó que achou cor e encerrou a descida não foi
        // visitado.
        let encerrados: HashSet<ExprId> = achadas.iter().filter(|x| x.3).map(|x| x.0).collect();
        let sob_encerrado = |e: ExprId| {
            let mut atual = pais.pai(e);
            for _ in 0..n + 1 {
                match atual {
                    Pai::Expr(p) => {
                        if encerrados.contains(&p) {
                            return true;
                        }
                        atual = pais.pai(p);
                    }
                    _ => return false,
                }
            }
            false
        };
        let mut saida: Vec<(Span, Cor)> = achadas.iter().filter(|x| !sob_encerrado(x.0)).map(|x| (x.1, x.2)).collect();
        // A ordem da visita: o pai antes do filho.
        saida.sort_by_key(|(s, _)| (s.start, std::cmp::Reverse(s.end)));
        saida.dedup_by_key(|(s, _)| s.start);
        saida
    }

    /// `tryAddColor`: o valor constante de `alvo` (ou o membro, ou o índice
    /// da paleta) como cor.
    fn cor_por_valor(&mut self, u: UnitId, inferidas: &HashSet<LibraryId>, alvo: ExprId, em_const: bool, membro: Membro) -> Option<Cor> {
        let lib = self.programa().unit(u).library;
        let valor = {
            let crate::consulta::Consulta { programa, nomes, tabela, core, outline, corpos, .. } = &mut self.consulta;
            let mut motor = Motor::novo(programa, nomes, tabela, core, outline, corpos, inferidas);
            match motor.avaliar(&Ctx::simples(u, lib), alvo, em_const) {
                Constante::Valor(v) if motor.relatos.is_empty() => v,
                _ => return None,
            }
        };
        let valor = match membro {
            Membro::Nenhum => valor,
            Membro::Nome(nome) => {
                let texto = self.nome(nome).to_string();
                match campo(&valor, nome) {
                    Some(v) => v,
                    None => {
                        let k = texto.strip_prefix("shade")?.parse::<i64>().ok()?;
                        self.da_paleta(&valor, k)?
                    }
                }
            }
            Membro::Indice(k) => self.da_paleta(&valor, k)?,
        };
        self.cor_do_valor(&valor)
    }

    /// `_getSwatchColor`: a entrada de chave `k` do campo `_swatch`.
    fn da_paleta(&self, v: &Valor, k: i64) -> Option<Valor> {
        let simbolo = self.consulta.nomes.lookup("_swatch")?;
        let paleta = campo(v, simbolo)?;
        let Estado::Mapa { entradas, .. } = &paleta.estado else { return None };
        entradas.iter().find(|(c, _)| matches!(c.estado, Estado::Int(Some(x)) if x == k)).map(|(_, v)| v.clone())
    }

    /// `getColorForObject`: o campo `color` (se há) e os `a`, `r`, `g`, `b`.
    fn cor_do_valor(&self, v: &Valor) -> Option<Cor> {
        match self.consulta.tabela.get(v.tipo) {
            Type::Interface { class, .. } if self.e_cor(*class) => {}
            _ => return None,
        }
        let nomes = &self.consulta.nomes;
        let v = nomes.lookup("color").and_then(|s| campo(v, s)).unwrap_or_else(|| v.clone());
        let d = |n: &str| -> Option<f64> {
            match campo(&v, nomes.lookup(n)?)?.estado {
                Estado::Double(Some(x)) => Some(x),
                Estado::Int(Some(x)) => Some(x as f64),
                _ => None,
            }
        };
        Some(cor_dos_doubles(d("a")?, d("r")?, d("g")?, d("b")?))
    }

    /// `tryAddKnownColorConstructor`: as chamadas conhecidas com literais.
    fn cor_de_construtor_conhecido(&self, u: UnitId, e: ExprId) -> Option<Cor> {
        let p = self.programa();
        let un = p.unit(u);
        let f = match self.consulta.corpos.units[u.0 as usize].get_resolved(e) {
            Some(Resolved::Constructor(f)) => *f,
            _ => return None,
        };
        let classe = p.function(f).class?;
        let nome_da_classe = self.nome(p.class(classe).name);
        let uri = &p.library(p.class(classe).library).uri;
        let construtor = self.nome(p.function(f).name);
        let args: &[ast::Argument] = match &un.ast.expr(e).kind {
            ExprKind::InstanceCreation { arguments, .. } | ExprKind::Call { arguments, .. } => &arguments.args,
            _ => return None,
        };
        let int = |a: &ast::Argument| match &un.ast.expr(a.value).kind {
            ExprKind::Int(s) => inteiro(&un.source, *s),
            _ => None,
        };
        let num = |a: &ast::Argument| match &un.ast.expr(a.value).kind {
            ExprKind::Int(s) => inteiro(&un.source, *s).map(|x| x as f64),
            ExprKind::Double(s) => double(&un.source, *s),
            _ => None,
        };
        if uri == "dart:ui" && nome_da_classe == "Color" {
            match construtor {
                "" if args.len() == 1 => int(&args[0]).map(cor_do_int),
                "from" => {
                    let mut x = [None; 4];
                    for a in args.iter().filter(|a| a.name.is_some()) {
                        let i = match self.nome(a.name?.sym) {
                            "alpha" => 0,
                            "red" => 1,
                            "green" => 2,
                            "blue" => 3,
                            _ => continue,
                        };
                        x[i] = num(a);
                    }
                    Some(cor_dos_doubles(x[0]?, x[1]?, x[2]?, x[3]?))
                }
                "fromARGB" if args.len() == 4 => {
                    let v: Vec<i64> = args.iter().map(int).collect::<Option<_>>()?;
                    Some([v[0] as u8, v[1] as u8, v[2] as u8, v[3] as u8])
                }
                "fromRGBO" if args.len() == 4 => {
                    let rgb: Vec<i64> = args[..3].iter().map(int).collect::<Option<_>>()?;
                    let alfa = (num(&args[3])? * 255.0) as i64;
                    Some([alfa as u8, rgb[0] as u8, rgb[1] as u8, rgb[2] as u8])
                }
                _ => None,
            }
        } else if (uri == "package:flutter/src/painting/colors.dart" && nome_da_classe == "ColorSwatch")
            || (uri == "package:flutter/src/material/colors.dart" && nome_da_classe == "MaterialAccentColor")
        {
            (construtor.is_empty() && !args.is_empty()).then(|| int(&args[0]).map(cor_do_int)).flatten()
        } else {
            None
        }
    }

    /// `_willRequireConstKeyword`: a expressão em `offset` é constante e não
    /// está num contexto constante.
    pub(crate) fn exige_const(&self, u: UnitId, offset: usize) -> bool {
        let p = self.programa();
        let un = p.unit(u);
        let lib = un.library;
        let pais = Pais::novo(&un.ast, &un.unit, &un.source, p.library(lib).features.versao().major < 3);
        let Some((i, x)) = un.ast.exprs.iter().enumerate().filter(|(_, x)| x.span.start <= offset && offset <= x.span.end).min_by_key(|(_, x)| x.span.end - x.span.start) else {
            return false;
        };
        let e = ExprId(i as u32);
        if pais.em_contexto_constante(&un.ast, e) {
            return false;
        }
        match &x.kind {
            ExprKind::InstanceCreation { keyword, .. } => *keyword == Some(ast::CreationKeyword::Const),
            ExprKind::Identifier(_) | ExprKind::Property { .. } => {
                let corpos = &self.consulta.corpos.units[u.0 as usize];
                let r = match pais.pai(e) {
                    Pai::Expr(pai) if matches!(un.ast.expr(pai).kind, ExprKind::Property { target, .. } if target == e) => corpos.get_resolved(pai),
                    _ => corpos.get_resolved(e),
                };
                let var = match r {
                    Some(Resolved::Element(dartforge_elements::model::Element::Variable(v))) => Some(*v),
                    Some(Resolved::Member { member: dartforge_types::MemberRef::Variable(v), .. }) => Some(*v),
                    Some(Resolved::Element(dartforge_elements::model::Element::Function(f))) | Some(Resolved::Member { member: dartforge_types::MemberRef::Function(f), .. }) => p.function(*f).variable,
                    _ => None,
                };
                var.is_some_and(|v| p.variable(v).const_)
            }
            _ => false,
        }
    }

    /// A classe `Color` do Flutter (`getFlutterClass('Color')`: a do
    /// `dart:ui`, que o `painting` exporta), se o programa a tem.
    pub(crate) fn tem_color_do_flutter(&self) -> bool {
        let p = self.programa();
        p.classes.iter().any(|c| self.nome(c.name) == "Color" && p.library(c.library).uri == "dart:ui")
    }
}

/// O que ler do valor.
enum Membro {
    Nenhum,
    Nome(dartforge_intern::SymbolId),
    Indice(i64),
}

/// A forma do nó para o visitante.
enum Superficial {
    Index { target: ExprId, index: ExprId },
    Criacao,
    Prefixado { target: ExprId, nome: dartforge_intern::SymbolId },
    Propriedade { target: ExprId, nome: dartforge_intern::SymbolId },
    Identificador,
    Outra,
}

/// A forma do nó `e` para o visitante. `construtor`: a chamada resolve para
/// um construtor (a criação sem `new`, que o analyzer reescreve como
/// `InstanceCreationExpression`).
fn forma_superficial(a: &ast::Ast, e: ExprId, construtor: bool) -> Superficial {
    match &a.expr(e).kind {
        ExprKind::Index { target, index, .. } => Superficial::Index { target: *target, index: *index },
        ExprKind::InstanceCreation { .. } => Superficial::Criacao,
        ExprKind::Call { .. } if construtor => Superficial::Criacao,
        // `A.b` com `A` identificador simples é o `PrefixedIdentifier`; o
        // resto, `PropertyAccess`.
        ExprKind::Property { target, name, null_aware: false } if matches!(a.expr(*target).kind, ExprKind::Identifier(_)) => {
            Superficial::Prefixado { target: *target, nome: name.sym }
        }
        ExprKind::Property { target, name, .. } => Superficial::Propriedade { target: *target, nome: name.sym },
        ExprKind::Identifier(_) => Superficial::Identificador,
        _ => Superficial::Outra,
    }
}

/// `getFieldFromHierarchy`: o campo pelo nome, subindo pelo `super`.
fn campo(v: &Valor, nome: dartforge_intern::SymbolId) -> Option<Valor> {
    let Estado::Generico { campos, .. } = &v.estado else { return None };
    if let Some((_, x)) = campos.iter().find(|(c, _)| *c == Campo::Nome(nome)) {
        return Some(x.clone());
    }
    let (_, sup) = campos.iter().find(|(c, _)| *c == Campo::Super)?;
    campo(sup, nome)
}

/// As quatro apresentações de uma cor (`_getPresentations`): o rótulo e o
/// texto que troca o intervalo (com `const ` quando exigido).
pub(crate) fn apresentacoes(alfa: f64, vermelho: f64, verde: f64, azul: f64, com_const: bool) -> Vec<(String, String)> {
    // `toStringAsFixed(3)` sem os zeros finais e o ponto.
    let arredondar = |x: f64| {
        let t = format!("{x:.3}");
        let t = t.trim_end_matches('0');
        t.trim_end_matches('.').to_string()
    };
    let (a, r, g, b) = ((alfa * 255.0) as i64, (vermelho * 255.0) as i64, (verde * 255.0) as i64, (azul * 255.0) as i64);
    let valor = (a << 24) | (r << 16) | (g << 8) | b;
    let hex = format!("0x{:08X}", valor as u32);
    // `num.toString` do Dart para a opacidade: `1.0`, `0.5`.
    let opacidade = if alfa.fract() == 0.0 { format!("{alfa:.1}") } else { format!("{alfa}") };
    let invocacoes = [
        format!(".fromARGB({a}, {r}, {g}, {b})"),
        format!(".fromRGBO({r}, {g}, {b}, {opacidade})"),
        format!(".from(alpha: {}, red: {}, green: {}, blue: {})", arredondar(alfa), arredondar(vermelho), arredondar(verde), arredondar(azul)),
        format!("({hex})"),
    ];
    invocacoes
        .into_iter()
        .map(|i| (format!("Color{i}"), format!("{}Color{i}", if com_const { "const " } else { "" })))
        .collect()
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn cores_e_apresentacoes() {
        assert_eq!(cor_do_int(0xFF336699), [0xFF, 0x33, 0x66, 0x99]);
        assert_eq!(cor_dos_doubles(1.0, 0.5, 0.0, 1.0), [255, 128, 0, 255]);
        let a = apresentacoes(1.0, 0.2, 0.4, 0.6, true);
        assert_eq!(a[0].0, "Color.fromARGB(255, 51, 102, 153)");
        assert_eq!(a[1].0, "Color.fromRGBO(51, 102, 153, 1.0)");
        assert_eq!(a[2].0, "Color.from(alpha: 1, red: 0.2, green: 0.4, blue: 0.6)");
        assert_eq!(a[3].0, "Color(0xFF336699)");
        assert!(a[3].1.starts_with("const Color("));
    }
}

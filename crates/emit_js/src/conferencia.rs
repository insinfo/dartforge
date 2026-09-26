//! Conferência da inferência do emissor contra a inferência comum.
//!
//! O emissor JS deduz o tipo de cada expressão com o seu próprio sistema
//! ([`Ty`], `emit_expr` devolvendo `(Js, Ty)`), enquanto `crates/types` já
//! entrega o tipo estático e o elemento resolvido de cada nó em
//! [`dartforge_types::resolved::BodyTypes`] — que o backend nativo consome
//! diretamente. Este módulo é o primeiro passo para uma fonte única: com
//! `DARTFORGE_JS_CONFERIR_TIPOS` ligado, cada expressão emitida (fora da
//! emissão especulativa) tem o seu `Ty` comparado com o tipo comum, convertido
//! para `Ty`, e cada identificador tem o alvo que o emissor resolveu comparado
//! com o `Resolved` comum. As divergências são registradas; a emissão não muda.
//!
//! Valores da variável:
//! - `1` (ou qualquer valor não vazio que não seja caminho): resumo por
//!   categoria em `stderr` ao fim da emissão;
//! - caminho de arquivo terminado em `.tsv`: além do resumo, cada divergência
//!   vira uma linha acrescentada ao arquivo (arquivo:linha, espécie do nó,
//!   categoria, texto da expressão, tipo do emissor, tipo comum, caminho,
//!   início e comprimento em bytes — estes três para cruzar com o oráculo do
//!   `package:analyzer`, `tools/oraculo_tipos`). Cada emissão acrescenta
//!   antes uma linha `#totais` (expressões conferidas, identificadores
//!   conferidos, alvos de escrita pulados).
//!
//! A comparação normaliza o que é só representação (ver [`normalizar`] e
//! [`equivalentes`]): `FutureOr` como classe ou como `Ty::FutureOr`, a classe
//! `Null` como `Ty::Null`, parâmetros de tipo com o mesmo nome mas ids
//! distintos (o emissor cria ids próprios para funções genéricas locais) e
//! parâmetros ligados de tipos de função, comparados por posição.
//! O inventário do corpus está em `docs/INFERENCIA-JS-ALINHAMENTO.md`.

use crate::body::FnEmitter;
use crate::ctx::{Ctx, MemberKind};
use crate::expr::IdentTarget;
use crate::ty::{Ty, TyParam};
use dartforge_elements::model::{Element, UnitId};
use dartforge_frontend::ast::{AssignOp, ExprId, ExprKind, UnaryOp};
use dartforge_intern::SymbolId;
use dartforge_types::resolved::{MemberRef, Resolved};
use dartforge_types::table::{Type, TypeId};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Write as _;

/// Nome da variável de ambiente que liga a conferência.
pub const VARIAVEL: &str = "DARTFORGE_JS_CONFERIR_TIPOS";

/// Uma divergência entre o emissor e a inferência comum.
#[derive(Clone, Debug)]
pub struct Divergencia {
    /// `arquivo:linha` da expressão.
    pub local: String,
    /// Espécie do nó (`Identifier`, `Call`, …).
    pub no: String,
    /// `tipo` ou `alvo`.
    pub especie: &'static str,
    pub categoria: String,
    /// Texto da expressão na fonte (uma linha, truncado).
    pub texto: String,
    pub emissor: String,
    pub comum: String,
    /// Caminho da unidade e intervalo da expressão em bytes, para cruzar
    /// com o oráculo do analyzer (`tools/oraculo_tipos`).
    pub caminho: String,
    pub inicio: usize,
    pub comprimento: usize,
}

/// Estado da conferência de uma emissão.
#[derive(Default)]
pub struct Conferencia {
    /// Nós já conferidos: o emissor emite a mesma expressão mais de uma vez
    /// (contextos diferentes, reemissões); vale a primeira emissão real.
    tipos_vistos: HashSet<(u32, u32)>,
    alvos_vistos: HashSet<(u32, u32)>,
    /// Por unidade, os alvos de atribuição composta e de `++`/`--`: a
    /// inferência comum registra o tipo de leitura deles no nó da
    /// atribuição, não no alvo (como o analyzer, que dá `readType` à
    /// atribuição e nenhum `staticType` ao alvo). O emissor lê o alvo com
    /// `emit_expr`; esses nós são contados à parte, não conferidos.
    alvos_de_escrita: HashMap<u32, HashSet<u32>>,
    /// Alvos de escrita emitidos (não conferidos).
    pub alvos_de_escrita_pulados: usize,
    pub tipos_conferidos: usize,
    pub alvos_conferidos: usize,
    pub divergencias: Vec<Divergencia>,
    /// Caminho do `.tsv` pedido na variável, se houver.
    saida: Option<std::path::PathBuf>,
}

impl Conferencia {
    /// A conferência pedida pelo ambiente, ou `None` quando desligada.
    pub fn do_ambiente() -> Option<Conferencia> {
        let v = std::env::var(VARIAVEL)
            .ok()
            .filter(|v| !v.is_empty() && v != "0")?;
        let saida = v.ends_with(".tsv").then(|| std::path::PathBuf::from(&v));
        Some(Conferencia {
            saida,
            ..Default::default()
        })
    }

    /// Se `e` é alvo de atribuição composta ou de incremento/decremento.
    fn e_alvo_de_escrita(&mut self, ctx: &Ctx, unit: UnitId, e: ExprId) -> bool {
        let alvos = self.alvos_de_escrita.entry(unit.0).or_insert_with(|| {
            let mut v = HashSet::new();
            for x in &ctx.program.unit(unit).ast.exprs {
                match &x.kind {
                    ExprKind::Assign { op, target, .. } if *op != AssignOp::Assign => {
                        v.insert(target.0);
                    }
                    ExprKind::Unary {
                        op:
                            UnaryOp::PrefixInc
                            | UnaryOp::PrefixDec
                            | UnaryOp::PostfixInc
                            | UnaryOp::PostfixDec,
                        operand,
                    } => {
                        v.insert(operand.0);
                    }
                    _ => {}
                }
            }
            v
        });
        alvos.contains(&e.0)
    }

    /// Resumo por categoria em `stderr` e, se pedido, as divergências no `.tsv`.
    pub fn relatar(&self) {
        let (mut tipos, mut alvos) = (0usize, 0usize);
        let mut por_categoria: BTreeMap<(&str, &str), usize> = BTreeMap::new();
        for d in &self.divergencias {
            if d.especie == "tipo" {
                tipos += 1
            } else {
                alvos += 1
            }
            *por_categoria
                .entry((d.especie, d.categoria.as_str()))
                .or_default() += 1;
        }
        eprintln!(
            "conferência de tipos: {} expressões, {tipos} divergem ({} alvos de escrita não conferidos); {} identificadores, {alvos} com alvo divergente",
            self.tipos_conferidos, self.alvos_de_escrita_pulados, self.alvos_conferidos
        );
        let mut v: Vec<_> = por_categoria.into_iter().collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        for ((especie, cat), n) in v {
            eprintln!("{n:7}  {especie}: {cat}");
        }
        let Some(p) = &self.saida else { return };
        let arquivo = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(p);
        match arquivo {
            Ok(mut f) => {
                let mut texto = String::new();
                // Linha de totais, para quem soma vários programas.
                texto.push_str(&format!(
                    "#totais\t{}\t{}\t{}\n",
                    self.tipos_conferidos, self.alvos_conferidos, self.alvos_de_escrita_pulados
                ));
                for d in &self.divergencias {
                    texto.push_str(&format!(
                        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                        d.local,
                        d.no,
                        d.especie,
                        d.categoria,
                        d.texto,
                        d.emissor,
                        d.comum,
                        d.caminho,
                        d.inicio,
                        d.comprimento
                    ));
                }
                if let Err(e) = f.write_all(texto.as_bytes()) {
                    eprintln!("aviso: {}: {e}", p.display());
                }
            }
            Err(e) => eprintln!("aviso: {}: {e}", p.display()),
        }
    }
}

impl<'m, 'a> FnEmitter<'m, 'a> {
    /// Confere o tipo `ty` que o emissor deduziu para `e` (antes das
    /// coerções de contexto) com o tipo estático comum.
    pub(crate) fn conferir_tipo(&self, e: ExprId, ty: &Ty) {
        let Some(c) = &self.ctx.conferencia else {
            return;
        };
        if self.especulando > 0 || e.0 == u32::MAX {
            return;
        }
        if !c.borrow_mut().tipos_vistos.insert((self.unit.0, e.0)) {
            return;
        }
        if c.borrow_mut().e_alvo_de_escrita(self.ctx, self.unit, e) {
            c.borrow_mut().alvos_de_escrita_pulados += 1;
            return;
        }
        c.borrow_mut().tipos_conferidos += 1;
        let Some(comum) = tipo_comum(self.ctx, self.unit, e) else {
            return;
        };
        let comum = ty_da_tabela(self.ctx, comum);
        let (a, b) = (normalizar(self.ctx, ty), normalizar(self.ctx, &comum));
        if equivalentes(&a, &b, &mut Vec::new()) {
            return;
        }
        let categoria = categoria_de_tipo(&a, &b).to_string();
        let d = self.divergencia(
            e,
            "tipo",
            categoria,
            mostrar(self.ctx, &a),
            mostrar(self.ctx, &b),
        );
        c.borrow_mut().divergencias.push(d);
    }

    /// Confere o alvo que o emissor resolveu para o identificador `e` com o
    /// `Resolved` comum. Alvos sem identidade estável no emissor (membros
    /// procurados por nome no `$this` de extensão) não são conferidos.
    pub(crate) fn conferir_alvo(&self, e: ExprId, alvo: &IdentTarget) {
        let Some(c) = &self.ctx.conferencia else {
            return;
        };
        if self.especulando > 0 || e.0 == u32::MAX {
            return;
        }
        if !c.borrow_mut().alvos_vistos.insert((self.unit.0, e.0)) {
            return;
        }
        let nome = match &self.expr(e).kind {
            ExprKind::Identifier(n) => Some(n.sym),
            _ => None,
        };
        let Some(emissor) = chave_do_emissor(self.ctx, alvo, nome) else {
            return;
        };
        c.borrow_mut().alvos_conferidos += 1;
        let comum = self
            .ctx
            .bodies
            .units
            .get(self.unit.0 as usize)
            .and_then(|u| u.get_resolved(e))
            .map(|r| chave_comum(self.ctx, r))
            .unwrap_or_else(|| "sem resolução".to_string());
        // Nome sem declaração (`dynamic`/`Never` como valor, nome indefinido):
        // nenhum dos dois tem elemento.
        if emissor == comum || (emissor == "desconhecido" && comum == "sem resolução") {
            return;
        }
        let categoria = format!(
            "{} × {}",
            especie_de_chave(&emissor),
            especie_de_chave(&comum)
        );
        let d = self.divergencia(e, "alvo", categoria, emissor, comum);
        c.borrow_mut().divergencias.push(d);
    }

    fn divergencia(
        &self,
        e: ExprId,
        especie: &'static str,
        categoria: String,
        emissor: String,
        comum: String,
    ) -> Divergencia {
        let unit = self.ctx.program.unit(self.unit);
        let expr = self.expr(e);
        let linha = unit.source[..expr.span.start.min(unit.source.len())]
            .matches('\n')
            .count()
            + 1;
        let arquivo = unit
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| unit.uri.clone());
        let mut texto: String = self
            .text(expr.span)
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if texto.chars().count() > 60 {
            texto = texto.chars().take(57).collect::<String>() + "...";
        }
        let no = format!("{:?}", expr.kind);
        let no = no
            .split(|c: char| !c.is_alphanumeric())
            .next()
            .unwrap_or("")
            .to_string();
        let caminho = unit
            .path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| unit.uri.clone());
        Divergencia {
            caminho,
            inicio: expr.span.start,
            comprimento: expr.span.end - expr.span.start,
            local: format!("{arquivo}:{linha}"),
            no,
            especie,
            categoria,
            texto,
            emissor,
            comum,
        }
    }
}

/// Tipo estático comum de `e`, se a unidade foi inferida.
fn tipo_comum(ctx: &Ctx, unit: UnitId, e: ExprId) -> Option<TypeId> {
    ctx.bodies
        .units
        .get(unit.0 as usize)
        .and_then(|u| u.get_type(e))
}

/// Converte um tipo da `TypeTable` para `Ty` sem efeitos colaterais no
/// contexto (ao contrário de [`Ctx::ty_of`], que registra limites de
/// parâmetros de tipo usados depois pela emissão): a conferência não pode
/// mudar o JS emitido.
pub fn ty_da_tabela(ctx: &Ctx, id: TypeId) -> Ty {
    match ctx.table.get(id) {
        Type::Dynamic => Ty::Dynamic,
        Type::Void => Ty::Void,
        Type::Never => Ty::Never,
        Type::Null => Ty::Null,
        Type::Interface {
            class,
            args,
            nullable,
        } => {
            let args: Vec<Ty> = args.iter().map(|a| ty_da_tabela(ctx, *a)).collect();
            let args = ctx.args_na_aridade(*class, &args).into_owned();
            Ty::Iface {
                class: *class,
                args,
                nullable: *nullable,
            }
        }
        Type::Function {
            type_params,
            ret,
            positional,
            optional,
            named,
            nullable,
        } => Ty::Fn {
            type_params: type_params
                .iter()
                .map(|p| {
                    let data = ctx.table.param(*p);
                    TyParam {
                        id: p.0,
                        name: ctx.interner.resolve(data.name).to_string(),
                        bound: Box::new(ty_da_tabela(ctx, data.bound)),
                    }
                })
                .collect(),
            ret: Box::new(ty_da_tabela(ctx, *ret)),
            pos: positional.iter().map(|a| ty_da_tabela(ctx, *a)).collect(),
            opt: optional.iter().map(|a| ty_da_tabela(ctx, *a)).collect(),
            named: {
                let mut v: Vec<(String, Ty, bool)> = named
                    .iter()
                    .map(|(n, t, r)| {
                        (
                            ctx.interner.resolve(*n).to_string(),
                            ty_da_tabela(ctx, *t),
                            *r,
                        )
                    })
                    .collect();
                v.sort_by(|a, b| a.0.cmp(&b.0));
                v
            },
            nullable: *nullable,
        },
        Type::Record {
            positional,
            named,
            nullable,
        } => {
            let mut named: Vec<(String, Ty)> = named
                .iter()
                .map(|(n, t)| (ctx.interner.resolve(*n).to_string(), ty_da_tabela(ctx, *t)))
                .collect();
            named.sort_by(|a, b| a.0.cmp(&b.0));
            Ty::Record {
                pos: positional.iter().map(|a| ty_da_tabela(ctx, *a)).collect(),
                named,
                nullable: *nullable,
            }
        }
        Type::TypeParameter { param, nullable } => {
            let name = ctx
                .interner
                .resolve(ctx.table.param(*param).name)
                .to_string();
            Ty::Param {
                id: param.0,
                name,
                nullable: *nullable,
            }
        }
        Type::FutureOr { arg, nullable } => Ty::FutureOr {
            arg: Box::new(ty_da_tabela(ctx, *arg)),
            nullable: *nullable,
        },
        // `X & B` só existe na inferência; em execução (e no emissor) é `X`.
        Type::Intersection { param, .. } => {
            let name = ctx
                .interner
                .resolve(ctx.table.param(*param).name)
                .to_string();
            Ty::Param {
                id: param.0,
                name,
                nullable: false,
            }
        }
        Type::ExtensionType {
            decl,
            args,
            nullable,
        } => {
            if ctx.interop_ext_types.contains(decl) {
                let args: Vec<Ty> = args.iter().map(|a| ty_da_tabela(ctx, *a)).collect();
                let args = ctx.args_na_aridade(*decl, &args).into_owned();
                return Ty::Iface {
                    class: *decl,
                    args,
                    nullable: *nullable,
                };
            }
            // Apagado para o tipo de representação, como faz `Ctx::ty_of`.
            let class = ctx.program.class(*decl);
            let Some(rep) = class.representation else {
                return Ty::Dynamic;
            };
            let v = &ctx.outline.variables[rep.0 as usize];
            let Some(t) = v.declared_type.or(v.inferred) else {
                return Ty::Dynamic;
            };
            let mut map = HashMap::new();
            if let Some(params) = ctx.class_params.get(decl.0 as usize) {
                for (p, a) in params.iter().zip(args.iter()) {
                    map.insert(p.id, ty_da_tabela(ctx, *a));
                }
            }
            let t = ty_da_tabela(ctx, t).subst_prop(&map);
            if *nullable { t.with_nullable(true) } else { t }
        }
    }
}

/// Forma canônica para comparar: `FutureOr` como classe vira
/// `Ty::FutureOr`, a classe `Null` vira `Ty::Null`, `Never?` vira `Null`,
/// `FutureOr<T>?` com `T` anulável perde o `?` redundante.
pub fn normalizar(ctx: &Ctx, t: &Ty) -> Ty {
    match t {
        Ty::Iface {
            class,
            args,
            nullable,
        } if Some(*class) == ctx.future_or => {
            let arg = args
                .first()
                .map(|a| normalizar(ctx, a))
                .unwrap_or(Ty::Dynamic);
            Ty::FutureOr {
                arg: Box::new(arg),
                nullable: *nullable,
            }
        }
        Ty::Iface { class, .. } if Some(*class) == ctx.null_ => Ty::Null,
        Ty::Iface {
            class,
            args,
            nullable,
        } => Ty::Iface {
            class: *class,
            args: args.iter().map(|a| normalizar(ctx, a)).collect(),
            nullable: *nullable,
        },
        Ty::Fn {
            type_params,
            ret,
            pos,
            opt,
            named,
            nullable,
        } => Ty::Fn {
            type_params: type_params
                .iter()
                .map(|p| TyParam {
                    id: p.id,
                    name: p.name.clone(),
                    bound: Box::new(normalizar(ctx, &p.bound)),
                })
                .collect(),
            ret: Box::new(normalizar(ctx, ret)),
            pos: pos.iter().map(|a| normalizar(ctx, a)).collect(),
            opt: opt.iter().map(|a| normalizar(ctx, a)).collect(),
            named: named
                .iter()
                .map(|(n, t, r)| (n.clone(), normalizar(ctx, t), *r))
                .collect(),
            nullable: *nullable,
        },
        Ty::Record {
            pos,
            named,
            nullable,
        } => Ty::Record {
            pos: pos.iter().map(|a| normalizar(ctx, a)).collect(),
            named: named
                .iter()
                .map(|(n, t)| (n.clone(), normalizar(ctx, t)))
                .collect(),
            nullable: *nullable,
        },
        Ty::FutureOr { arg, nullable } => {
            let arg = normalizar(ctx, arg);
            let nullable = *nullable && !arg.is_nullable();
            Ty::FutureOr {
                arg: Box::new(arg),
                nullable,
            }
        }
        _ => t.clone(),
    }
}

/// Igualdade estrutural módulo representação: parâmetros ligados de tipos de
/// função casam por posição (`ligados`: pares de ids), parâmetros livres por
/// id ou, com ids diferentes, pelo nome.
pub fn equivalentes(a: &Ty, b: &Ty, ligados: &mut Vec<(u32, u32)>) -> bool {
    match (a, b) {
        (Ty::Dynamic, Ty::Dynamic)
        | (Ty::Void, Ty::Void)
        | (Ty::Never, Ty::Never)
        | (Ty::Null, Ty::Null) => true,
        (
            Ty::Iface {
                class: c1,
                args: a1,
                nullable: n1,
            },
            Ty::Iface {
                class: c2,
                args: a2,
                nullable: n2,
            },
        ) => {
            c1 == c2
                && n1 == n2
                && a1.len() == a2.len()
                && a1.iter().zip(a2).all(|(x, y)| equivalentes(x, y, ligados))
        }
        (
            Ty::Fn {
                type_params: p1,
                ret: r1,
                pos: s1,
                opt: o1,
                named: m1,
                nullable: n1,
            },
            Ty::Fn {
                type_params: p2,
                ret: r2,
                pos: s2,
                opt: o2,
                named: m2,
                nullable: n2,
            },
        ) => {
            if n1 != n2
                || p1.len() != p2.len()
                || s1.len() != s2.len()
                || o1.len() != o2.len()
                || m1.len() != m2.len()
            {
                return false;
            }
            let marca = ligados.len();
            ligados.extend(p1.iter().zip(p2).map(|(x, y)| (x.id, y.id)));
            let ok = p1
                .iter()
                .zip(p2)
                .all(|(x, y)| equivalentes(&x.bound, &y.bound, ligados))
                && equivalentes(r1, r2, ligados)
                && s1.iter().zip(s2).all(|(x, y)| equivalentes(x, y, ligados))
                && o1.iter().zip(o2).all(|(x, y)| equivalentes(x, y, ligados))
                && m1.iter().zip(m2).all(|((k1, t1, q1), (k2, t2, q2))| {
                    k1 == k2 && q1 == q2 && equivalentes(t1, t2, ligados)
                });
            ligados.truncate(marca);
            ok
        }
        (
            Ty::Record {
                pos: s1,
                named: m1,
                nullable: n1,
            },
            Ty::Record {
                pos: s2,
                named: m2,
                nullable: n2,
            },
        ) => {
            n1 == n2
                && s1.len() == s2.len()
                && m1.len() == m2.len()
                && s1.iter().zip(s2).all(|(x, y)| equivalentes(x, y, ligados))
                && m1
                    .iter()
                    .zip(m2)
                    .all(|((k1, t1), (k2, t2))| k1 == k2 && equivalentes(t1, t2, ligados))
        }
        (
            Ty::Param {
                id: i1,
                name: x1,
                nullable: n1,
            },
            Ty::Param {
                id: i2,
                name: x2,
                nullable: n2,
            },
        ) => {
            if n1 != n2 {
                return false;
            }
            // O ligado mais interno decide; livre casa por id ou nome.
            match ligados.iter().rev().find(|(x, y)| x == i1 || y == i2) {
                Some((x, y)) => x == i1 && y == i2,
                None => i1 == i2 || x1 == x2,
            }
        }
        (
            Ty::FutureOr {
                arg: a1,
                nullable: n1,
            },
            Ty::FutureOr {
                arg: a2,
                nullable: n2,
            },
        ) => n1 == n2 && equivalentes(a1, a2, ligados),
        _ => false,
    }
}

/// Categoria de uma divergência de tipos (`a` do emissor, `b` comum).
fn categoria_de_tipo(a: &Ty, b: &Ty) -> &'static str {
    let especie = |t: &Ty| match t {
        Ty::Dynamic => 0,
        Ty::Void => 1,
        Ty::Never | Ty::Null => 2,
        Ty::Iface { .. } => 3,
        Ty::Fn { .. } => 4,
        Ty::Record { .. } => 5,
        Ty::Param { .. } => 6,
        Ty::FutureOr { .. } => 7,
    };
    if a.is_dynamic() {
        return "emissor dynamic, comum preciso";
    }
    if b.is_dynamic() {
        return "comum dynamic, emissor preciso";
    }
    if equivalentes(
        &a.with_nullable(false),
        &b.with_nullable(false),
        &mut Vec::new(),
    ) {
        return "só a nulabilidade de topo";
    }
    match (a, b) {
        (Ty::Iface { class: c1, .. }, Ty::Iface { class: c2, .. }) if c1 == c2 => {
            "mesma classe, argumentos de tipo diferentes"
        }
        (Ty::Iface { .. }, Ty::Iface { .. }) => "classes diferentes",
        // O emissor instancia o tear-off genérico na coerção (`coerce_to`);
        // a inferência comum já registra no nó o tipo instanciado, e o
        // analyzer, o genérico (a instanciação é um nó à parte).
        (
            Ty::Fn {
                type_params: p1,
                pos: s1,
                opt: o1,
                named: m1,
                ..
            },
            Ty::Fn {
                type_params: p2,
                pos: s2,
                opt: o2,
                named: m2,
                ..
            },
        ) if !p1.is_empty()
            && p2.is_empty()
            && s1.len() == s2.len()
            && o1.len() == o2.len()
            && m1.len() == m2.len() =>
        {
            "tear-off genérico: comum já instanciado"
        }
        (Ty::Fn { .. }, Ty::Fn { .. }) => "tipos de função diferentes",
        (Ty::Record { .. }, Ty::Record { .. }) => "records diferentes",
        (Ty::Param { .. }, Ty::Param { .. }) => "parâmetros de tipo diferentes",
        (Ty::FutureOr { .. }, Ty::FutureOr { .. }) => "FutureOr com argumentos diferentes",
        _ if especie(a) == 1 || especie(b) == 1 => "void × outro",
        _ if especie(a) == 2 || especie(b) == 2 => "Never/Null × outro",
        _ if especie(a) == 6 || especie(b) == 6 => "parâmetro de tipo × outro",
        _ if especie(a) == 7 || especie(b) == 7 => "FutureOr × outro",
        _ => "espécies diferentes",
    }
}

/// Texto de um tipo no estilo do Dart (`List<int>?`, `int Function(String)`).
pub fn mostrar(ctx: &Ctx, t: &Ty) -> String {
    let q = |n: bool| if n { "?" } else { "" };
    let lista = |v: &[Ty]| {
        v.iter()
            .map(|a| mostrar(ctx, a))
            .collect::<Vec<_>>()
            .join(", ")
    };
    match t {
        Ty::Dynamic => "dynamic".into(),
        Ty::Void => "void".into(),
        Ty::Never => "Never".into(),
        Ty::Null => "Null".into(),
        Ty::Iface {
            class,
            args,
            nullable,
        } => {
            let n = ctx.class_name(*class);
            if args.is_empty() {
                format!("{n}{}", q(*nullable))
            } else {
                format!("{n}<{}>{}", lista(args), q(*nullable))
            }
        }
        Ty::Fn {
            type_params,
            ret,
            pos,
            opt,
            named,
            nullable,
        } => {
            let tp = if type_params.is_empty() {
                String::new()
            } else {
                format!(
                    "<{}>",
                    type_params
                        .iter()
                        .map(|p| {
                            // Como o analyzer: o limite só aparece quando não é
                            // o implícito (`Object?`/`dynamic`).
                            let implicito = p.bound.is_dynamic()
                                || (p.bound.is_class(ctx.object) && p.bound.is_nullable());
                            if implicito {
                                p.name.clone()
                            } else {
                                format!("{} extends {}", p.name, mostrar(ctx, &p.bound))
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };
            let mut ps: Vec<String> = pos.iter().map(|a| mostrar(ctx, a)).collect();
            if !opt.is_empty() {
                ps.push(format!("[{}]", lista(opt)));
            }
            if !named.is_empty() {
                let v: Vec<String> = named
                    .iter()
                    .map(|(n, t, r)| {
                        format!(
                            "{}{} {n}",
                            if *r { "required " } else { "" },
                            mostrar(ctx, t)
                        )
                    })
                    .collect();
                ps.push(format!("{{{}}}", v.join(", ")));
            }
            let f = format!("{} Function{tp}({})", mostrar(ctx, ret), ps.join(", "));
            // Como o analyzer: `String Function(int)?`.
            if *nullable { format!("{f}?") } else { f }
        }
        Ty::Record {
            pos,
            named,
            nullable,
        } => {
            let mut ps: Vec<String> = pos.iter().map(|a| mostrar(ctx, a)).collect();
            if !named.is_empty() {
                ps.push(format!(
                    "{{{}}}",
                    named
                        .iter()
                        .map(|(n, t)| format!("{} {n}", mostrar(ctx, t)))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            if pos.len() == 1 && named.is_empty() {
                ps[0].push(',');
            }
            format!("({}){}", ps.join(", "), q(*nullable))
        }
        Ty::Param { name, nullable, .. } => format!("{name}{}", q(*nullable)),
        Ty::FutureOr { arg, nullable } => {
            format!("FutureOr<{}>{}", mostrar(ctx, arg), q(*nullable))
        }
    }
}

/// Chave de um elemento de topo: a identidade da declaração, com o nome.
fn chave_de_elemento(ctx: &Ctx, el: &Element) -> String {
    let p = ctx.program;
    let nome = |s| ctx.interner.resolve(s);
    match *el {
        Element::Class(c) => format!("classe {}", ctx.class_name(c)),
        Element::Extension(x) => format!("extensão #{}", x.0),
        Element::Typedef(t) => format!("typedef #{}", t.0),
        // Acessor implícito de campo/variável: a identidade é a variável
        // (o emissor resolve para o campo; a inferência comum, para o getter).
        Element::Function(f) => match p.function(f).variable {
            Some(v) => chave_de_elemento(ctx, &Element::Variable(v)),
            None => format!("função {}#{}", nome(p.function(f).name), f.0),
        },
        Element::Variable(v) => format!("variável {}#{}", nome(p.variable(v).name), v.0),
        Element::Prefix(..) => "prefixo".into(),
    }
}

/// Chave do alvo que o emissor resolveu para o identificador `nome`; `None`
/// para alvos sem identidade estável (membro achado por nome no `$this` de
/// extensão, extensão aplicável a `this`).
fn chave_do_emissor(ctx: &Ctx, alvo: &IdentTarget, nome: Option<SymbolId>) -> Option<String> {
    let membro = |mk: &MemberKind| match *mk {
        MemberKind::Method(f) | MemberKind::Getter(f) | MemberKind::Setter(f) => {
            chave_de_elemento(ctx, &Element::Function(f))
        }
        MemberKind::Field(v) => chave_de_elemento(ctx, &Element::Variable(v)),
    };
    Some(match alvo {
        IdentTarget::Local(..) => "local".into(),
        IdentTarget::TypeParam(_) => "parâmetro de tipo".into(),
        IdentTarget::ThisMember(m) => membro(&m.kind),
        IdentTarget::Static(_, mk) => membro(mk),
        IdentTarget::Element(el) => chave_de_elemento(ctx, el),
        IdentTarget::Prefix(_) => "prefixo".into(),
        IdentTarget::ExtMember(_, f) | IdentTarget::ExtStatic(_, f) => {
            chave_de_elemento(ctx, &Element::Function(*f))
        }
        IdentTarget::Unknown => "desconhecido".into(),
        // Campo estático da extensão: o emissor guarda a extensão; o campo
        // é o de mesmo nome.
        IdentTarget::ExtField(x) => {
            let nome = nome?;
            let v = ctx
                .program
                .extension(*x)
                .fields
                .iter()
                .find(|&&v| ctx.program.variable(v).name == nome)?;
            chave_de_elemento(ctx, &Element::Variable(*v))
        }
        IdentTarget::ExtThisMember(_) | IdentTarget::ThisExt => return None,
    })
}

/// Chave do alvo comum, no mesmo espaço de [`chave_do_emissor`].
fn chave_comum(ctx: &Ctx, r: &Resolved) -> String {
    match r {
        Resolved::Local(_) | Resolved::Parameter { .. } => "local".into(),
        Resolved::TypeParameter(_) => "parâmetro de tipo".into(),
        Resolved::Element(el) => chave_de_elemento(ctx, el),
        Resolved::Member {
            member: MemberRef::Function(f),
            ..
        } => chave_de_elemento(ctx, &Element::Function(*f)),
        Resolved::Member {
            member: MemberRef::Variable(v),
            ..
        } => chave_de_elemento(ctx, &Element::Variable(*v)),
        Resolved::Prefix(_) => "prefixo".into(),
        Resolved::Dynamic => "dynamic".into(),
        Resolved::ExtensionMember { member, .. } => {
            chave_de_elemento(ctx, &Element::Function(*member))
        }
        Resolved::Constructor(f) => format!("construtor #{}", f.0),
    }
}

/// Espécie de uma chave (a primeira palavra), para categorizar.
fn especie_de_chave(k: &str) -> &str {
    k.split(' ').next().unwrap_or(k)
}

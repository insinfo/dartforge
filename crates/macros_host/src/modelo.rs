//! O modelo que o hospedeiro serve ao executor (docs/MACROS-PROTOCOLO.md §5):
//! declarações e anotações de tipo do `elements` em JSON, com identificadores
//! **estáveis** entre as fases.
//!
//! Entre uma fase e outra o programa é recarregado com a augmentation
//! parcial, e os ids do `elements` mudam (uma classe nova da fase 1 desloca
//! os `ClassId`). A montagem funde os resultados de todas as fases pelo
//! identificador do tipo aumentado (Regra 1), então o mesmo tipo tem de ter o
//! mesmo id na fase 2 e na 3: o id é o de uma [`Chave`] textual (biblioteca,
//! dono, nome, tipo), resolvida de novo no programa corrente a cada uso.
//!
//! O que cada declaração mostra segue o CFE 3.6.2 (o oráculo), inclusive
//! onde ele diverge da spec (`kernel_macro_introspectors.dart`): membros na
//! ordem de declaração (não lexicográfica), `hasBody` sempre verdadeiro,
//! metadados vazios, sem parâmetros de tipo em métodos e construtores, tipo
//! de retorno de construtor omitido.
use dartforge_elements::model::*;
use dartforge_frontend::ast::{self, DeclKind, MemberKind, ParameterKind, TypeKind};
use dartforge_intern::{Interner, SymbolId};
use serde_json::{Value, json};
use std::collections::HashMap;

/// O que um identificador denota, em termos que sobrevivem à recarga.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Chave {
    /// Classe, mixin, enum ou extension type de topo.
    Tipo { lib: String, nome: String },
    Typedef { lib: String, nome: String },
    Extension { lib: String, nome: String },
    /// Função, getter ou setter de topo (`setter` distingue o par).
    FuncaoDeTopo { lib: String, nome: String, setter: bool },
    VariavelDeTopo { lib: String, nome: String },
    /// Método, getter, setter ou operador.
    Metodo { lib: String, dono: String, nome: String, setter: bool },
    Campo { lib: String, dono: String, nome: String },
    /// Construtor (`""` para o sem nome).
    Construtor { lib: String, dono: String, nome: String },
    /// Parâmetro formal de um membro ([`Chave`] do membro em `dono`).
    Parametro { dono: Box<Chave>, nome: String },
    /// Parâmetro de tipo de uma classe ou typedef.
    ParametroDeTipo { dono: Box<Chave>, nome: String },
    Void,
    Dynamic,
    /// Nome que não resolveu (programa inválido): sai como está.
    Solto(String),
}

impl Chave {
    /// O nome do identificador.
    pub fn nome(&self) -> &str {
        match self {
            Chave::Tipo { nome, .. }
            | Chave::Typedef { nome, .. }
            | Chave::Extension { nome, .. }
            | Chave::FuncaoDeTopo { nome, .. }
            | Chave::VariavelDeTopo { nome, .. }
            | Chave::Metodo { nome, .. }
            | Chave::Campo { nome, .. }
            | Chave::Construtor { nome, .. }
            | Chave::Parametro { nome, .. }
            | Chave::ParametroDeTipo { nome, .. } => nome,
            Chave::Void => "void",
            Chave::Dynamic => "dynamic",
            Chave::Solto(n) => n,
        }
    }
}

/// Onde fica um tipo omitido (para `inferType` e para a montagem).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Omitido {
    pub dono: Chave,
    /// `retorno`, `tipo` (de campo/variável) ou o nome do parâmetro.
    pub lugar: String,
}

/// A tabela de ids da sessão de macros: persistente entre as recargas.
#[derive(Debug, Default)]
pub struct Tabela {
    chaves: Vec<Chave>,
    por_chave: HashMap<Chave, u64>,
    omitidos: Vec<Omitido>,
    por_omitido: HashMap<Omitido, u64>,
    bibliotecas: Vec<String>,
}

impl Tabela {
    /// O id de `c` (a partir de 1).
    pub fn id(&mut self, c: Chave) -> u64 {
        if let Some(&id) = self.por_chave.get(&c) {
            return id;
        }
        self.chaves.push(c.clone());
        let id = self.chaves.len() as u64;
        self.por_chave.insert(c, id);
        id
    }

    pub fn chave(&self, id: u64) -> Option<&Chave> {
        self.chaves.get((id as usize).wrapping_sub(1))
    }

    pub fn omitido(&mut self, o: Omitido) -> u64 {
        if let Some(&id) = self.por_omitido.get(&o) {
            return id;
        }
        self.omitidos.push(o.clone());
        let id = self.omitidos.len() as u64;
        self.por_omitido.insert(o, id);
        id
    }

    pub fn onde_omitido(&self, id: u64) -> Option<&Omitido> {
        self.omitidos.get((id as usize).wrapping_sub(1))
    }

    /// O id de uma biblioteca (pela URI).
    pub fn biblioteca(&mut self, uri: &str) -> u64 {
        match self.bibliotecas.iter().position(|u| u == uri) {
            Some(i) => i as u64 + 1,
            None => {
                self.bibliotecas.push(uri.to_string());
                self.bibliotecas.len() as u64
            }
        }
    }

    pub fn uri_da_biblioteca(&self, id: u64) -> Option<&str> {
        self.bibliotecas.get((id as usize).wrapping_sub(1)).map(String::as_str)
    }
}

/// Parâmetros de tipo visíveis numa anotação: nome → chave.
type Escopo = Vec<(SymbolId, Chave)>;

/// O programa corrente visto pelo modelo.
pub struct Vista<'p> {
    pub program: &'p Program,
    pub interner: &'p Interner,
}

impl<'p> Vista<'p> {
    fn n(&self, s: SymbolId) -> String {
        self.interner.resolve(s).to_string()
    }

    pub fn biblioteca_por_uri(&self, uri: &str) -> Option<LibraryId> {
        self.program.libraries.iter().position(|l| l.uri == uri).map(|i| LibraryId(i as u32))
    }

    fn uri(&self, lib: LibraryId) -> String {
        self.program.library(lib).uri.clone()
    }

    /// A chave de um elemento de topo ou de classe.
    pub fn chave_da_classe(&self, c: ClassId) -> Chave {
        let cl = self.program.class(c);
        Chave::Tipo { lib: self.uri(cl.library), nome: self.n(cl.name) }
    }

    /// A classe de uma chave de tipo, no programa corrente.
    pub fn classe(&self, c: &Chave) -> Option<ClassId> {
        let Chave::Tipo { lib, nome } = c else { return None };
        let l = self.biblioteca_por_uri(lib)?;
        let s = self.interner.lookup(nome)?;
        match self.program.library(l).declared.get(&s)?.getter? {
            Element::Class(id) => Some(id),
            _ => None,
        }
    }

    /// O elemento de função (método, acessor, construtor) de uma chave.
    pub fn funcao(&self, c: &Chave) -> Option<FunctionElementId> {
        match c {
            Chave::Metodo { lib, dono, nome, setter } => {
                let cl = self.program.class(self.classe(&Chave::Tipo { lib: lib.clone(), nome: dono.clone() })?);
                let chave = if *setter { format!("{nome}_=") } else { nome.clone() };
                let s = self.interner.lookup(&chave)?;
                cl.instance_members.get(&s).or_else(|| cl.static_members.get(&s)).copied()
            }
            Chave::Construtor { lib, dono, nome } => {
                let cl = self.program.class(self.classe(&Chave::Tipo { lib: lib.clone(), nome: dono.clone() })?);
                let s = self.interner.lookup(nome)?;
                cl.constructors.get(&s).copied()
            }
            Chave::FuncaoDeTopo { lib, nome, setter } => {
                let l = self.biblioteca_por_uri(lib)?;
                let b = self.program.library(l).declared.get(&self.interner.lookup(nome)?)?;
                match if *setter { b.setter } else { b.getter }? {
                    Element::Function(f) => Some(f),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    // ------------------------------------------------------------- tipos

    /// A chave de um nome de tipo escrito numa unidade.
    fn chave_do_nome(&self, lib: LibraryId, nome: &[ast::Name], escopo: &Escopo) -> Chave {
        let de_elemento = |el: Option<Element>, texto: String| match el {
            Some(Element::Class(c)) => self.chave_da_classe(c),
            Some(Element::Typedef(t)) => {
                let td = self.program.typedef(t);
                Chave::Typedef { lib: self.uri(td.library), nome: self.n(td.name) }
            }
            Some(Element::Extension(e)) => {
                let ex = self.program.extension(e);
                Chave::Extension { lib: self.uri(ex.library), nome: ex.name.map(|s| self.n(s)).unwrap_or_default() }
            }
            _ => Chave::Solto(texto),
        };
        match nome {
            [n] => {
                if let Some((_, c)) = escopo.iter().rev().find(|(s, _)| *s == n.sym) {
                    return c.clone();
                }
                let texto = self.n(n.sym);
                if texto == "dynamic" {
                    return Chave::Dynamic;
                }
                de_elemento(self.program.lookup(lib, n.sym).and_then(|b| b.getter), texto)
            }
            [p, n] => de_elemento(
                self.program.lookup_prefixed(lib, p.sym, n.sym).and_then(|b| b.getter),
                format!("{}.{}", self.n(p.sym), self.n(n.sym)),
            ),
            _ => Chave::Solto(nome.iter().map(|x| self.n(x.sym)).collect::<Vec<_>>().join(".")),
        }
    }

    /// Uma anotação de tipo escrita, ou omitida (`None`).
    pub fn tipo(
        &self,
        t: &mut Tabela,
        unit: UnitId,
        tipo: Option<ast::TypeId>,
        escopo: &Escopo,
        omitido: impl FnOnce() -> Omitido,
    ) -> Value {
        let Some(tid) = tipo else {
            return json!({"t": "omitido", "chave": t.omitido(omitido())});
        };
        self.tipo_escrito(t, unit, tid, escopo)
    }

    fn tipo_escrito(&self, t: &mut Tabela, unit: UnitId, tid: ast::TypeId, escopo: &Escopo) -> Value {
        let u = self.program.unit(unit);
        let ty = u.ast.ty(tid);
        let lib = u.library;
        let mut v = match &ty.kind {
            TypeKind::Named { name, args } => {
                let c = self.chave_do_nome(lib, name, escopo);
                let nome = c.nome().to_string();
                let args: Vec<Value> = args.iter().map(|a| self.tipo_escrito(t, unit, *a, escopo)).collect();
                json!({"t": "nomeado", "ident": {"id": t.id(c), "nome": nome}, "args": args})
            }
            TypeKind::Void => json!({"t": "nomeado", "ident": {"id": t.id(Chave::Void), "nome": "void"}, "args": []}),
            TypeKind::Function { return_type, type_params, parameters } => {
                let retorno = match return_type {
                    Some(r) => self.tipo_escrito(t, unit, *r, escopo),
                    None => json!({"t": "nomeado", "ident": {"id": t.id(Chave::Dynamic), "nome": "dynamic"}, "args": []}),
                };
                let tparams: Vec<Value> = type_params.iter().map(|p| json!({"nome": self.n(p.name.sym)})).collect();
                let mut pos = Vec::new();
                let mut nom = Vec::new();
                for p in parameters.iter() {
                    let tipo = match p.ty {
                        Some(x) => self.tipo_escrito(t, unit, x, escopo),
                        None => json!({"t": "nomeado", "ident": {"id": t.id(Chave::Dynamic), "nome": "dynamic"}, "args": []}),
                    };
                    let j = json!({"nome": p.name.map(|n| self.n(n.sym)), "tipo": tipo,
                        "nomeado": p.kind == ParameterKind::Named,
                        "obrigatorio": p.kind == ParameterKind::Required || p.required});
                    if p.kind == ParameterKind::Named { nom.push(j) } else { pos.push(j) }
                }
                json!({"t": "funcao", "retorno": retorno, "posicionais": pos, "nomeados": nom, "tparams": tparams})
            }
            TypeKind::Record { positional, named } => {
                let pos: Vec<Value> = positional.iter().map(|x| json!({"tipo": self.tipo_escrito(t, unit, *x, escopo)})).collect();
                let nom: Vec<Value> =
                    named.iter().map(|(n, x)| json!({"nome": self.n(n.sym), "tipo": self.tipo_escrito(t, unit, *x, escopo)})).collect();
                json!({"t": "record", "posicionais": pos, "nomeados": nom})
            }
        };
        if ty.nullable {
            v["anulavel"] = json!(true);
        }
        v
    }

    // ------------------------------------------------------ declarações

    pub fn biblioteca_json(&self, t: &mut Tabela, lib: LibraryId) -> Value {
        let l = self.program.library(lib);
        let v = l.features.versao();
        json!({"k": "biblioteca", "id": t.biblioteca(&l.uri), "uri": l.uri, "versao": [v.major, v.minor]})
    }

    fn ident(&self, t: &mut Tabela, c: Chave) -> Value {
        let nome = c.nome().to_string();
        json!({"id": t.id(c), "nome": nome})
    }

    fn escopo_da_classe(&self, c: ClassId) -> (Chave, Escopo) {
        let dono = self.chave_da_classe(c);
        let escopo = self
            .program
            .class(c)
            .type_params
            .iter()
            .map(|p| (p.name, Chave::ParametroDeTipo { dono: Box::new(dono.clone()), nome: self.n(p.name) }))
            .collect();
        (dono, escopo)
    }

    /// A declaração de uma classe, mixin, enum ou extension type.
    pub fn classe_json(&self, t: &mut Tabela, c: ClassId) -> Value {
        let cl = self.program.class(c);
        let (chave, escopo) = self.escopo_da_classe(c);
        // A biblioteca vai inteira em cada declaração: a resposta de uma
        // consulta pode citar uma biblioteca que o executor ainda não viu.
        let lib = self.biblioteca_json(t, cl.library);
        let tparams: Vec<Value> = cl
            .type_params
            .iter()
            .map(|p| {
                let limite = p.bound.map(|(u, b)| self.tipo_escrito(t, u, b, &escopo));
                json!({"k": "tparam", "ident": self.ident(t, Chave::ParametroDeTipo { dono: Box::new(chave.clone()), nome: self.n(p.name) }),
                    "lib": lib, "limite": limite})
            })
            .collect();
        let tipos = |t: &mut Tabela, l: &[(UnitId, ast::TypeId)]| -> Vec<Value> {
            l.iter().map(|(u, x)| self.tipo_escrito(t, *u, *x, &escopo)).collect()
        };
        let ident = self.ident(t, chave.clone());
        let m = cl.modifiers;
        match cl.kind {
            ClassKind::Mixin => json!({"k": "mixin", "ident": ident, "lib": lib, "tparams": tparams, "base": m.base,
                "interfaces": tipos(t, &cl.interfaces), "restricoes": tipos(t, &cl.on)}),
            ClassKind::Enum => json!({"k": "enum", "ident": ident, "lib": lib, "tparams": tparams,
                "interfaces": tipos(t, &cl.interfaces), "mixins": tipos(t, &cl.mixins)}),
            ClassKind::ExtensionType => {
                let rep = cl.representation.and_then(|v| match self.program.variable(v).node {
                    VariableRef::Representation { unit, decl } => match &self.program.unit(unit).ast.decl(decl).kind {
                        DeclKind::ExtensionType(et) => Some(self.tipo_escrito(t, unit, et.representation_type, &escopo)),
                        _ => None,
                    },
                    _ => None,
                });
                json!({"k": "extensionType", "ident": ident, "lib": lib, "tparams": tparams, "representacao": rep})
            }
            _ => {
                let superclasse = cl.supertype.map(|(u, x)| self.tipo_escrito(t, u, x, &escopo));
                json!({"k": "classe", "ident": ident, "lib": lib, "tparams": tparams, "abstract": m.abstract_,
                    "base": m.base, "external": false, "final": m.final_, "interface": m.interface, "mixin": m.mixin,
                    "sealed": m.sealed, "superclasse": superclasse,
                    "interfaces": tipos(t, &cl.interfaces), "mixins": tipos(t, &cl.mixins)})
            }
        }
    }

    fn parametros(
        &self,
        t: &mut Tabela,
        unit: UnitId,
        dono: &Chave,
        ps: &[ast::Parameter],
        escopo: &Escopo,
    ) -> (Vec<Value>, Vec<Value>) {
        let lib = self.biblioteca_json(t, self.program.unit(unit).library);
        let mut pos = Vec::new();
        let mut nom = Vec::new();
        for p in ps {
            let Some(n) = p.name else { continue };
            let nome = self.n(n.sym);
            let tipo = self.tipo(t, unit, p.ty, escopo, || Omitido { dono: dono.clone(), lugar: nome.clone() });
            let estilo = if p.this_ { "this" } else if p.super_ { "super" } else { "normal" };
            let nomeado = p.kind == ParameterKind::Named;
            let j = json!({"k": "parametro",
                "ident": self.ident(t, Chave::Parametro { dono: Box::new(dono.clone()), nome: nome.clone() }),
                "lib": lib, "tipo": tipo, "nomeado": nomeado,
                "obrigatorio": if nomeado { p.required } else { p.kind == ParameterKind::Required },
                "estilo": estilo});
            if nomeado { nom.push(j) } else { pos.push(j) }
        }
        (pos, nom)
    }

    /// Os membros de `c` de um tipo (`campos`, `metodos`, `construtores`),
    /// na ordem de declaração da cadeia (introdutória e augmentations), sem as
    /// declarações `augment` (que não introduzem membro) — como o
    /// `fullMemberIterator` do CFE 3.6.2.
    pub fn membros_json(&self, t: &mut Tabela, c: ClassId, tipo: &str) -> Vec<Value> {
        let (chave_da_classe, escopo) = self.escopo_da_classe(c);
        let Chave::Tipo { lib: lib_uri, nome: dono } = chave_da_classe.clone() else { return Vec::new() };
        let lib = self.biblioteca_json(t, self.program.class(c).library);
        let definidor = self.ident(t, chave_da_classe);
        let mut out = Vec::new();
        for (unit, mid) in self.program.membros_da_classe(c) {
            let ast = &self.program.unit(unit).ast;
            let m = ast.member(mid);
            if m.augment {
                continue;
            }
            match (&m.kind, tipo) {
                (MemberKind::Field(v), "campos") => {
                    for var in v.variables.iter() {
                        let nome = self.n(var.name.sym);
                        let chave = Chave::Campo { lib: lib_uri.clone(), dono: dono.clone(), nome: nome.clone() };
                        let tipo = self.tipo(t, unit, v.ty, &escopo, || Omitido { dono: chave.clone(), lugar: "tipo".into() });
                        out.push(json!({"k": "campo", "ident": self.ident(t, chave), "lib": lib, "dono": definidor,
                            "static": v.static_, "abstract": v.abstract_, "const": v.const_, "external": v.external,
                            "final": v.final_, "late": v.late, "inicializador": var.initializer.is_some(), "tipo": tipo}));
                    }
                }
                (MemberKind::Method(fid), "metodos") => {
                    let f = ast.function(*fid);
                    let Some(n) = f.name else { continue };
                    let setter = f.kind == ast::FunctionKind::Setter;
                    let chave = Chave::Metodo { lib: lib_uri.clone(), dono: dono.clone(), nome: self.n(n.sym), setter };
                    let retorno = self.tipo(t, unit, f.return_type, &escopo, || Omitido { dono: chave.clone(), lugar: "retorno".into() });
                    let (pos, nom) = self.parametros(t, unit, &chave, f.parameters.as_deref().unwrap_or(&[]), &escopo);
                    out.push(json!({"k": "metodo", "ident": self.ident(t, chave), "lib": lib, "dono": definidor,
                        "static": f.static_, "corpo": true, "external": f.external,
                        "operador": f.kind == ast::FunctionKind::Operator, "getter": f.kind == ast::FunctionKind::Getter,
                        "setter": setter, "retorno": retorno, "posicionais": pos, "nomeados": nom, "tparams": []}));
                }
                (MemberKind::Constructor(k), "construtores") => {
                    let nome = k.name.map(|n| self.n(n.sym)).unwrap_or_default();
                    let chave = Chave::Construtor { lib: lib_uri.clone(), dono: dono.clone(), nome };
                    let retorno = json!({"t": "omitido", "chave": t.omitido(Omitido { dono: chave.clone(), lugar: "retorno".into() })});
                    let (pos, nom) = self.parametros(t, unit, &chave, &k.parameters, &escopo);
                    out.push(json!({"k": "construtor", "ident": self.ident(t, chave), "lib": lib, "dono": definidor,
                        "corpo": true, "external": k.external, "const": k.const_, "factory": k.factory,
                        "retorno": retorno, "posicionais": pos, "nomeados": nom, "tparams": []}));
                }
                _ => {}
            }
        }
        out
    }

    /// A declaração de uma chave, no programa corrente.
    pub fn declaracao_json(&self, t: &mut Tabela, c: &Chave) -> Option<Value> {
        match c {
            Chave::Tipo { .. } => self.classe(c).map(|id| self.classe_json(t, id)),
            Chave::FuncaoDeTopo { .. } => {
                let f = self.program.function(self.funcao(c)?);
                let FunctionRef::Function { unit, function } = f.node else { return None };
                let ast = self.program.unit(unit).ast.function(function);
                let lib = self.biblioteca_json(t, f.library);
                let escopo = Vec::new();
                let retorno = self.tipo(t, unit, ast.return_type, &escopo, || Omitido { dono: c.clone(), lugar: "retorno".into() });
                let (pos, nom) = self.parametros(t, unit, c, ast.parameters.as_deref().unwrap_or(&[]), &escopo);
                Some(json!({"k": "funcao", "ident": self.ident(t, c.clone()), "lib": lib,
                    "corpo": true, "external": f.external,
                    "operador": ast.kind == ast::FunctionKind::Operator,
                    "getter": ast.kind == ast::FunctionKind::Getter,
                    "setter": ast.kind == ast::FunctionKind::Setter,
                    "retorno": retorno, "posicionais": pos, "nomeados": nom, "tparams": []}))
            }
            Chave::Metodo { lib, dono, .. } | Chave::Construtor { lib, dono, .. } | Chave::Campo { lib, dono, .. } => {
                let cid = self.classe(&Chave::Tipo { lib: lib.clone(), nome: dono.clone() })?;
                let tipo = match c {
                    Chave::Metodo { .. } => "metodos",
                    Chave::Construtor { .. } => "construtores",
                    _ => "campos",
                };
                let id = t.id(c.clone());
                self.membros_json(t, cid, tipo).into_iter().find(|d| d["ident"]["id"].as_u64() == Some(id))
            }
            _ => None,
        }
    }

    /// As declarações de tipo de uma biblioteca (só classes, como o CFE).
    pub fn tipos_da_biblioteca(&self, t: &mut Tabela, lib: LibraryId) -> Vec<Value> {
        let mut out = Vec::new();
        for &u in &self.program.library(lib).units {
            let unit = self.program.unit(u);
            for &d in &unit.unit.declarations {
                let decl = unit.ast.decl(d);
                if decl.augment {
                    continue;
                }
                let nome = match &decl.kind {
                    DeclKind::Class(c) => c.name.sym,
                    DeclKind::Mixin(m) => m.name.sym,
                    _ => continue,
                };
                if let Some(Element::Class(c)) = self.program.library(lib).declared.get(&nome).and_then(|b| b.getter) {
                    out.push(self.classe_json(t, c));
                }
            }
        }
        out
    }
}

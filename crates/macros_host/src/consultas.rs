//! As consultas que o executor faz durante uma execução (`macro.consulta`),
//! servidas do programa corrente, e o [`Resolvedor`] da montagem.
//!
//! O comportamento segue o do CFE 3.6.2 (`kernel_macro_introspectors.dart`,
//! `identifiers.dart`), o oráculo:
//!
//! * `resolveIdentifier(uri, nome)` procura só os membros **locais** da
//!   biblioteca (`nome=` pede o setter);
//! * um identificador de tipo sai com a URI da biblioteca dele (prefixo na
//!   montagem); membro de instância sai sem URI (`this.`); membro estático e
//!   construtor saem como `prefixo.Classe.nome`; parâmetro, só o nome;
//! * `typesOf` só devolve classes; `topLevelDeclarationsOf` e `valuesOf` não
//!   existem no CFE 3.6.2 (erro de implementação aqui também).
//!
//! Onde o `crates/types` é necessário (`inferType`, subtipo genérico,
//! `asInstanceOf`) a consulta responde erro claro: é o pedido registrado ao
//! dono de `types`, não uma aproximação.
use crate::executor::{ErroDeConsulta, ServicoDeConsultas};
use crate::modelo::{Chave, Tabela, Vista};
use crate::montagem::{Codigo, IdentificadorResolvido, Parte, Resolvedor, TipoAumentado, TipoDeIdentificador};
use dartforge_elements::model::*;
use serde_json::{Value, json};

/// Um tipo resolvido (`StaticType`), comparável estruturalmente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TipoEstatico {
    Nomeado { chave: Chave, args: Vec<TipoEstatico>, anulavel: bool },
    /// Tipos de função e record: comparados pelo texto canônico.
    Estrutural { texto: String, anulavel: bool },
}

/// O serviço de consultas de uma execução.
pub struct Consultor<'a, 'p> {
    pub vista: &'a Vista<'p>,
    pub tabela: &'a mut Tabela,
    pub estaticos: &'a mut Vec<TipoEstatico>,
    /// O registro das consultas (tipo, argumentos) — a base do cache por
    /// digest (docs/MACROS-PROTOCOLO.md §6).
    pub registro: Vec<(String, Value)>,
}

fn u(args: &Value, k: &str) -> Result<u64, ErroDeConsulta> {
    args.get(k).and_then(Value::as_u64).ok_or_else(|| ErroDeConsulta::inesperado(format!("argumento '{k}' ausente")))
}

impl Consultor<'_, '_> {
    fn chave(&self, id: u64) -> Result<Chave, ErroDeConsulta> {
        self.tabela.chave(id).cloned().ok_or_else(|| ErroDeConsulta::inesperado(format!("identificador {id} desconhecido")))
    }

    fn resolver_identificador(&mut self, args: &Value) -> Result<Value, ErroDeConsulta> {
        let uri = args.get("uri").and_then(Value::as_str).unwrap_or("");
        let nome = args.get("nome").and_then(Value::as_str).unwrap_or("");
        let v = self.vista;
        let lib = v
            .biblioteca_por_uri(uri)
            .ok_or_else(|| ErroDeConsulta::implementacao(format!("Library at uri {uri} could not be resolved.")))?;
        let (membro, setter) = match nome.strip_suffix('=') {
            Some(m) => (m, true),
            None => (nome, false),
        };
        let nao_achado = || ErroDeConsulta::implementacao(format!("Unable to find top level identifier \"{nome}\" in {uri}"));
        let sym = v.interner.lookup(membro).ok_or_else(nao_achado)?;
        let b = v.program.library(lib).declared.get(&sym).copied().ok_or_else(nao_achado)?;
        let el = if setter { b.setter } else { b.getter }.ok_or_else(nao_achado)?;
        let lib_uri = uri.to_string();
        let chave = match el {
            Element::Class(_) => Chave::Tipo { lib: lib_uri, nome: membro.into() },
            Element::Typedef(_) => Chave::Typedef { lib: lib_uri, nome: membro.into() },
            Element::Extension(_) => Chave::Extension { lib: lib_uri, nome: membro.into() },
            Element::Function(_) => Chave::FuncaoDeTopo { lib: lib_uri, nome: membro.into(), setter },
            Element::Variable(_) => Chave::VariavelDeTopo { lib: lib_uri, nome: membro.into() },
            Element::Prefix(..) => return Err(nao_achado()),
        };
        Ok(json!({"id": self.tabela.id(chave), "nome": nome}))
    }

    /// O `StaticType` de uma anotação de tipo (o JSON do modelo).
    fn estatico(&self, t: &Value) -> Result<TipoEstatico, ErroDeConsulta> {
        let anulavel = t.get("anulavel").and_then(Value::as_bool).unwrap_or(false);
        match t.get("t").and_then(Value::as_str) {
            Some("nomeado") => {
                let id = t.get("ident").and_then(|i| i.get("id")).and_then(Value::as_u64).unwrap_or(0);
                let chave = self.chave(id)?;
                let args = t
                    .get("args")
                    .and_then(Value::as_array)
                    .map(|l| l.iter().map(|a| self.estatico(a)).collect::<Result<Vec<_>, _>>())
                    .transpose()?
                    .unwrap_or_default();
                Ok(TipoEstatico::Nomeado { chave, args, anulavel })
            }
            Some("funcao" | "record") => {
                let mut v = t.clone();
                v.as_object_mut().map(|o| o.remove("anulavel"));
                Ok(TipoEstatico::Estrutural { texto: v.to_string(), anulavel })
            }
            Some("omitido") => Err(ErroDeConsulta::implementacao("resolve de um tipo omitido: use inferType")),
            _ => Err(ErroDeConsulta::implementacao(
                "resolve só aceita NamedTypeAnnotationCode, FunctionTypeAnnotationCode e RecordTypeAnnotationCode",
            )),
        }
    }

    fn estatico_json(&mut self, e: TipoEstatico) -> Value {
        let (declaracao, args) = match &e {
            TipoEstatico::Nomeado { chave, args, .. } => {
                let d = self.vista.declaracao_json(self.tabela, chave);
                let a: Vec<Value> = args.clone().into_iter().map(|x| self.estatico_json(x)).collect();
                (d, a)
            }
            _ => (None, Vec::new()),
        };
        self.estaticos.push(e);
        json!({"chave": self.estaticos.len(), "declaracao": declaracao, "args": args})
    }

    fn estatico_por_chave(&self, args: &Value, k: &str) -> Result<TipoEstatico, ErroDeConsulta> {
        let n = u(args, k)? as usize;
        self.estaticos.get(n.wrapping_sub(1)).cloned().ok_or_else(|| ErroDeConsulta::inesperado(format!("tipo {n} desconhecido")))
    }

    /// Subtipo nominal sem argumentos de tipo (o que dá para decidir pelo
    /// `elements`); com argumentos, é da inferência (`crates/types`).
    fn eh_subtipo(&self, a: &TipoEstatico, b: &TipoEstatico) -> Result<bool, ErroDeConsulta> {
        if a == b {
            return Ok(true);
        }
        let (TipoEstatico::Nomeado { chave: ca, args: aa, anulavel: na }, TipoEstatico::Nomeado { chave: cb, args: ab, anulavel: nb }) = (a, b)
        else {
            return Err(ErroDeConsulta::inesperado("isSubtypeOf de tipos de função e record exige o crates/types"));
        };
        let topo = |c: &Chave| matches!(c, Chave::Dynamic) || matches!(c, Chave::Tipo { lib, nome } if lib == "dart:core" && nome == "Object");
        if matches!(cb, Chave::Dynamic) || (topo(cb) && (*nb || !*na)) {
            return Ok(true);
        }
        if *na && !*nb {
            return Ok(false);
        }
        if !aa.is_empty() || !ab.is_empty() {
            return Err(ErroDeConsulta::inesperado("isSubtypeOf com argumentos de tipo exige o crates/types"));
        }
        let (Some(x), Some(y)) = (self.vista.classe(ca), self.vista.classe(cb)) else { return Ok(false) };
        let mut pilha = vec![x];
        let mut vistos = std::collections::HashSet::new();
        while let Some(c) = pilha.pop() {
            if c == y {
                return Ok(true);
            }
            if !vistos.insert(c) {
                continue;
            }
            let cl = self.vista.program.class(c);
            pilha.extend(cl.supertype_class);
            pilha.extend(cl.interface_classes.iter().copied());
            pilha.extend(cl.mixin_classes.iter().copied());
            pilha.extend(cl.on_classes.iter().copied());
        }
        Ok(false)
    }
}

impl ServicoDeConsultas for Consultor<'_, '_> {
    fn consultar(&mut self, tipo: &str, args: &Value) -> Result<Value, ErroDeConsulta> {
        self.registro.push((tipo.to_string(), args.clone()));
        match tipo {
            "resolverIdentificador" => self.resolver_identificador(args),
            "declaracao" => {
                let c = self.chave(u(args, "ident")?)?;
                self.vista
                    .declaracao_json(self.tabela, &c)
                    .ok_or_else(|| ErroDeConsulta::implementacao(format!("Unable to resolve identifier {}", c.nome())))
            }
            "membros" => {
                let c = self.chave(u(args, "dono")?)?;
                let tipo = args.get("tipo").and_then(Value::as_str).unwrap_or("");
                if tipo == "valores" {
                    return Err(ErroDeConsulta::implementacao("valuesOf não existe no CFE 3.6.2"));
                }
                let cid = self
                    .vista
                    .classe(&c)
                    .ok_or_else(|| ErroDeConsulta::implementacao("Only introspection on classes is supported"))?;
                Ok(Value::Array(self.vista.membros_json(self.tabela, cid, tipo)))
            }
            "tiposDe" => {
                let id = u(args, "biblioteca")?;
                let uri = self.tabela.uri_da_biblioteca(id).map(str::to_string).unwrap_or_default();
                let lib = self
                    .vista
                    .biblioteca_por_uri(&uri)
                    .ok_or_else(|| ErroDeConsulta::implementacao(format!("Library at uri {uri} could not be resolved.")))?;
                Ok(Value::Array(self.vista.tipos_da_biblioteca(self.tabela, lib)))
            }
            "declaracoesDe" => Err(ErroDeConsulta::implementacao("topLevelDeclarationsOf não existe no CFE 3.6.2")),
            "resolver" => {
                let t = args.get("tipo").ok_or_else(|| ErroDeConsulta::inesperado("argumento 'tipo' ausente"))?;
                let e = self.estatico(t)?;
                Ok(self.estatico_json(e))
            }
            "ehExatamente" => {
                let (a, b) = (self.estatico_por_chave(args, "a")?, self.estatico_por_chave(args, "b")?);
                Ok(json!(a == b))
            }
            "ehSubtipo" => {
                let (a, b) = (self.estatico_por_chave(args, "a")?, self.estatico_por_chave(args, "b")?);
                self.eh_subtipo(&a, &b).map(|x| json!(x))
            }
            "comoInstanciaDe" => Err(ErroDeConsulta::inesperado("asInstanceOf exige o crates/types")),
            "inferirTipo" => Err(ErroDeConsulta::inesperado("inferType exige a inferência do crates/types")),
            _ => Err(ErroDeConsulta::inesperado(format!("consulta desconhecida: {tipo}"))),
        }
    }
}

// ----------------------------------------------------------- montagem

/// O [`Resolvedor`] da montagem sobre o programa corrente.
pub struct ResolvedorDoPrograma<'a, 'p> {
    pub vista: &'a Vista<'p>,
    pub tabela: &'a std::cell::RefCell<&'a mut Tabela>,
}

/// O código de uma anotação de tipo do modelo (como `TypeAnnotation.code`).
pub fn codigo_de_tipo(t: &Value) -> Codigo {
    let mut partes = Vec::new();
    match t.get("t").and_then(Value::as_str) {
        Some("nomeado") => {
            partes.push(Parte::Ident(t["ident"]["id"].as_u64().unwrap_or(0)));
            let args = t.get("args").and_then(Value::as_array).cloned().unwrap_or_default();
            if !args.is_empty() {
                partes.push(Parte::Texto("<".into()));
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        partes.push(Parte::Texto(", ".into()));
                    }
                    partes.push(Parte::Codigo(codigo_de_tipo(a)));
                }
                partes.push(Parte::Texto(">".into()));
            }
        }
        Some("omitido") => partes.push(Parte::Omitido(t["chave"].as_u64().unwrap_or(0))),
        _ => partes.push(Parte::Texto("dynamic".into())),
    }
    if t.get("anulavel").and_then(Value::as_bool) == Some(true) {
        partes.push(Parte::Texto("?".into()));
    }
    Codigo { tipo: "namedTypeAnnotation".into(), partes }
}

impl Resolvedor for ResolvedorDoPrograma<'_, '_> {
    fn identificador(&self, id: u64) -> Result<IdentificadorResolvido, String> {
        let chave = self.tabela.borrow().chave(id).cloned().ok_or_else(|| format!("identificador {id} desconhecido"))?;
        let nome = chave.nome().to_string();
        let topo = |uri: Option<String>| IdentificadorResolvido { nome: nome.clone(), tipo: TipoDeIdentificador::Topo, uri, escopo: None };
        Ok(match &chave {
            Chave::Tipo { lib, .. } | Chave::Typedef { lib, .. } | Chave::FuncaoDeTopo { lib, .. } | Chave::VariavelDeTopo { lib, .. } => {
                topo(Some(lib.clone()))
            }
            // O CFE 3.6.2 não dá URI a extension (TODO dele): sai o nome.
            Chave::Extension { .. } | Chave::ParametroDeTipo { .. } | Chave::Void | Chave::Solto(_) => topo(None),
            Chave::Dynamic => topo(Some("dart:core".into())),
            Chave::Parametro { .. } => IdentificadorResolvido { nome, tipo: TipoDeIdentificador::Local, uri: None, escopo: None },
            Chave::Construtor { lib, dono, .. } => {
                IdentificadorResolvido { nome, tipo: TipoDeIdentificador::Estatico, uri: Some(lib.clone()), escopo: Some(dono.clone()) }
            }
            Chave::Metodo { lib, dono, .. } | Chave::Campo { lib, dono, .. } => {
                let estatico = match &chave {
                    Chave::Metodo { .. } => self.vista.funcao(&chave).is_some_and(|f| self.vista.program.function(f).static_),
                    _ => self
                        .vista
                        .classe(&Chave::Tipo { lib: lib.clone(), nome: dono.clone() })
                        .and_then(|c| {
                            self.vista.program.class(c).fields.iter().copied().find(|v| {
                                self.vista.interner.resolve(self.vista.program.variable(*v).name) == nome
                            })
                        })
                        .is_some_and(|v| self.vista.program.variable(v).static_),
                };
                if estatico {
                    IdentificadorResolvido { nome, tipo: TipoDeIdentificador::Estatico, uri: Some(lib.clone()), escopo: Some(dono.clone()) }
                } else {
                    IdentificadorResolvido { nome, tipo: TipoDeIdentificador::Instancia, uri: None, escopo: None }
                }
            }
        })
    }

    fn tipo_aumentado(&self, id: u64) -> Result<TipoAumentado, String> {
        let chave = self.tabela.borrow().chave(id).cloned().ok_or_else(|| format!("identificador {id} desconhecido"))?;
        let c = self.vista.classe(&chave).ok_or_else(|| format!("Unsupported augmentation type {}", chave.nome()))?;
        let cl = self.vista.program.class(c);
        let m = cl.modifiers;
        let (palavra, modificadores) = match cl.kind {
            ClassKind::Class => (
                "class",
                [("abstract", m.abstract_), ("base", m.base), ("final", m.final_), ("interface", m.interface), ("mixin", m.mixin), ("sealed", m.sealed)]
                    .into_iter()
                    .filter_map(|(p, s)| s.then_some(p))
                    .collect(),
            ),
            ClassKind::Mixin => ("mixin", if m.base { vec!["base"] } else { vec![] }),
            ClassKind::Enum => ("enum", vec![]),
            _ => return Err(format!("Unsupported augmentation type {}", chave.nome())),
        };
        let json = {
            let mut t = self.tabela.borrow_mut();
            self.vista.classe_json(&mut t, c)
        };
        let parametros = json["tparams"]
            .as_array()
            .map(|l| {
                l.iter()
                    .map(|p| {
                        let mut partes = vec![Parte::Texto(p["ident"]["nome"].as_str().unwrap_or("").to_string())];
                        if !p["limite"].is_null() {
                            partes.push(Parte::Texto(" extends ".into()));
                            partes.push(Parte::Codigo(codigo_de_tipo(&p["limite"])));
                        }
                        Codigo { tipo: "typeParameter".into(), partes }
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(TipoAumentado { palavra, modificadores, nome: chave.nome().to_string(), parametros })
    }

    fn tipo_inferido(&self, _omitido: u64) -> Option<Codigo> {
        // `inferType` é da inferência do `crates/types` (pedido registrado);
        // sem ela, o texto que usa tipo omitido não é montado.
        None
    }
}

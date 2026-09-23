//! Leitura do `.dart_tool/build/entrypoint/build.dart` que o `build_runner`
//! gerou — o oráculo do plano. Ele é **lido, nunca gerado** (rodar o
//! `build_runner` no projeto do usuário não é permitido). Daqui sai a mesma
//! forma canônica de `Aplicacao::texto_canonico`, para comparar linha a linha.
//!
//! A gramática é a que o `code_builder` + `dart_style` produzem nesse
//! arquivo: chamadas com argumentos posicionais e nomeados, listas e mapas
//! com argumentos de tipo opcionais e `const`, textos (`r'…'` e comuns),
//! números, `true`/`false`/`null` e referências `_iN.nome`.
use crate::config::InputSet;
use crate::plano::{Aplicacao, Filtro};
use crate::valor::{Mapa, Valor};
use std::collections::HashMap;

#[derive(Debug, Clone)]
enum Expr {
    Chamada { alvo: String, pos: Vec<Expr>, nom: Vec<(String, Expr)> },
    Lista(Vec<Expr>),
    Mapa(Vec<(Expr, Expr)>),
    Texto(String),
    Num(String),
    Bool(bool),
    Nulo,
    Ref(String),
}

struct Lx<'a> {
    s: &'a [u8],
    i: usize,
}

impl Lx<'_> {
    fn espacos(&mut self) {
        loop {
            while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
                self.i += 1;
            }
            if self.s[self.i..].starts_with(b"//") {
                while self.i < self.s.len() && self.s[self.i] != b'\n' {
                    self.i += 1;
                }
                continue;
            }
            if self.s[self.i..].starts_with(b"/*") {
                while self.i + 1 < self.s.len() && !self.s[self.i..].starts_with(b"*/") {
                    self.i += 1;
                }
                self.i = (self.i + 2).min(self.s.len());
                continue;
            }
            break;
        }
    }
    fn olha(&mut self, t: &str) -> bool {
        self.espacos();
        self.s[self.i..].starts_with(t.as_bytes())
    }
    fn toma(&mut self, t: &str) -> bool {
        if self.olha(t) {
            self.i += t.len();
            true
        } else {
            false
        }
    }
    fn exige(&mut self, t: &str) -> Result<(), String> {
        if self.toma(t) {
            Ok(())
        } else {
            Err(format!("build.dart: esperava `{t}` na posição {}", self.i))
        }
    }
    fn ident(&mut self) -> Option<String> {
        self.espacos();
        let ini = self.i;
        while self.i < self.s.len() && (self.s[self.i].is_ascii_alphanumeric() || self.s[self.i] == b'_' || self.s[self.i] == b'$')
        {
            self.i += 1;
        }
        (self.i > ini).then(|| String::from_utf8_lossy(&self.s[ini..self.i]).to_string())
    }

    /// `<...>` de argumentos de tipo: ignorado.
    fn tipos(&mut self) {
        if self.olha("<") {
            let mut n = 0;
            while self.i < self.s.len() {
                match self.s[self.i] {
                    b'<' => n += 1,
                    b'>' => {
                        n -= 1;
                        if n == 0 {
                            self.i += 1;
                            return;
                        }
                    }
                    _ => {}
                }
                self.i += 1;
            }
        }
    }

    fn texto(&mut self) -> Result<Option<String>, String> {
        self.espacos();
        let cru = self.s[self.i..].starts_with(b"r'") || self.s[self.i..].starts_with(b"r\"");
        if cru {
            self.i += 1;
        }
        let Some(&q) = self.s.get(self.i) else { return Ok(None) };
        if q != b'\'' && q != b'"' {
            if cru {
                self.i -= 1;
            }
            return Ok(None);
        }
        self.i += 1;
        let mut v: Vec<u8> = Vec::new();
        while self.i < self.s.len() && self.s[self.i] != q {
            let c = self.s[self.i];
            if c == b'\\' && !cru {
                self.i += 1;
                let e = self.s.get(self.i).copied().unwrap_or(b'\\');
                v.push(match e {
                    b'n' => b'\n',
                    b't' => b'\t',
                    b'r' => b'\r',
                    x => x,
                });
            } else {
                v.push(c);
            }
            self.i += 1;
        }
        self.i += 1;
        let s = String::from_utf8(v).map_err(|e| e.to_string())?;
        // Textos adjacentes se concatenam.
        if let Some(mais) = self.texto()? {
            return Ok(Some(s + &mais));
        }
        Ok(Some(s))
    }

    fn expr(&mut self) -> Result<Expr, String> {
        self.toma("const ");
        if let Some(t) = self.texto()? {
            return Ok(Expr::Texto(t));
        }
        self.espacos();
        if self.olha("<") {
            self.tipos();
        }
        if self.toma("[") {
            let mut v = Vec::new();
            while !self.toma("]") {
                v.push(self.expr()?);
                if !self.toma(",") {
                    self.exige("]")?;
                    break;
                }
            }
            return Ok(Expr::Lista(v));
        }
        if self.toma("{") {
            let mut v = Vec::new();
            while !self.toma("}") {
                let k = self.expr()?;
                self.exige(":")?;
                let val = self.expr()?;
                v.push((k, val));
                if !self.toma(",") {
                    self.exige("}")?;
                    break;
                }
            }
            return Ok(Expr::Mapa(v));
        }
        self.espacos();
        let c = self.s.get(self.i).copied().unwrap_or(0);
        if c.is_ascii_digit() || c == b'-' {
            let ini = self.i;
            self.i += 1;
            while self.i < self.s.len()
                && (self.s[self.i].is_ascii_alphanumeric() || matches!(self.s[self.i], b'.' | b'+' | b'-'))
            {
                self.i += 1;
            }
            return Ok(Expr::Num(String::from_utf8_lossy(&self.s[ini..self.i]).to_string()));
        }
        let mut nome = self.ident().ok_or_else(|| format!("build.dart: expressão inesperada na posição {}", self.i))?;
        match nome.as_str() {
            "true" => return Ok(Expr::Bool(true)),
            "false" => return Ok(Expr::Bool(false)),
            "null" => return Ok(Expr::Nulo),
            _ => {}
        }
        while self.toma(".") {
            nome.push('.');
            nome.push_str(&self.ident().ok_or("build.dart: nome esperado")?);
        }
        self.tipos();
        if !self.toma("(") {
            return Ok(Expr::Ref(nome));
        }
        let (mut pos, mut nom) = (Vec::new(), Vec::new());
        while !self.toma(")") {
            let volta = self.i;
            let nomeado = self.ident().filter(|_| self.toma(":") && !self.olha(":"));
            match nomeado {
                Some(n) => nom.push((n, self.expr()?)),
                None => {
                    self.i = volta;
                    pos.push(self.expr()?);
                }
            }
            if !self.toma(",") {
                self.exige(")")?;
                break;
            }
        }
        Ok(Expr::Chamada { alvo: nome, pos, nom })
    }
}

fn valor(e: &Expr) -> Result<Valor, String> {
    Ok(match e {
        Expr::Texto(t) => Valor::Texto(t.clone()),
        Expr::Num(n) => match n.parse::<i64>() {
            Ok(i) if !n.contains('.') && !n.contains('e') => Valor::Int(i),
            _ => Valor::Real(n.clone()),
        },
        Expr::Bool(b) => Valor::Bool(*b),
        Expr::Nulo => Valor::Nulo,
        Expr::Lista(l) => Valor::Lista(l.iter().map(valor).collect::<Result<_, _>>()?),
        Expr::Mapa(m) => {
            let mut r = Mapa::default();
            for (k, v) in m {
                r.inserir(valor(k)?, valor(v)?);
            }
            Valor::Mapa(r)
        }
        outro => return Err(format!("build.dart: valor de opção inesperado: {outro:?}")),
    })
}

fn textos(e: &Expr) -> Result<Vec<String>, String> {
    match e {
        Expr::Lista(l) => l
            .iter()
            .map(|x| match x {
                Expr::Texto(t) => Ok(t.clone()),
                o => Err(format!("build.dart: esperava texto, veio {o:?}")),
            })
            .collect(),
        o => Err(format!("build.dart: esperava lista, veio {o:?}")),
    }
}

fn opcoes(e: &Expr) -> Result<Mapa, String> {
    match e {
        Expr::Chamada { pos, .. } if pos.len() == 1 => match valor(&pos[0])? {
            Valor::Mapa(m) => Ok(m),
            o => Err(format!("build.dart: BuilderOptions sem mapa: {o:?}")),
        },
        o => Err(format!("build.dart: esperava BuilderOptions(...), veio {o:?}")),
    }
}

fn metodo(alvo: &str) -> &str {
    alvo.rsplit('.').next().unwrap_or(alvo)
}

/// As aplicações do `build.dart`, na ordem, como `Aplicacao` (os campos que
/// o script não traz ficam vazios).
pub fn ler_build_dart(texto: &str) -> Result<Vec<Aplicacao>, String> {
    let mut lx = Lx { s: texto.as_bytes(), i: 0 };
    // Imports: `import 'uri' as _iN;`
    let mut prefixos: HashMap<String, String> = HashMap::new();
    loop {
        lx.espacos();
        if !lx.toma("import") {
            break;
        }
        let uri = lx.texto()?.ok_or("build.dart: import sem URI")?;
        if lx.toma("as") {
            let p = lx.ident().ok_or("build.dart: import sem prefixo")?;
            prefixos.insert(p, uri);
        }
        lx.exige(";")?;
    }
    lx.exige("final")?;
    lx.ident();
    lx.exige("=")?;
    let Expr::Lista(itens) = lx.expr()? else {
        return Err("build.dart: `_builders` não é uma lista".into());
    };
    let resolver = |r: &str| -> String {
        match r.split_once('.') {
            Some((p, n)) => format!("{}#{n}", prefixos.get(p).cloned().unwrap_or_else(|| p.to_string())),
            None => r.to_string(),
        }
    };
    let mut v = Vec::new();
    for it in itens {
        let Expr::Chamada { alvo, pos, nom } = it else {
            return Err("build.dart: item de `_builders` não é chamada".into());
        };
        let e_pos = match metodo(&alvo) {
            "apply" => false,
            "applyPostProcess" => true,
            o => return Err(format!("build.dart: chamada inesperada `{o}`")),
        };
        let chave = match pos.first() {
            Some(Expr::Texto(t)) => t.clone(),
            _ => return Err("build.dart: aplicação sem chave".into()),
        };
        let refs: Vec<String> = match pos.get(1) {
            Some(Expr::Lista(l)) => l
                .iter()
                .map(|x| match x {
                    Expr::Ref(r) => Ok(resolver(r)),
                    o => Err(format!("build.dart: fábrica inesperada {o:?}")),
                })
                .collect::<Result<_, _>>()?,
            Some(Expr::Ref(r)) => vec![resolver(r)],
            _ => return Err(format!("build.dart: `{chave}` sem fábricas")),
        };
        let import = refs.first().and_then(|r| r.split_once('#')).map(|(i, _)| i.to_string()).unwrap_or_default();
        let fabricas: Vec<String> = refs.iter().map(|r| r.split_once('#').map(|(_, n)| n).unwrap_or(r).to_string()).collect();
        let filtro = if e_pos {
            Filtro::Nenhum
        } else {
            match pos.get(2) {
                Some(Expr::Chamada { alvo, pos, .. }) => match metodo(alvo) {
                    "toNoneByDefault" => Filtro::Nenhum,
                    "toAllPackages" => Filtro::Todos,
                    "toRoot" => Filtro::Raiz,
                    "toDependentsOf" => match pos.first() {
                        Some(Expr::Texto(p)) => Filtro::DependentesDe(p.clone()),
                        _ => return Err("build.dart: toDependentsOf sem pacote".into()),
                    },
                    o => return Err(format!("build.dart: filtro inesperado `{o}`")),
                },
                _ => return Err(format!("build.dart: `{chave}` sem filtro")),
            }
        };
        let mut a = Aplicacao {
            chave,
            pos: e_pos,
            import,
            fabricas,
            filtro,
            opcional: false,
            oculta: e_pos,
            generate_for_padrao: None,
            opcoes_padrao: Mapa::default(),
            opcoes_dev: Mapa::default(),
            opcoes_release: Mapa::default(),
            aplica: Vec::new(),
            pacote: String::new(),
            extensoes_declaradas: Vec::new(),
        };
        for (n, e) in &nom {
            match n.as_str() {
                "isOptional" => a.opcional = matches!(e, Expr::Bool(true)),
                "hideOutput" => a.oculta = matches!(e, Expr::Bool(true)),
                "defaultGenerateFor" => {
                    let Expr::Chamada { nom, .. } = e else {
                        return Err("build.dart: defaultGenerateFor não é InputSet".into());
                    };
                    let mut s = InputSet::default();
                    for (k, v) in nom {
                        match k.as_str() {
                            "include" => s.include = Some(textos(v)?),
                            "exclude" => s.exclude = Some(textos(v)?),
                            o => return Err(format!("build.dart: InputSet.{o}")),
                        }
                    }
                    a.generate_for_padrao = Some(s);
                }
                "defaultOptions" => a.opcoes_padrao = opcoes(e)?,
                "defaultDevOptions" => a.opcoes_dev = opcoes(e)?,
                "defaultReleaseOptions" => a.opcoes_release = opcoes(e)?,
                "appliesBuilders" => a.aplica = textos(e)?,
                o => return Err(format!("build.dart: argumento nomeado inesperado `{o}`")),
            }
        }
        v.push(a);
    }
    Ok(v)
}

/// Compara o plano com o `build.dart`: `Ok(())` se iguais, senão a lista
/// das diferenças por linha (`-` oráculo, `+` motor).
pub fn comparar(plano: &[Aplicacao], build_dart: &str) -> Result<Result<(), Vec<String>>, String> {
    let oraculo: Vec<String> = ler_build_dart(build_dart)?.iter().map(Aplicacao::texto_canonico).collect();
    let nosso: Vec<String> = plano.iter().map(Aplicacao::texto_canonico).collect();
    if oraculo == nosso {
        return Ok(Ok(()));
    }
    let mut dif = Vec::new();
    for i in 0..oraculo.len().max(nosso.len()) {
        match (oraculo.get(i), nosso.get(i)) {
            (Some(a), Some(b)) if a == b => {}
            (a, b) => {
                if let Some(a) = a {
                    dif.push(format!("- [{i}] {a}"));
                }
                if let Some(b) = b {
                    dif.push(format!("+ [{i}] {b}"));
                }
            }
        }
    }
    Ok(Err(dif))
}

#[cfg(test)]
mod testes {
    use super::*;

    const TRECHO: &str = r#"// @dart=3.6
// ignore_for_file: directives_ordering
import 'package:build_runner_core/build_runner_core.dart' as _i1;
import 'package:source_gen/builder.dart' as _i2;
import 'package:sass_builder/sass_builder.dart' as _i3;
import 'package:build/build.dart' as _i4;
import 'package:build_config/build_config.dart' as _i8;
import 'dart:isolate' as _i11;

final _builders = <_i1.BuilderApplication>[
  _i1.apply(
    r'source_gen:combining_builder',
    [_i2.combiningBuilder],
    _i1.toNoneByDefault(),
    hideOutput: false,
    appliesBuilders: const [r'source_gen:part_cleanup'],
  ),
  _i1.apply(
    r'sass_builder:sass_builder',
    [_i3.sassBuilder],
    _i1.toDependentsOf(r'sass_builder'),
    hideOutput: true,
    defaultDevOptions:
        const _i4.BuilderOptions(<String, dynamic>{r'sourceMaps': true}),
    defaultReleaseOptions: const _i4.BuilderOptions(<String, dynamic>{
      r'outputStyle': r'compressed',
      r'sourceMaps': false,
    }),
    appliesBuilders: const [r'sass_builder:sass_source_cleanup'],
  ),
  _i1.applyPostProcess(
    r'source_gen:part_cleanup',
    _i2.partCleanup,
    defaultGenerateFor: const _i8.InputSet(include: [r'lib/**']),
  ),
];
void main(
  List<String> args, [
  _i11.SendPort? sendPort,
]) async {}
"#;

    #[test]
    fn le_o_build_dart() {
        let v = ler_build_dart(TRECHO).unwrap();
        assert_eq!(v.len(), 3);
        assert_eq!(
            v[1].texto_canonico(),
            r#"apply "sass_builder:sass_builder" ["package:sass_builder/sass_builder.dart#sassBuilder"] toDependentsOf("sass_builder") isOptional=false hideOutput=true defaultGenerateFor=- defaultOptions={} defaultDevOptions={"sourceMaps": true} defaultReleaseOptions={"outputStyle": "compressed", "sourceMaps": false} appliesBuilders=["sass_builder:sass_source_cleanup"]"#
        );
        assert_eq!(
            v[2].texto_canonico(),
            r#"applyPostProcess "source_gen:part_cleanup" ["package:source_gen/builder.dart#partCleanup"] defaultGenerateFor=InputSet(include: ["lib/**"], exclude: -) defaultOptions={} defaultDevOptions={} defaultReleaseOptions={}"#
        );
    }
}

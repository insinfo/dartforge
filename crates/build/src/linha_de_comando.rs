//! As opções de linha de comando do `build_runner` que mudam o plano ou as
//! saídas: `--define`, `--config` e `--build-filter`. Porte de
//! `entrypoint/options.dart` (2.4.15) e `build_plan/build_options.dart`
//! (2.16.1) — o `--define` e o `--config` são iguais nas duas; o
//! `--build-filter` ganhou o esquema `asset:` no 2.14.0.
//!
//! Precedência das opções de um builder (`createBuildPhases`): `defaults` <
//! `dev_options`/`release_options` dos defaults < `options` do alvo <
//! `dev_options`/`release_options` do alvo < `global_options` (com o
//! `dev`/`release` dele) < `--define`, este fundido por chave sobre o global.
use crate::config::chave_builder_uso;
use crate::glob::Glob;
use crate::grafo::AssetId;
use crate::valor::{Mapa, Valor, real_dart};

/// Os `--define` já agrupados por builder (chave normalizada), na ordem em
/// que apareceram.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Definicoes(pub Vec<(String, Mapa)>);

impl Definicoes {
    /// `_parseBuilderConfigOverrides`: cada argumento é
    /// `<builder>=<opção>=<valor>`; o resto depois do segundo `=` é o valor,
    /// lido como JSON e, se não for JSON, como texto. A chave do builder é
    /// normalizada com o pacote raiz (`:x` → `raiz:x`, `x` → `x:x`).
    ///
    /// ```
    /// use dartforge_build::linha_de_comando::Definicoes;
    /// let d = Definicoes::ler(&[":b=n=1", "p:b=t=a=b", "p:b=l=[1, 2.0]"], "p").unwrap();
    /// assert_eq!(d.0[0].0, "p:b");
    /// assert_eq!(d.0[0].1.texto_canonico(), r#"{"n": 1, "t": "a=b", "l": [1, 2.0]}"#);
    /// assert!(Definicoes::ler(&["b=n"], "p").is_err());
    /// assert!(Definicoes::ler(&["b=n=1", "b=n=2"], "p").is_err());
    /// ```
    ///
    /// # Erros
    /// Menos de dois `=`, ou a mesma opção do mesmo builder duas vezes — as
    /// mensagens do `build_runner`.
    pub fn ler<S: AsRef<str>>(argumentos: &[S], raiz: &str) -> Result<Definicoes, String> {
        let mut d = Definicoes::default();
        for a in argumentos {
            let a = a.as_ref();
            let mut partes = a.splitn(3, '=');
            let (Some(builder), Some(opcao), Some(valor)) =
                (partes.next(), partes.next(), partes.next())
            else {
                return Err(format!(
                    "Invalid argument (define): Expected at least 2 `=` signs, should be of the format like --define \"<builder_key>=<option>=<value>\": \"{a}\""
                ));
            };
            let chave = chave_builder_uso(builder, raiz);
            let valor = json(valor).unwrap_or_else(|| Valor::Texto(valor.to_string()));
            let i = match d.0.iter().position(|(k, _)| *k == chave) {
                Some(i) => i,
                None => {
                    d.0.push((chave.clone(), Mapa::default()));
                    d.0.len() - 1
                }
            };
            let m = &mut d.0[i].1;
            if m.obter(opcao).is_some() {
                return Err(format!(
                    "Invalid argument(s): Got duplicate overrides for the same builder option: {chave}={opcao}. Only one is allowed."
                ));
            }
            m.inserir(Valor::Texto(opcao.to_string()), valor);
        }
        Ok(d)
    }

    /// As opções definidas para o builder `chave`.
    pub fn de(&self, chave: &str) -> Option<&Mapa> {
        self.0.iter().find(|(k, _)| k == chave).map(|(_, m)| m)
    }
}

/// `json.decode` do Dart: `None` se o texto não é JSON. Números com `.` ou
/// expoente são `double`, os outros `int` (ou `double`, se não cabem em 64
/// bits); objetos mantêm a ordem das chaves (a última repetida vence).
///
/// ```
/// use dartforge_build::linha_de_comando::json;
/// use dartforge_build::valor::Valor;
/// assert_eq!(json("1"), Some(Valor::Int(1)));
/// assert_eq!(json("1e3"), Some(Valor::Real("1000.0".into())));
/// assert_eq!(json(" \"a\\u00e9\" "), Some(Valor::Texto("aé".into())));
/// assert_eq!(json("{a: 1}"), None);
/// assert_eq!(json("~"), None);
/// ```
pub fn json(texto: &str) -> Option<Valor> {
    let b = texto.as_bytes();
    let mut i = 0;
    let v = valor_json(b, &mut i)?;
    espacos(b, &mut i);
    (i == b.len()).then_some(v)
}

fn espacos(b: &[u8], i: &mut usize) {
    while *i < b.len() && matches!(b[*i], b' ' | b'\t' | b'\n' | b'\r') {
        *i += 1;
    }
}

fn valor_json(b: &[u8], i: &mut usize) -> Option<Valor> {
    espacos(b, i);
    match *b.get(*i)? {
        b'{' => {
            *i += 1;
            let mut m = Mapa::default();
            espacos(b, i);
            if b.get(*i) == Some(&b'}') {
                *i += 1;
                return Some(Valor::Mapa(m));
            }
            loop {
                espacos(b, i);
                let Valor::Texto(k) = texto_json(b, i)? else {
                    return None;
                };
                espacos(b, i);
                if b.get(*i) != Some(&b':') {
                    return None;
                }
                *i += 1;
                let v = valor_json(b, i)?;
                m.inserir(Valor::Texto(k), v);
                espacos(b, i);
                match b.get(*i)? {
                    b',' => *i += 1,
                    b'}' => {
                        *i += 1;
                        return Some(Valor::Mapa(m));
                    }
                    _ => return None,
                }
            }
        }
        b'[' => {
            *i += 1;
            let mut l = Vec::new();
            espacos(b, i);
            if b.get(*i) == Some(&b']') {
                *i += 1;
                return Some(Valor::Lista(l));
            }
            loop {
                l.push(valor_json(b, i)?);
                espacos(b, i);
                match b.get(*i)? {
                    b',' => *i += 1,
                    b']' => {
                        *i += 1;
                        return Some(Valor::Lista(l));
                    }
                    _ => return None,
                }
            }
        }
        b'"' => texto_json(b, i),
        b't' => literal(b, i, "true", Valor::Bool(true)),
        b'f' => literal(b, i, "false", Valor::Bool(false)),
        b'n' => literal(b, i, "null", Valor::Nulo),
        b'-' | b'0'..=b'9' => numero_json(b, i),
        _ => None,
    }
}

fn literal(b: &[u8], i: &mut usize, t: &str, v: Valor) -> Option<Valor> {
    if b[*i..].starts_with(t.as_bytes()) {
        *i += t.len();
        Some(v)
    } else {
        None
    }
}

fn numero_json(b: &[u8], i: &mut usize) -> Option<Valor> {
    let inicio = *i;
    if b[*i] == b'-' {
        *i += 1;
    }
    let digitos = |i: &mut usize| {
        let d = *i;
        while *i < b.len() && b[*i].is_ascii_digit() {
            *i += 1;
        }
        *i > d
    };
    // Inteiro: `0` ou um dígito não nulo seguido de dígitos.
    if b.get(*i) == Some(&b'0') {
        *i += 1;
    } else if !digitos(i) {
        return None;
    }
    let mut real = false;
    if b.get(*i) == Some(&b'.') {
        *i += 1;
        real = true;
        if !digitos(i) {
            return None;
        }
    }
    if matches!(b.get(*i), Some(b'e' | b'E')) {
        *i += 1;
        real = true;
        if matches!(b.get(*i), Some(b'+' | b'-')) {
            *i += 1;
        }
        if !digitos(i) {
            return None;
        }
    }
    let t = std::str::from_utf8(&b[inicio..*i]).ok()?;
    if !real && let Ok(n) = t.parse::<i64>() {
        return Some(Valor::Int(n));
    }
    t.parse::<f64>().ok().map(|v| Valor::Real(real_dart(v)))
}

fn texto_json(b: &[u8], i: &mut usize) -> Option<Valor> {
    if b.get(*i) != Some(&b'"') {
        return None;
    }
    *i += 1;
    let mut unidades: Vec<u16> = Vec::new();
    loop {
        let c = *b.get(*i)?;
        match c {
            b'"' => {
                *i += 1;
                return Some(Valor::Texto(String::from_utf16_lossy(&unidades)));
            }
            b'\\' => {
                *i += 1;
                let e = *b.get(*i)?;
                *i += 1;
                let u = match e {
                    b'"' => '"' as u16,
                    b'\\' => '\\' as u16,
                    b'/' => '/' as u16,
                    b'b' => 8,
                    b'f' => 12,
                    b'n' => 10,
                    b'r' => 13,
                    b't' => 9,
                    b'u' => {
                        let h = std::str::from_utf8(b.get(*i..*i + 4)?).ok()?;
                        *i += 4;
                        u16::from_str_radix(h, 16).ok()?
                    }
                    _ => return None,
                };
                unidades.push(u);
            }
            0..=0x1f => return None,
            _ => {
                // Um caractere UTF-8 inteiro.
                let resto = std::str::from_utf8(&b[*i..]).ok().or_else(|| {
                    let fim = (*i + 4).min(b.len());
                    (*i + 1..=fim)
                        .rev()
                        .find_map(|f| std::str::from_utf8(&b[*i..f]).ok())
                })?;
                let ch = resto.chars().next()?;
                let mut buf = [0u16; 2];
                unidades.extend_from_slice(ch.encode_utf16(&mut buf));
                *i += ch.len_utf8();
            }
        }
    }
}

/// As opções `--config`, `--define` e `--build-filter` de um projeto, lidas
/// contra o `pubspec.yaml` (o nome do pacote raiz normaliza as chaves e os
/// caminhos relativos) e o `pubspec.lock` (o perfil decide se `asset:` vale).
#[derive(Debug, Clone, Default)]
pub struct OpcoesDoBuild {
    pub config: Option<String>,
    pub definicoes: Definicoes,
    pub filtros: Vec<FiltroBuild>,
}

impl OpcoesDoBuild {
    /// Lê as opções para o projeto em `raiz`.
    ///
    /// # Erros
    /// `pubspec.yaml`/`pubspec.lock` ilegíveis, `--define` malformado ou
    /// `--build-filter` inválido.
    pub fn do_projeto<S: AsRef<str>>(
        raiz: &std::path::Path,
        config: Option<&str>,
        defines: &[S],
        filtros: &[S],
    ) -> Result<OpcoesDoBuild, String> {
        let pubspec = crate::config::Pubspec::ler(raiz)?;
        let nome = pubspec
            .nome
            .ok_or("The current package has no name, please add one to the pubspec.yaml.")?;
        let perfil = match std::fs::read_to_string(raiz.join("pubspec.lock")) {
            Ok(t) => {
                let lock = crate::config::ler_lock(&t, "pubspec.lock")?;
                let v = |p: &str| {
                    lock.iter()
                        .find(|(n, _)| n == p)
                        .map(|(_, t)| t.versao.as_str())
                };
                crate::perfil::Perfil::das_versoes(v("build_config"), v("build_runner"))
            }
            Err(_) => crate::perfil::Perfil::default(),
        };
        Ok(OpcoesDoBuild {
            config: config.map(str::to_string),
            definicoes: Definicoes::ler(defines, &nome)?,
            filtros: filtros
                .iter()
                .map(|f| FiltroBuild::do_argumento(f.as_ref(), &nome, perfil.filtro_asset()))
                .collect::<Result<_, _>>()?,
        })
    }

    /// Aplica às opções do motor.
    pub fn aplicar(self, opcoes: &mut crate::motor::OpcoesMotor) {
        opcoes.config = self.config;
        opcoes.definicoes = self.definicoes;
        opcoes.filtros = self.filtros;
    }
}

/// `BuildFilter`: um glob de pacote e um de caminho.
#[derive(Debug, Clone)]
pub struct FiltroBuild {
    pub pacote: Glob,
    pub caminho: Glob,
}

impl FiltroBuild {
    /// `BuildFilter.fromArg`: caminho relativo (do pacote raiz), `package:`
    /// (sob `lib/`) e, com `asset` (`build_runner` ≥ 2.14.0), `asset:` (o
    /// pacote inteiro).
    ///
    /// ```
    /// use dartforge_build::grafo::AssetId;
    /// use dartforge_build::linha_de_comando::FiltroBuild;
    /// let f = FiltroBuild::do_argumento("package:a/*.dart", "raiz", false).unwrap();
    /// assert!(f.casa(&AssetId::novo("a", "lib/x.dart")));
    /// let r = FiltroBuild::do_argumento("web/**", "raiz", false).unwrap();
    /// assert!(r.casa(&AssetId::novo("raiz", "web/a/b.js")));
    /// assert!(FiltroBuild::do_argumento("asset:a/web/x", "raiz", false).is_err());
    /// assert!(FiltroBuild::do_argumento("asset:a/web/x", "raiz", true).is_ok());
    /// ```
    ///
    /// # Erros
    /// Esquema não suportado ou glob inválido, com a mensagem do
    /// `build_runner`.
    pub fn do_argumento(arg: &str, raiz: &str, asset: bool) -> Result<FiltroBuild, String> {
        let erro = |e: String| {
            format!(
                "Invalid argument (--build-filter): Not a valid build filter, must be either a relative path or `package:` uri.\n\n{e}: \"{arg}\""
            )
        };
        // `Uri.parse`: esquema = letras, dígitos, `+`, `-`, `.` antes do
        // primeiro `:`, começando por letra.
        let esquema = arg.split_once(':').and_then(|(e, _)| {
            (!e.is_empty()
                && e.as_bytes()[0].is_ascii_alphabetic()
                && e.bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"+-.".contains(&c)))
            .then_some(e)
        });
        // O caminho da URI, sem consulta nem fragmento.
        let caminho_da_uri = |resto: &str| -> String {
            let fim = resto.find(['?', '#']).unwrap_or(resto.len());
            resto[..fim].to_string()
        };
        let (pacote, caminho) = match esquema.map(|e| e.to_ascii_lowercase()).as_deref() {
            Some("package") => {
                let p = desescapar(&caminho_da_uri(&arg["package:".len()..]));
                let mut seg = p.split('/');
                let pacote = seg.next().unwrap_or_default().to_string();
                let resto: Vec<&str> = seg.collect();
                let mut c = String::from("lib");
                for s in resto {
                    c.push('/');
                    c.push_str(s);
                }
                (pacote, c)
            }
            Some("asset") if asset => {
                let p = desescapar(&caminho_da_uri(&arg["asset:".len()..]));
                let (pacote, resto) = p.split_once('/').unwrap_or((p.as_str(), ""));
                (pacote.to_string(), resto.to_string())
            }
            Some(e) => return Err(erro(format!("Unsupported scheme {e}"))),
            None => (raiz.to_string(), caminho_escapado(&caminho_da_uri(arg))),
        };
        Ok(FiltroBuild {
            pacote: Glob::novo(&pacote).map_err(erro)?,
            caminho: Glob::novo(&caminho).map_err(erro)?,
        })
    }

    /// `BuildFilter.matches`.
    pub fn casa(&self, id: &AssetId) -> bool {
        self.pacote.casa(&id.pacote) && self.caminho.casa(&id.caminho)
    }
}

/// Os segmentos de caminho de uma URI com os escapes `%XX` desfeitos
/// (`Uri.pathSegments`, juntados por `/`).
fn desescapar(s: &str) -> String {
    let b = s.as_bytes();
    let mut v = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && b[i + 1].is_ascii_hexdigit()
            && b[i + 2].is_ascii_hexdigit()
        {
            let h = |c: u8| (c as char).to_digit(16).unwrap_or(0) as u8;
            v.push(h(b[i + 1]) * 16 + h(b[i + 2]));
            i += 3;
        } else {
            v.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&v).into_owned()
}

/// `Uri.path` de uma referência relativa: o `Uri.parse` do Dart escapa o que
/// não pode ficar num caminho (espaço, `{`, `}`, `[`, `]`, não ASCII…) e
/// normaliza os `%XX` existentes (maiúsculas; o de um caractere não
/// reservado vira o próprio caractere). O glob do filtro é feito desse texto,
/// então `{a,b}` num caminho relativo não é alternativa, como no oficial.
fn caminho_escapado(s: &str) -> String {
    let b = s.as_bytes();
    let permitido = |c: u8| c.is_ascii_alphanumeric() || b"-._~!$&'()*+,;=:@/".contains(&c);
    let mut o = String::with_capacity(s.len());
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == b'%'
            && i + 2 < b.len()
            && b[i + 1].is_ascii_hexdigit()
            && b[i + 2].is_ascii_hexdigit()
        {
            let h = |c: u8| (c as char).to_digit(16).unwrap_or(0) as u8;
            let x = h(b[i + 1]) * 16 + h(b[i + 2]);
            if x.is_ascii_alphanumeric() || b"-._~".contains(&x) {
                o.push(x as char);
            } else {
                o.push_str(&format!("%{:02X}", x));
            }
            i += 3;
        } else if permitido(c) {
            o.push(c as char);
            i += 1;
        } else {
            o.push_str(&format!("%{c:02X}"));
            i += 1;
        }
    }
    o
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn json_como_o_dart() {
        assert_eq!(json("true"), Some(Valor::Bool(true)));
        assert_eq!(json("null"), Some(Valor::Nulo));
        assert_eq!(json("-0.5"), Some(Valor::Real("-0.5".into())));
        assert_eq!(json("2.0"), Some(Valor::Real("2.0".into())));
        assert_eq!(json("01"), None);
        assert_eq!(json("[1,"), None);
        assert_eq!(json("'x'"), None);
        assert_eq!(
            json(r#"{"b": 1, "a": null, "b": 2}"#).map(|v| v.texto_canonico()),
            Some(r#"{"b": 2, "a": null}"#.into())
        );
        assert_eq!(json(r#""😀""#), Some(Valor::Texto("😀".into())));
        assert_eq!(json("\"três\""), Some(Valor::Texto("três".into())));
    }

    #[test]
    fn define_normaliza_e_mantem_o_resto() {
        let d = Definicoes::ler(&["x=a=1", "q|y=b=texto sem json"], "raiz").unwrap();
        assert_eq!(d.0[0].0, "x:x");
        assert_eq!(d.0[1].0, "q:y");
        assert_eq!(
            d.de("q:y").unwrap().texto_canonico(),
            r#"{"b": "texto sem json"}"#
        );
    }

    #[test]
    fn filtro_com_escape_e_glob() {
        // Relativo: o caminho é o `Uri.path`, escapado.
        let f = FiltroBuild::do_argumento("lib/a b.*", "r", false).unwrap();
        assert!(f.casa(&AssetId::novo("r", "lib/a%20b.txt")));
        assert!(!f.casa(&AssetId::novo("r", "lib/a b.txt")));
        assert!(
            !FiltroBuild::do_argumento("lib/{a,b}.txt", "r", false)
                .unwrap()
                .casa(&AssetId::novo("r", "lib/a.txt"))
        );
        // `package:`: os segmentos, desescapados.
        let p = FiltroBuild::do_argumento("package:r/a%20b.*", "r", false).unwrap();
        assert!(p.casa(&AssetId::novo("r", "lib/a b.txt")));
        let g = FiltroBuild::do_argumento("package:{a,b}/**", "r", false).unwrap();
        assert!(g.casa(&AssetId::novo("b", "lib/x/y.dart")));
        assert!(!g.casa(&AssetId::novo("b", "web/y.dart")));
        assert!(FiltroBuild::do_argumento("http://x/y", "r", true).is_err());
    }
}

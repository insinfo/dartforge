//! `i18n:yamlBasedBuilder` (`package:i18n` 4.2.1, `lib/builder.dart` e
//! `lib/src/i18n_impl.dart`): cada `x.i18n.yaml` vira `x.i18n.dart` no
//! próprio pacote (`build_to: source`) — uma classe por mapa do YAML, um
//! getter por mensagem (com o texto num comentário de documentação) e o mapa
//! `xMap` com as chaves achatadas —, formatado pelo `DartFormatter` de 80
//! colunas (dart_style 2.3.8, o estilo curto).
//!
//! O texto sai já formatado: o gerador só produz poucas formas (a classe, o
//! construtor, os getters `=>`, o comentário, o mapa), e cada uma quebra num
//! lugar só quando passa de 80 colunas — o getter depois do `=>`, com o
//! corpo seis colunas para dentro; a entrada do mapa depois do `:`, com o
//! valor dez colunas para dentro. O que o formatador faria de outro jeito (a
//! declaração da classe ou o construtor longos demais, a primeira linha de
//! um texto de várias linhas que não cabe) é recusado, como o que o builder
//! oficial rejeita (chave repetida entre os idiomas fora de ordem, código de
//! idioma inválido, valor que não é texto, número nem booleano).
use crate::consulta::Consulta;
use crate::executor::{CtxGerador, GeradorNativo, PedidoNativo, SaidaNativa};
use std::path::Path;
use yaml_rust2::{Yaml, YamlLoader};

pub struct I18nNativo;

/// As pastas de fontes de um pacote no `build_runner` (os `sources` padrão
/// do alvo `$default`), onde o `findAssets('**.i18n.yaml')` procura.
const PASTAS_DE_FONTES: &[&str] = &["benchmark", "bin", "example", "lib", "test", "tool", "web"];

/// Largura de página do `DartFormatter()` padrão.
const LARGURA: usize = 80;

/// `Metadata` do `i18n`.
#[derive(Clone)]
struct Metadados {
    pai: Option<Box<Metadados>>,
    padrao: bool,
    nome_padrao: String,
    arquivo_padrao: Option<String>,
    nome: String,
    localidade: String,
    idioma: String,
}

impl Metadados {
    fn aninhar(&self, prefixo: &str) -> Metadados {
        Metadados {
            pai: Some(Box::new(self.clone())),
            padrao: self.padrao,
            nome_padrao: format!("{prefixo}{}", self.nome_padrao),
            arquivo_padrao: self.arquivo_padrao.clone(),
            nome: format!("{prefixo}{}", self.nome),
            localidade: self.localidade.clone(),
            idioma: self.idioma.clone(),
        }
    }
}

/// `String.firstUpper`.
fn primeira_maiuscula(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(p) => p.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// `String.firstLower`.
fn primeira_minuscula(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(p) => p.to_lowercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// `String.filterHyphen`: `a-b-c` vira `aBC`.
fn sem_hifen(s: &str) -> String {
    let mut partes = s.split('-');
    let primeira = partes.next().unwrap_or_default().to_string();
    partes.fold(primeira, |a, p| a + &primeira_maiuscula(p))
}

/// `String.filterSpaces`: os espaços antes do primeiro `(` somem e cada
/// palavra depois da primeira ganha maiúscula; o resto (sem os `(`) volta
/// depois de um `(`.
fn sem_espacos(s: &str) -> String {
    let mut f: Vec<&str> = s.split('(').collect();
    let primeira = f.remove(0);
    let mut palavras = primeira.split(' ');
    let inicio = palavras.next().unwrap_or_default().to_string();
    let convertido = palavras.fold(inicio, |a, p| a + &primeira_maiuscula(p));
    if f.is_empty() {
        return convertido;
    }
    format!("{convertido}({}", f.concat())
}

/// `String.convertName`: `AppMessages_en_US` vira `AppMessagesEnUS`; cada
/// parte depois do primeiro `_` tem de ter duas letras.
fn nome_convertido(s: &str) -> Result<String, String> {
    let rep = sem_espacos(&sem_hifen(s));
    let partes: Vec<&str> = rep.split('_').collect();
    if partes.len() == 1 {
        return Ok(rep);
    }
    let mut saida = partes[0].to_string();
    for p in &partes[1..] {
        if p.chars().count() != 2 {
            return Err(format!("i18n: `{p}` não é código de idioma nem de país (o oficial lança)"));
        }
        saida += &primeira_maiuscula(p);
    }
    Ok(saida)
}

/// `generateMessageObjectName`.
fn metadados_do_arquivo(nome_do_arquivo: &str) -> Result<Metadados, String> {
    let nome = nome_do_arquivo.replace(".i18n.yaml", "");
    let partes: Vec<&str> = nome.split('_').collect();
    let nome_padrao = primeira_maiuscula(partes[0]);
    if partes.len() == 1 {
        return Ok(Metadados {
            pai: None,
            padrao: true,
            nome_padrao: nome_padrao.clone(),
            arquivo_padrao: None,
            nome: nome_padrao,
            localidade: "en".into(),
            idioma: "en".into(),
        });
    }
    if partes.len() > 3 {
        return Err("i18n: nome de arquivo com mais de dois `_` (o oficial lança)".into());
    }
    let idioma = partes[1];
    if !(idioma.len() == 2 && idioma.bytes().all(|b| b.is_ascii_lowercase())) {
        return Err(format!("i18n: código de idioma `{idioma}` inválido (o oficial lança)"));
    }
    let mut localidade = idioma.to_string();
    if partes.len() == 3 {
        let pais = partes[2];
        if !(pais.len() == 2 && pais.bytes().all(|b| b.is_ascii_uppercase())) {
            return Err(format!("i18n: código de país `{pais}` inválido (o oficial lança)"));
        }
        localidade = format!("{idioma}_{pais}");
    }
    Ok(Metadados {
        pai: None,
        padrao: false,
        nome: format!("{nome_padrao}_{localidade}"),
        nome_padrao,
        arquivo_padrao: Some(format!("{}.i18n.dart", partes[0])),
        localidade,
        idioma: idioma.to_string(),
    })
}

/// Um valor do YAML como o `"$v"` do Dart o escreve.
fn texto_do_valor(v: &Yaml) -> Result<Option<String>, String> {
    Ok(match v {
        Yaml::String(s) => Some(s.clone()),
        Yaml::Integer(i) => Some(i.to_string()),
        Yaml::Boolean(b) => Some(b.to_string()),
        Yaml::Null => None,
        _ => return Err("i18n: valor que não é texto, inteiro nem booleano".into()),
    })
}

/// O mapa do YAML, com as chaves em texto e na ordem escrita.
fn entradas(m: &yaml_rust2::yaml::Hash) -> Result<Vec<(String, &Yaml)>, String> {
    m.iter()
        .map(|(k, v)| match k {
            Yaml::String(s) => Ok((s.clone(), v)),
            _ => Err("i18n: chave que não é texto (o oficial lança)".to_string()),
        })
        .collect()
}

/// `YamlMapX.allKeys`: as chaves em pré-ordem.
fn todas_as_chaves(m: &yaml_rust2::yaml::Hash, saida: &mut Vec<String>) -> Result<(), String> {
    for (k, v) in entradas(m)? {
        saida.push(k);
        if let Yaml::Hash(h) = v {
            todas_as_chaves(h, saida)?;
        }
    }
    Ok(())
}

/// Largura de uma linha como o `DartFormatter` a mede (`String.length`,
/// em unidades UTF-16).
fn largura(s: &str) -> usize {
    s.encode_utf16().count()
}

/// `_wrapWithComments`, já recuado.
fn comentario(texto: &str, saida: &mut String) {
    let linhas: Vec<&str> = texto.lines().collect();
    let mut s = vec!["```dart".to_string()];
    if linhas.len() > 1 {
        s.push("\"\"\"".into());
        s.extend(linhas.iter().map(|l| l.to_string()));
        s.push("\"\"\"".into());
    } else {
        s.push(format!("\"{texto}\""));
    }
    s.push("```".into());
    for l in s {
        // O formatador tira o espaço do fim da linha de comentário.
        saida.push_str(format!("  /// {l}").trim_end());
        saida.push('\n');
    }
}

/// `recuo` + `cabeca => corpo;`, quebrando depois do `=>` quando a linha
/// (até a primeira quebra do corpo) passa de 80 colunas.
fn getter(cabeca: &str, corpo: &str, saida: &mut String) -> Result<(), String> {
    let primeira = corpo.split('\n').next().unwrap_or_default();
    let multilinha = corpo.contains('\n');
    let fim = if multilinha { "" } else { ";" };
    let numa_linha = format!("  {cabeca} => {primeira}{fim}");
    if largura(&numa_linha) <= LARGURA {
        saida.push_str(&format!("  {cabeca} => {corpo};\n"));
        return Ok(());
    }
    if multilinha {
        return Err("i18n: texto de várias linhas cuja primeira não cabe (forma do formatador sem caso)".into());
    }
    if largura(&format!("  {cabeca} =>")) > LARGURA {
        return Err("i18n: assinatura de getter longa demais (forma do formatador sem caso)".into());
    }
    saida.push_str(&format!("  {cabeca} =>\n      {corpo};\n"));
    Ok(())
}

/// Uma linha que o formatador deixaria como está, ou recusa.
fn linha_curta(l: &str, saida: &mut String) -> Result<(), String> {
    if largura(l) > LARGURA {
        return Err(format!("i18n: `{}` passa de 80 colunas (forma do formatador sem caso)", l.trim()));
    }
    saida.push_str(l);
    saida.push('\n');
    Ok(())
}

/// `renderTranslation`.
fn classe(meta: &Metadados, conteudo: &yaml_rust2::yaml::Hash, saida: &mut String) -> Result<(), String> {
    let padrao = nome_convertido(&meta.nome_padrao)?;
    let nome = nome_convertido(&meta.nome)?;
    let pai = meta.pai.as_ref().map(|p| nome_convertido(&p.nome)).transpose()?;
    if meta.padrao {
        linha_curta(&format!("class {nome} {{"), saida)?;
    } else {
        linha_curta(&format!("class {nome} extends {padrao} {{"), saida)?;
    }
    match &pai {
        None => {
            linha_curta(&format!("  const {nome}();"), saida)?;
            linha_curta(&format!("  String get locale => \"{}\";", meta.localidade), saida)?;
            linha_curta(&format!("  String get languageCode => \"{}\";", meta.idioma), saida)?;
        }
        Some(p) => {
            linha_curta(&format!("  final {p} _parent;"), saida)?;
            if meta.padrao {
                linha_curta(&format!("  const {nome}(this._parent);"), saida)?;
            } else {
                linha_curta(&format!("  const {nome}(this._parent) : super(_parent);"), saida)?;
            }
        }
    }
    for (k, v) in entradas(conteudo)? {
        let chave = sem_hifen(&sem_espacos(&k));
        if let Yaml::Hash(_) = v {
            let filho = nome_convertido(&meta.aninhar(&primeira_maiuscula(&chave)).nome)?;
            getter(&format!("{filho} get {chave}"), &format!("{filho}(this)"), saida)?;
            continue;
        }
        let texto = texto_do_valor(v)?;
        // O comentário (ou a linha vazia que o `writeln('')` deixa) abre o
        // membro com uma linha em branco.
        saida.push('\n');
        if let Some(t) = texto.as_deref().filter(|t| !t.is_empty()) {
            comentario(t, saida);
        }
        let valor = format!("\"\"\"{}\"\"\"", texto.as_deref().unwrap_or("null"));
        if k.contains('(') {
            getter(&format!("String {chave}"), &valor, saida)?;
        } else {
            getter(&format!("String get {chave}"), &valor, saida)?;
        }
    }
    saida.push_str("}\n");
    Ok(())
}

/// `prepareTranslationList` e o `renderTranslation` de cada classe, em
/// pré-ordem. O nome da classe aninhada sai da chave sem filtrar
/// (`k.firstUpper()`), como no oficial.
fn classes(meta: &Metadados, m: &yaml_rust2::yaml::Hash, saida: &mut String) -> Result<(), String> {
    classe(meta, m, saida)?;
    saida.push('\n');
    for (k, v) in entradas(m)? {
        if let Yaml::Hash(h) = v {
            classes(&meta.aninhar(&primeira_maiuscula(&k)), h, saida)?;
        }
    }
    Ok(())
}

/// `String.containsReference`.
fn tem_referencia(s: &str) -> Result<bool, String> {
    let mut refs: Vec<&str> = s.split('$').collect();
    refs.pop();
    if refs.is_empty() {
        return Ok(false);
    }
    let escapadas = refs.iter().filter(|r| r.ends_with('\\')).count();
    if escapadas == 0 {
        return Ok(true);
    }
    if escapadas == refs.len() {
        return Ok(false);
    }
    Err("i18n: texto que mistura `$` escapado e não escapado (o oficial lança)".into())
}

/// `renderMapEntries`: só os valores de texto sem referência.
fn entradas_do_mapa(m: &yaml_rust2::yaml::Hash, prefixo: &str, saida: &mut String) -> Result<(), String> {
    for (k, v) in entradas(m)? {
        match v {
            Yaml::Hash(h) => entradas_do_mapa(h, &format!("{prefixo}{k}."), saida)?,
            Yaml::String(s) if !tem_referencia(s)? => {
                let chave = format!("\"\"\"{prefixo}{k}\"\"\":");
                let valor = format!("\"\"\"{s}\"\"\",");
                let primeira = valor.split('\n').next().unwrap_or_default();
                if largura(&format!("      {chave} {primeira}")) <= LARGURA {
                    saida.push_str(&format!("      {chave} {valor}\n"));
                } else if valor.contains('\n') {
                    return Err("i18n: texto de várias linhas cuja primeira não cabe no mapa (forma do formatador sem caso)".into());
                } else {
                    saida.push_str(&format!("      {chave}\n          {valor}\n"));
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// As funções `_plural`, `_ordinal` e `_cardinal`, como o formatador as
/// deixa.
fn funcoes(saida: &mut String) {
    for f in ["plural", "ordinal", "cardinal"] {
        saida.push_str(&format!("String _{f}(\n  int count, {{\n"));
        for p in ["zero", "one", "two", "few", "many", "other"] {
            saida.push_str(&format!("  String? {p},\n"));
        }
        saida.push_str(&format!("}}) =>\n    i18n.{f}(\n      count,\n      _languageCode,\n"));
        for p in ["zero", "one", "two", "few", "many", "other"] {
            saida.push_str(&format!("      {p}: {p},\n"));
        }
        saida.push_str("    );\n");
    }
}

/// `generateDartContentFromYaml`, formatado.
fn gerar(nome_do_arquivo: &str, yaml: &str) -> Result<String, String> {
    let meta = metadados_do_arquivo(nome_do_arquivo)?;
    let docs = YamlLoader::load_from_str(yaml).map_err(|e| format!("i18n: YAML ilegível: {e}"))?;
    let Some(Yaml::Hash(m)) = docs.first() else {
        return Err("i18n: o YAML não é um mapa (o oficial lança)".into());
    };
    let mut s = String::new();
    s.push_str("// GENERATED FILE, do not edit!\n");
    s.push_str("// ignore_for_file: annotate_overrides, non_constant_identifier_names, prefer_single_quotes, unused_element, unused_field\n");
    s.push_str("import 'package:i18n/i18n.dart' as i18n;\n");
    if let Some(a) = &meta.arquivo_padrao {
        s.push_str(&format!("import '{a}';\n"));
    }
    s.push('\n');
    s.push_str(&format!("String get _languageCode => '{}';\n", meta.idioma));
    funcoes(&mut s);
    s.push('\n');
    classes(&meta, m, &mut s)?;
    let mapa = format!("{}Map", primeira_minuscula(&nome_convertido(&meta.nome)?));
    let mut corpo = String::new();
    entradas_do_mapa(m, "", &mut corpo)?;
    if corpo.is_empty() {
        linha_curta(&format!("Map<String, String> get {mapa} => {{}};"), &mut s)?;
    } else {
        linha_curta(&format!("Map<String, String> get {mapa} => {{"), &mut s)?;
        s.push_str(&corpo);
        s.push_str("    };\n");
    }
    Ok(s)
}

/// O arquivo padrão de `atual` entre os `**.i18n.yaml` do pacote (em ordem
/// de caminho): o primeiro sem `_` no nome cujo nome é prefixo do atual
/// (`firstWhere`, que lança sem nenhum).
fn arquivo_padrao<'a>(atual: &str, todos: &'a [String]) -> Option<&'a String> {
    let nome_atual = atual.rsplit('/').next().unwrap_or(atual).replace(".i18n.yaml", "");
    todos.iter().find(|c| {
        let nome = c.rsplit('/').next().unwrap_or(c).replace(".i18n.yaml", "");
        !nome.contains('_') && nome_atual.starts_with(&nome)
    })
}

impl GeradorNativo for I18nNativo {
    fn chave(&self) -> &'static str {
        "i18n:yamlBasedBuilder"
    }

    fn cobre(&self, _fabrica: &str) -> bool {
        true
    }

    fn por_pacote(&self) -> bool {
        false
    }

    fn verificado(&self) -> bool {
        true
    }

    fn gerar(&self, ctx: &mut CtxGerador<'_>, pedido: &PedidoNativo) -> Result<SaidaNativa, String> {
        let mut s = SaidaNativa::default();
        // `findAssets(Glob('**.i18n.yaml'))` nas pastas de fontes do pacote.
        let glob = crate::glob::Glob::novo("**.i18n.yaml").map_err(|e| e.to_string())?;
        let mut todos: Vec<String> = Vec::new();
        for pasta in PASTAS_DE_FONTES {
            let dir = pedido.raiz_do_pacote.join(pasta);
            ctx.registrar(Consulta::Glob {
                dir: dartforge_elements::gerado::chave(&dir),
                padrao: "**.i18n.yaml".into(),
            });
            todos.extend(
                crate::grafo::listar(&dir, std::slice::from_ref(&glob))
                    .into_iter()
                    .map(|r| format!("{pasta}/{r}")),
            );
        }
        todos.sort();
        for a in &pedido.acoes {
            let nome = a.entrada.caminho.rsplit('/').next().unwrap_or_default().to_string();
            let Some(fonte) = ctx.ler(&a.entrada_natural) else {
                s.recusas.insert(a.entrada_natural.clone(), "i18n: entrada ilegível".into());
                continue;
            };
            let texto = String::from_utf8_lossy(&fonte).into_owned();
            let resultado = (|| -> Result<String, String> {
                let padrao = arquivo_padrao(&a.entrada.caminho, &todos)
                    .ok_or("i18n: sem o arquivo padrão (o oficial lança)")?;
                // Cada chave no mesmo lugar do arquivo padrão (o oficial
                // acusa erro, `log.severe`, e a build falha).
                if padrao.as_str() != &*a.entrada.caminho {
                    let caminho = pedido.raiz_do_pacote.join(padrao);
                    let bytes = ctx.ler(Path::new(&caminho)).ok_or("i18n: arquivo padrão ilegível")?;
                    let chaves = |t: &str| -> Result<Vec<String>, String> {
                        let docs = YamlLoader::load_from_str(t).map_err(|e| format!("i18n: YAML ilegível: {e}"))?;
                        let Some(Yaml::Hash(m)) = docs.first() else {
                            return Err("i18n: o YAML não é um mapa".into());
                        };
                        let mut v = Vec::new();
                        todas_as_chaves(m, &mut v)?;
                        Ok(v)
                    };
                    if chaves(&texto)? != chaves(&String::from_utf8_lossy(&bytes))? {
                        return Err("i18n: chaves diferentes das do arquivo padrão (o oficial acusa erro)".into());
                    }
                }
                gerar(&nome, &texto)
            })();
            match resultado {
                Ok(dart) => {
                    if let Some((_, n)) = a.saidas.first() {
                        s.saidas.insert(n.clone(), dart.into_bytes());
                    }
                }
                Err(f) => {
                    s.recusas.insert(a.entrada_natural.clone(), f);
                }
            }
        }
        Ok(s)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn nomes_como_o_i18n() {
        assert_eq!(sem_espacos("my key(int n)"), "myKey(int n)");
        assert_eq!(sem_hifen("a-b-c"), "aBC");
        assert_eq!(nome_convertido("AppMessages_en").unwrap(), "AppMessagesEn");
        assert_eq!(nome_convertido("AppMessages_pt_BR").unwrap(), "AppMessagesPtBR");
        assert!(nome_convertido("App_foo").is_err());
        assert!(tem_referencia("a $b").unwrap());
        assert!(!tem_referencia(r"a \$b").unwrap());
    }

    #[test]
    fn getter_longo_quebra_depois_da_seta() {
        let mut s = String::new();
        getter("String get x", &format!("\"\"\"{}\"\"\"", "a".repeat(80)), &mut s).unwrap();
        assert!(s.starts_with("  String get x =>\n      \"\"\"aaa"));
    }
}

//! Apoio dos testes semânticos do LSP (completar, renomear, ações): um SDK
//! mínimo em disco, um projeto temporário e o servidor falando JSON-RPC.
#![allow(dead_code)]

use dartforge_elements::sdk::SdkLayout;
use dartforge_lsp::{AnalisadorSemantico, Servidor};
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};

/// `dart:core` reduzido: o bastante para membros herdados, genéricos,
/// enums e extensões, sem depender de um SDK instalado.
const CORE: &str = r#"library dart.core;
class Object {
  const Object();
  bool operator ==(Object other) => true;
  external int get hashCode;
  external String toString();
  external dynamic noSuchMethod(Invocation invocation);
  external Type get runtimeType;
}
class Invocation {}
class Type {}
class Null {}
class bool {}
abstract class Comparable<T> { int compareTo(T other); }
abstract class num implements Comparable<num> {
  num operator +(num other);
  num abs();
}
abstract class int extends num {
  int operator +(num other);
  bool get isEven;
}
abstract class double extends num {}
abstract class String implements Comparable<String> {
  int get length;
  String substring(int start, [int? end]);
  String toUpperCase();
}
abstract class Iterable<E> {
  E get first;
  int get length;
}
abstract class List<E> implements Iterable<E> {
  void add(E value);
  E removeLast();
}
abstract class Map<K, V> {}
abstract class Set<E> implements Iterable<E> {}
abstract class Function {}
abstract class Record {}
abstract class Enum {
  int get index;
}
abstract class Symbol {}
abstract class Pattern {}
class Deprecated { const Deprecated(String message); }
const Object override = _Override();
class _Override { const _Override(); }
void print(Object? object) {}
"#;

/// Projeto temporário com SDK mínimo e servidor semântico.
pub struct Projeto {
    pub raiz: PathBuf,
    pub servidor: Servidor<AnalisadorSemantico>,
    proximo_id: i64,
    /// A resposta do `initialize` do servidor (o servidor recusa um segundo
    /// `initialize`, como o do Dart: `serverAlreadyInitialized`).
    #[allow(dead_code)]
    pub inicializacao: Value,
    /// Os documentos abertos, para [`Projeto::reiniciar`].
    abertos: Vec<(String, String)>,
}

impl Projeto {
    /// Cria `target/tmp-agent/<nome>-<pid>` com `pubspec.yaml` e SDK.
    pub fn novo(nome: &str) -> Self {
        Self::com_capacidades(nome, json!({}))
    }

    /// [`Projeto::novo`] com `CodeAction` literal (sem ela, o servidor do
    /// Dart não oferece correções nem assistências).
    #[allow(dead_code)]
    pub fn com_literais(nome: &str) -> Self {
        Self::com_capacidades(
            nome,
            json!({"textDocument": {"codeAction": {"codeActionLiteralSupport": {"codeActionKind": {"valueSet": ["quickfix", "refactor", "source"]}}}}}),
        )
    }

    /// [`Projeto::novo`] com as capacidades de cliente dadas no `initialize`.
    #[allow(dead_code)]
    pub fn com_capacidades(nome: &str, capacidades: Value) -> Self {
        let raiz = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/tmp-agent/lsp-{nome}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&raiz);
        fs::create_dir_all(raiz.join("projeto/lib")).unwrap();
        let raiz = fs::canonicalize(raiz).unwrap();
        let lib = raiz.join("sdk/lib");
        fs::create_dir_all(lib.join("core")).unwrap();
        fs::write(
            lib.join("libraries.json"),
            r#"{"dartdevc":{"libraries":{
                "core":{"uri":"core/core.dart","patches":[]},
                "math":{"uri":"math/math.dart","patches":[]},
                "_interna":{"uri":"interna/interna.dart","patches":[]}
            }}}"#,
        )
        .unwrap();
        fs::write(lib.join("core/core.dart"), CORE).unwrap();
        fs::create_dir_all(lib.join("math")).unwrap();
        fs::write(
            lib.join("math/math.dart"),
            "library dart.math;\npart 'aleatorio.dart';\nconst double pi = 3.14;\n",
        )
        .unwrap();
        fs::write(
            lib.join("math/aleatorio.dart"),
            "part of dart.math;\nclass Random { int nextInt(int max) => 0; }\n",
        )
        .unwrap();
        fs::create_dir_all(lib.join("interna")).unwrap();
        fs::write(
            lib.join("interna/interna.dart"),
            "library dart._interna;\nclass Random {}\n",
        )
        .unwrap();
        let sdk = SdkLayout::load(&lib, "dartdevc").unwrap();
        fs::write(raiz.join("projeto/pubspec.yaml"), "name: projeto\n").unwrap();
        fs::create_dir_all(raiz.join("projeto/.dart_tool")).unwrap();
        fs::write(
            raiz.join("projeto/.dart_tool/package_config.json"),
            r#"{"configVersion":2,"packages":[{"name":"projeto","rootUri":"../","packageUri":"lib/","languageVersion":"3.6"}]}"#,
        )
        .unwrap();
        let servidor = Servidor::com_analisador(AnalisadorSemantico::novo(Some(sdk)));
        let mut p = Self {
            raiz,
            servidor,
            proximo_id: 1,
            inicializacao: Value::Null,
            abertos: Vec::new(),
        };
        p.inicializacao = p.inicializar(json!({"capabilities": capacidades}));
        p
    }

    /// O `initialize` (com o `rootUri` do projeto acrescentado aos `params`).
    fn inicializar(&mut self, mut params: Value) -> Value {
        params["rootUri"] = json!(url::Url::from_file_path(self.raiz.join("projeto")).unwrap().to_string());
        self.servidor.receber(json!({"jsonrpc":"2.0","id":0,"method":"initialize","params":params}));
        self.servidor.bombear().into_iter().find(|m| m["id"] == json!(0)).unwrap_or(Value::Null)
    }

    /// Um servidor novo, inicializado com `params`, com os documentos que
    /// estavam abertos reabertos; devolve a resposta do `initialize`.
    #[allow(dead_code)]
    pub fn reiniciar(&mut self, params: Value) -> Value {
        let sdk = SdkLayout::load(&self.raiz.join("sdk/lib"), "dartdevc").unwrap();
        self.servidor = Servidor::com_analisador(AnalisadorSemantico::novo(Some(sdk)));
        self.inicializacao = self.inicializar(params);
        for (rel, texto) in std::mem::take(&mut self.abertos) {
            self.abrir(&rel, &texto);
        }
        self.inicializacao.clone()
    }

    /// Caminho de um arquivo do projeto.
    pub fn caminho(&self, relativo: &str) -> PathBuf {
        self.raiz.join("projeto").join(relativo)
    }

    /// URI `file:` de um arquivo do projeto.
    pub fn uri(&self, relativo: &str) -> String {
        url::Url::from_file_path(self.caminho(relativo))
            .unwrap()
            .to_string()
    }

    /// Grava no disco (sem abrir no editor).
    pub fn gravar(&self, relativo: &str, texto: &str) {
        let caminho = self.caminho(relativo);
        fs::create_dir_all(caminho.parent().unwrap()).unwrap();
        fs::write(caminho, texto).unwrap();
    }

    /// Grava no disco e abre no editor; devolve os diagnósticos publicados.
    /// Sem publicação (como o servidor do Dart, nenhuma lista vazia sai
    /// para um arquivo que não tinha erros publicados), a lista vazia.
    pub fn abrir(&mut self, relativo: &str, texto: &str) -> Value {
        self.gravar(relativo, texto);
        self.abertos.retain(|(r, _)| r != relativo);
        self.abertos.push((relativo.to_string(), texto.to_string()));
        let uri = self.uri(relativo);
        self.servidor.receber(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
            "textDocument":{"uri":uri,"languageId":"dart","version":1,"text":texto}
        }}));
        self.servidor
            .bombear()
            .into_iter()
            .rev()
            .find(|m| m["method"] == "textDocument/publishDiagnostics" && m["params"]["uri"] == uri)
            .unwrap_or_else(|| json!({"jsonrpc": "2.0", "method": "textDocument/publishDiagnostics", "params": {"uri": uri, "diagnostics": []}}))
    }

    /// Substitui o texto do documento aberto (versão nova), sem gravar.
    pub fn mudar(&mut self, relativo: &str, versao: i64, texto: &str) {
        self.servidor.receber(
            json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
                "textDocument":{"uri":self.uri(relativo),"version":versao},
                "contentChanges":[{"text":texto}]
            }}),
        );
        self.servidor.bombear();
    }

    /// Envia uma requisição e devolve a resposta inteira.
    pub fn requisitar(&mut self, metodo: &str, params: Value) -> Value {
        let id = self.proximo_id;
        self.proximo_id += 1;
        self.servidor
            .receber(json!({"jsonrpc":"2.0","id":id,"method":metodo,"params":params}));
        let saidas = self.servidor.bombear();
        saidas
            .into_iter()
            .find(|m| m["id"] == json!(id))
            .expect("resposta")
    }

    /// Responde aos `workspace/configuration` do servidor com a seção `dart`
    /// dada (o servidor os pede no `initialized` e a cada
    /// `workspace/didChangeConfiguration`; o cliente precisa ter anunciado
    /// `workspace.configuration`).
    #[allow(dead_code)]
    pub fn configurar(&mut self, dart: Value) {
        self.servidor
            .receber(json!({"jsonrpc":"2.0","method":"workspace/didChangeConfiguration","params":{"settings":{}}}));
        let mut saidas = self.servidor.bombear();
        while let Some(pedido) = saidas.iter().find(|m| m["method"] == "workspace/configuration").cloned() {
            let n = pedido["params"]["items"].as_array().map_or(1, |a| a.len());
            let resultado: Vec<Value> = (0..n).map(|_| dart.clone()).collect();
            self.servidor
                .receber(json!({"jsonrpc":"2.0","id":pedido["id"].clone(),"result":resultado}));
            saidas = self.servidor.bombear();
        }
    }

    /// Requisição com `textDocument` + `position` na posição do marcador
    /// `▮` em `texto` (o marcador não faz parte do arquivo).
    pub fn na_posicao(
        &mut self,
        metodo: &str,
        relativo: &str,
        linha: u32,
        coluna: u32,
        extra: Value,
    ) -> Value {
        let mut params = json!({
            "textDocument": {"uri": self.uri(relativo)},
            "position": {"line": linha, "character": coluna},
        });
        if let Value::Object(mais) = extra {
            for (k, v) in mais {
                params[k] = v;
            }
        }
        self.requisitar(metodo, params)
    }
}

impl Drop for Projeto {
    fn drop(&mut self) {
        // A análise tipada grava o cache do SDK mínimo (um por projeto, pelo
        // caminho): some com ele, para não acumular entre execuções.
        if let Ok(sdk) = SdkLayout::load(&self.raiz.join("sdk/lib"), "dartdevc") {
            let _ = fs::remove_file(dartforge_elements::SdkCache::caminho(&sdk, "dartdevc"));
        }
        let _ = fs::remove_dir_all(&self.raiz);
    }
}

/// Separa o marcador `▮` do texto: devolve o texto sem ele e a posição
/// LSP (linha, coluna UTF-16) onde estava.
pub fn marcar(texto: &str) -> (String, u32, u32) {
    let i = texto.find('▮').expect("marcador");
    let antes = &texto[..i];
    let linha = antes.matches('\n').count() as u32;
    let coluna = antes.rsplit('\n').next().unwrap().encode_utf16().count() as u32;
    (texto.replacen('▮', "", 1), linha, coluna)
}

/// Rótulos dos itens de uma resposta de completar, na ordem.
pub fn rotulos(resposta: &Value) -> Vec<String> {
    resposta["result"]["items"]
        .as_array()
        .unwrap_or_else(|| panic!("sem itens: {resposta}"))
        .iter()
        .map(|i| i["label"].as_str().unwrap().to_string())
        .collect()
}

/// O item de rótulo `rotulo`.
pub fn item<'a>(resposta: &'a Value, rotulo: &str) -> &'a Value {
    resposta["result"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["label"] == rotulo)
        .unwrap_or_else(|| panic!("sem {rotulo} em {:?}", rotulos(resposta)))
}

/// Offset em bytes de uma posição LSP (linha, coluna UTF-16).
pub fn offset_de(texto: &str, posicao: &Value) -> usize {
    let linha = posicao["line"].as_u64().unwrap() as usize;
    let coluna = posicao["character"].as_u64().unwrap() as usize;
    let inicio: usize = texto.split_inclusive('\n').take(linha).map(str::len).sum();
    let mut unidades = 0;
    for (i, c) in texto[inicio..].char_indices() {
        if unidades >= coluna || c == '\n' {
            return inicio + i;
        }
        unidades += c.len_utf16();
    }
    texto.len()
}

/// Aplica as edições (`TextEdit[]`) de `uri` de um `WorkspaceEdit` a `texto`.
pub fn aplicar(edicao: &Value, uri: &str, texto: &str) -> String {
    let Some(lista) = edicao["changes"][uri].as_array() else {
        return texto.to_string();
    };
    let mut trocas: Vec<(usize, usize, &str)> = lista
        .iter()
        .map(|e| {
            (
                offset_de(texto, &e["range"]["start"]),
                offset_de(texto, &e["range"]["end"]),
                e["newText"].as_str().unwrap(),
            )
        })
        .collect();
    trocas.sort_by_key(|t| std::cmp::Reverse(t.0));
    let mut saida = texto.to_string();
    for (de, ate, novo) in trocas {
        saida.replace_range(de..ate, novo);
    }
    saida
}

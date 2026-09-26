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
}

impl Projeto {
    /// Cria `target/tmp-agent/<nome>-<pid>` com `pubspec.yaml` e SDK.
    pub fn novo(nome: &str) -> Self {
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
            r#"{"dartdevc":{"libraries":{"core":{"uri":"core/core.dart","patches":[]}}}}"#,
        )
        .unwrap();
        fs::write(lib.join("core/core.dart"), CORE).unwrap();
        let sdk = SdkLayout::load(&lib, "dartdevc").unwrap();
        fs::write(raiz.join("projeto/pubspec.yaml"), "name: projeto\n").unwrap();
        let mut servidor = Servidor::com_analisador(AnalisadorSemantico::novo(Some(sdk)));
        servidor.receber(
            json!({"jsonrpc":"2.0","id":0,"method":"initialize","params":{
                "rootUri": url::Url::from_file_path(raiz.join("projeto")).unwrap().to_string(),
                "capabilities": {}
            }}),
        );
        servidor.bombear();
        Self {
            raiz,
            servidor,
            proximo_id: 1,
        }
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
    pub fn abrir(&mut self, relativo: &str, texto: &str) -> Value {
        self.gravar(relativo, texto);
        self.servidor.receber(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
            "textDocument":{"uri":self.uri(relativo),"languageId":"dart","version":1,"text":texto}
        }}));
        self.servidor.bombear().pop().unwrap_or(Value::Null)
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

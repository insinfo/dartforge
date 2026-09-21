//! Contrato dos prefixos de import e dos diagnósticos de biblioteca ausente.
//!
//! Um `import '...' as p;` não acrescenta nada ao namespace sem qualificação: o
//! namespace da biblioteca importada passa a ser alcançável **apenas** por
//! `p.nome`. O contrato completo, com os limites que permanecem, está em
//! `docs/IMPORTS.md`.
use dartforge_compiler::{Optimization, compile_path};
use dartforge_diagnostics::Span;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(0);

/// Diretório temporário exclusivo, removido ao fim do teste.
struct Fixture(PathBuf);
impl Fixture {
    /// Grava os arquivos pedidos sem depender do diretório atual nem do SDK.
    fn new(files: &[(&str, &str)]) -> Self {
        let path = std::env::temp_dir().join(format!(
            "dartforge-prefixos-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        let fixture = Self(path);
        for (name, text) in files {
            fixture.write(name, text);
        }
        fixture
    }
    /// Grava um arquivo criando os diretórios intermediários necessários.
    fn write(&self, name: &str, text: &str) {
        let path = self.0.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    /// Devolve a entrada da fixture.
    fn entry(&self) -> PathBuf {
        self.0.join("main.dart")
    }
}
impl Drop for Fixture {
    /// Remove apenas o diretório exclusivo criado por esta fixture.
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Compila a entrada da fixture e devolve o módulo JavaScript emitido.
fn javascript(fixture: &Fixture) -> String {
    compile_path(&fixture.entry(), Optimization::None)
        .unwrap_or_else(|error| panic!("{}: {}", error.path.display(), error.message))
}

/// Confere mensagem e intervalo exatos do diagnóstico, no arquivo esperado.
fn rejeita(fixture: &Fixture, arquivo: &str, fonte: &str, mensagem: &str, trecho: &str) {
    let error = compile_path(&fixture.entry(), Optimization::None).expect_err(mensagem);
    assert_eq!(error.message, mensagem);
    assert_eq!(
        error.path.file_name(),
        Some(std::ffi::OsStr::new(arquivo)),
        "{}",
        error.path.display()
    );
    assert_eq!(error.span, Some(intervalo(fonte, trecho)), "{trecho}");
}

/// Calcula o intervalo de um trecho único da fonte, em bytes.
fn intervalo(fonte: &str, trecho: &str) -> Span {
    let start = fonte.find(trecho).expect(trecho);
    assert_eq!(fonte.rfind(trecho), Some(start), "trecho ambíguo: {trecho}");
    Span {
        start,
        end: start + trecho.len(),
    }
}

/// Executa o módulo emitido no Node e devolve a saída padrão normalizada.
fn node(js: &str) -> String {
    let output = std::process::Command::new("node")
        .args(["--input-type=module", "--eval", js])
        .output()
        .expect("node no PATH");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("saída UTF-8")
        .replace("\r\n", "\n")
}

/// Biblioteca de imagens usada como destino dos imports com prefixo.
const IMAGEM: &str = "class Image { int largura; Image(this.largura); Image.vazia() : largura = 0; \
int area() { return largura * 2; } } \
Image decodePng(int bytes) { return Image(bytes); } \
class Config { static int padrao() { return 9; } } \
int _oculto() { return 1; }";

/// Função com prefixo, tipo com prefixo e construtores nomeados resolvem.
#[test]
fn funcao_tipo_e_construtores_com_prefixo_resolvem() {
    let fixture = Fixture::new(&[
        (
            "main.dart",
            "import 'imagem.dart' as im; \
             im.Image dobra(im.Image entrada) { return im.Image(entrada.largura); } \
             void main() { im.Image quadro = im.decodePng(3); \
             im.Image vazio = im.Image.vazia(); \
             print(dobra(quadro).area() + vazio.largura + im.Config.padrao()); }",
        ),
        ("imagem.dart", IMAGEM),
    ]);
    let js = javascript(&fixture);
    assert!(js.contains("export function main"), "{js}");
}

/// Dois imports com o mesmo prefixo compõem um único namespace, como em Dart.
#[test]
fn dois_imports_com_o_mesmo_prefixo_compoem() {
    let fixture = Fixture::new(&[
        (
            "main.dart",
            "import 'a.dart' as p; import 'b.dart' as p; \
             void main() { print(p.daA() + p.daB()); }",
        ),
        ("a.dart", "int daA() { return 1; }"),
        ("b.dart", "int daB() { return 2; }"),
    ]);
    assert!(javascript(&fixture).contains("export function main"));
}

/// O mesmo nome sob o mesmo prefixo, de origens diferentes, é ambíguo.
#[test]
fn mesmo_nome_no_mesmo_prefixo_e_ambiguo() {
    let fonte = "import 'a.dart' as p; import 'b.dart' as p; void main() { print(p.valor()); }";
    let fixture = Fixture::new(&[
        ("main.dart", fonte),
        ("a.dart", "int valor() { return 1; }"),
        ("b.dart", "int valor() { return 2; }"),
    ]);
    rejeita(
        &fixture,
        "main.dart",
        fonte,
        "import ambíguo: p.valor; use show/hide para desambiguar",
        "import 'b.dart' as p;",
    );
}

/// `show` e `hide` compõem com o prefixo e continuam filtrando o namespace.
#[test]
fn prefixo_combina_com_show_e_hide() {
    let fixture = Fixture::new(&[
        (
            "main.dart",
            "import 'a.dart' as p show visivel; \
             void main() { print(p.visivel()); }",
        ),
        (
            "a.dart",
            "int visivel() { return 1; } int filtrado() { return 2; }",
        ),
    ]);
    assert!(javascript(&fixture).contains("export function main"));
    let fonte = "import 'a.dart' as p hide filtrado; void main() { print(p.filtrado()); }";
    let escondido = Fixture::new(&[
        ("main.dart", fonte),
        (
            "a.dart",
            "int visivel() { return 1; } int filtrado() { return 2; }",
        ),
    ]);
    rejeita(
        &escondido,
        "main.dart",
        fonte,
        "nome não exportado pela biblioteca importada com prefixo p: filtrado",
        "p.filtrado()",
    );
}

/// Privacidade por biblioteca continua valendo: `_nome` não atravessa o prefixo.
#[test]
fn privado_nao_atravessa_nem_com_prefixo() {
    let fonte = "import 'imagem.dart' as im; void main() { print(im._oculto()); }";
    let fixture = Fixture::new(&[("main.dart", fonte), ("imagem.dart", IMAGEM)]);
    rejeita(
        &fixture,
        "main.dart",
        fonte,
        "nome não exportado pela biblioteca importada com prefixo im: _oculto",
        "im._oculto()",
    );
}

/// Um prefixo não é identificador: `p` sozinho, sem ponto, é erro.
#[test]
fn prefixo_sem_ponto_e_recusado() {
    let fonte = "import 'a.dart' as p; void main() { print(p); }";
    let fixture = Fixture::new(&[("main.dart", fonte), ("a.dart", "int valor() { return 1; }")]);
    let error = compile_path(&fixture.entry(), Optimization::None).unwrap_err();
    assert_eq!(error.message, "prefixo de import p exige um nome: use p.nome");
    // O intervalo é o do identificador sozinho, não o da chamada que o contém.
    let start = fonte.find("print(p)").expect("chamada") + "print(".len();
    assert_eq!(error.span, Some(Span { start, end: start + 1 }));
}

/// Um nome de topo com o mesmo texto de um prefixo não pode ser confundido com ele.
#[test]
fn nome_de_topo_nao_se_confunde_com_prefixo() {
    let fonte = "import 'a.dart' as p; int p() { return 0; } void main() { print(p()); }";
    let fixture = Fixture::new(&[("main.dart", fonte), ("a.dart", "int valor() { return 1; }")]);
    rejeita(
        &fixture,
        "main.dart",
        fonte,
        "prefixo de import p colide com um nome visível nesta biblioteca",
        "import 'a.dart' as p;",
    );
}

/// `deferred as` é recusado por um diagnóstico próprio, não como prefixo comum.
#[test]
fn deferred_as_tem_diagnostico_proprio() {
    let fonte = "import 'a.dart' deferred as p; void main() { print(p.valor()); }";
    let fixture = Fixture::new(&[("main.dart", fonte), ("a.dart", "int valor() { return 1; }")]);
    rejeita(
        &fixture,
        "main.dart",
        fonte,
        "carregamento diferido não é suportado: remova deferred, a biblioteca importada é ligada estaticamente",
        "deferred",
    );
}

/// Uma biblioteca `dart:` ausente é nomeada, em vez de falhar por URI genérica.
#[test]
fn biblioteca_dart_ausente_e_nomeada() {
    for (uri, biblioteca) in [
        ("dart:math", "math"),
        ("dart:typed_data", "typed_data"),
        ("dart:convert", "convert"),
        ("dart:collection", "collection"),
    ] {
        let fonte = format!("import '{uri}' as m; void main() {{}}");
        let fixture = Fixture::new(&[("main.dart", fonte.as_str())]);
        rejeita(
            &fixture,
            "main.dart",
            &fonte,
            &format!(
                "biblioteca dart:{biblioteca} ainda não existe neste subconjunto; só dart:core, dart:async e dart:ffi são reconhecidas"
            ),
            &format!("import '{uri}' as m;"),
        );
    }
}

/// Um pacote ausente é nomeado junto do arquivo de configuração consultado.
#[test]
fn pacote_ausente_nomeia_pacote_e_caminho() {
    let fonte = "import 'package:vector_math/vector_math.dart' as vm; void main() {}";
    let fixture = Fixture::new(&[
        ("main.dart", fonte),
        (
            ".dart_tool/package_config.json",
            r#"{"configVersion":2,"packages":[]}"#,
        ),
    ]);
    let error = compile_path(&fixture.entry(), Optimization::None).unwrap_err();
    let consultado = std::fs::canonicalize(fixture.0.join(".dart_tool/package_config.json"))
        .unwrap()
        .display()
        .to_string();
    assert_eq!(
        error.message,
        format!("pacote vector_math não encontrado na configuração de pacotes consultada: {consultado}")
    );
    assert_eq!(
        error.span,
        Some(intervalo(
            fonte,
            "import 'package:vector_math/vector_math.dart' as vm;"
        ))
    );
}

/// Sem `package_config.json`, o diagnóstico diz onde o arquivo foi procurado.
#[test]
fn pacote_sem_configuracao_informa_onde_procurou() {
    let fonte = "import 'package:vector_math/vector_math.dart' as vm; void main() {}";
    let fixture = Fixture::new(&[("main.dart", fonte)]);
    let error = compile_path(&fixture.entry(), Optimization::None).unwrap_err();
    assert!(
        error
            .message
            .starts_with("pacote vector_math não encontrado: nenhuma configuração de pacotes foi encontrada a partir de "),
        "{}",
        error.message
    );
    assert!(
        error.message.ends_with("package_config.json"),
        "{}",
        error.message
    );
}

/// O prefixo alcança o namespace exportado, inclusive por reexport transitivo.
#[test]
fn reexport_transitivo_atravessa_o_prefixo() {
    let fixture = Fixture::new(&[
        (
            "main.dart",
            "import 'fachada.dart' as f; void main() { print(f.folha() + f.Caixa(1).valor); }",
        ),
        ("fachada.dart", "export 'meio.dart';"),
        ("meio.dart", "export 'folha.dart' show folha, Caixa;"),
        (
            "folha.dart",
            "int folha() { return 7; } class Caixa { int valor; Caixa(this.valor); } \
             int naoExportado() { return 0; }",
        ),
    ]);
    assert!(javascript(&fixture).contains("export function main"));
    let fonte = "import 'fachada.dart' as f; void main() { print(f.naoExportado()); }";
    let filtrado = Fixture::new(&[
        ("main.dart", fonte),
        ("fachada.dart", "export 'meio.dart';"),
        ("meio.dart", "export 'folha.dart' show folha, Caixa;"),
        (
            "folha.dart",
            "int folha() { return 7; } class Caixa { int valor; Caixa(this.valor); } \
             int naoExportado() { return 0; }",
        ),
    ]);
    rejeita(
        &filtrado,
        "main.dart",
        fonte,
        "nome não exportado pela biblioteca importada com prefixo f: naoExportado",
        "f.naoExportado()",
    );
}

/// Uma parte compartilha os imports do declarante, então o prefixo vale nela.
#[test]
fn parte_compartilha_o_prefixo_do_declarante() {
    let fixture = Fixture::new(&[
        (
            "main.dart",
            "import 'imagem.dart' as im; part 'parte.dart'; \
             void main() { print(usaPrefixo().area()); }",
        ),
        (
            "parte.dart",
            "part of 'main.dart'; im.Image usaPrefixo() { return im.decodePng(4); }",
        ),
        ("imagem.dart", IMAGEM),
    ]);
    assert!(javascript(&fixture).contains("export function main"));
}

/// O nome importado com prefixo não fica visível sem qualificação.
#[test]
fn nome_com_prefixo_nao_e_alcancavel_sem_ele() {
    let fonte = "import 'a.dart' as p; void main() { print(valor()); }";
    let fixture = Fixture::new(&[("main.dart", fonte), ("a.dart", "int valor() { return 1; }")]);
    rejeita(
        &fixture,
        "main.dart",
        fonte,
        "função não visível nesta biblioteca: valor",
        "valor()",
    );
}

/// Um nome sem prefixo e outro com prefixo coexistem sem ambiguidade.
#[test]
fn prefixado_e_nao_prefixado_coexistem() {
    let fixture = Fixture::new(&[
        (
            "main.dart",
            "import 'a.dart'; import 'b.dart' as p; void main() { print(valor() + p.valor()); }",
        ),
        ("a.dart", "int valor() { return 1; }"),
        ("b.dart", "int valor() { return 2; }"),
    ]);
    assert!(javascript(&fixture).contains("export function main"));
}

/// A saída emitida executa com o namespace de cada biblioteca preservado.
#[test]
#[ignore = "requer Node.js no PATH"]
fn prefixos_executam_no_node() {
    let fixture = Fixture::new(&[
        (
            "main.dart",
            "import 'imagem.dart' as im; import 'a.dart' as p; import 'b.dart' as p; \
             im.Image dobra(im.Image entrada) { return im.Image(entrada.largura * 2); } \
             void main() { im.Image quadro = im.decodePng(3); \
             print(dobra(quadro).area()); \
             print(im.Image.vazia().largura); \
             print(im.Config.padrao()); \
             print(p.daA() + p.daB()); }",
        ),
        ("imagem.dart", IMAGEM),
        ("a.dart", "int daA() { return 10; }"),
        ("b.dart", "int daB() { return 32; }"),
    ]);
    assert_eq!(node(&javascript(&fixture)), "12\n0\n9\n42\n");
}

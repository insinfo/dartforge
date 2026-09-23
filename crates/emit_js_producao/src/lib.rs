//! Perfil de **produção** do backend JavaScript: um arquivo, com só o que o
//! mundo fechado alcança. O plano inteiro, com a referência estudada e as
//! medições que o motivam, está em `docs/JS-PRODUCAO.md`.
//!
//! O perfil de desenvolvimento (`crates/emit_js`) continua intacto: este crate
//! **consome** a saída dele. É de propósito — a trilha semântica é a mesma, e
//! qualquer construto que o desenvolvimento emita certo, a produção emite
//! certo, porque o texto é o mesmo texto.

pub mod alcance;
pub mod bundle;
pub mod sdk;
pub mod varredura;

/// O que a compilação de produção produziu, com os números que interessam ao
/// relatório (e que o `crates/diferencial` imprime).
pub struct Producao {
    /// O arquivo único.
    pub js: String,
    /// Módulos do perfil de desenvolvimento que entraram.
    pub modulos: usize,
    /// Ciclos de `import` encontrados (deveria ser vazio; ver `bundle::ordenar`).
    pub ciclos: Vec<String>,
    /// Tamanho do `dart_sdk.js` antes e depois da poda.
    pub sdk_antes: usize,
    pub sdk_depois: usize,
    /// Unidades do `dart_sdk.js`: total e vivas.
    pub sdk_unidades: usize,
    pub sdk_vivas: usize,
}

/// Opções do perfil.
#[derive(Clone, Copy)]
pub struct Opcoes {
    /// Podar o `dart_sdk.js` pelo alcance (etapa 2 do plano).
    pub podar_sdk: bool,
    /// Partir as classes do `dart_sdk.js` por membro (etapa 4 do plano).
    pub por_membro: bool,
}

impl Default for Opcoes {
    fn default() -> Self {
        Opcoes { podar_sdk: true, por_membro: true }
    }
}

/// Compila `entrada` no perfil de produção.
///
/// `dart_sdk_js` é o runtime do DDC (`runtime/ddc/dart_sdk.js`); ele é lido,
/// podado e **embutido** no resultado.
pub fn compilar(
    entrada: &std::path::Path,
    sdk_lib: Option<&std::path::Path>,
    packages: Option<&std::path::Path>,
    dart_sdk_js: &std::path::Path,
    op: Opcoes,
) -> Result<Producao, String> {
    let emitido = dartforge_emit_js::compilar(entrada, sdk_lib, packages)?;
    let sdk_texto = std::fs::read_to_string(dart_sdk_js)
        .map_err(|e| format!("{}: {e}", dart_sdk_js.display()))?;
    Ok(montar(&emitido, &sdk_texto, op))
}

/// Como [`compilar`], a partir de uma emissão de desenvolvimento já feita.
/// Separado para que o teste não precise de disco nem de SDK.
pub fn montar(emitido: &dartforge_emit_js::Emitido, sdk_texto: &str, op: Opcoes) -> Producao {
    let modulos: Vec<bundle::Modulo> = emitido
        .modulos
        .iter()
        // O `preambulo.js` do perfil de desenvolvimento existe só para dar
        // `self` ao `dart_sdk.js` no Node; no bundle isso é a primeira linha.
        .filter(|(p, _)| p != "preambulo.js")
        .map(|(p, t)| bundle::separar(p, t))
        .collect();
    let (modulos, ciclos) = bundle::ordenar(modulos);
    let entrada = bundle::entrada_do_mjs(&emitido.entrada, &modulos)
        .unwrap_or_else(|| "L$main".to_string());

    let sdk_antes = sdk_texto.len();
    let (sdk_podado, unidades, vivas) = if op.podar_sdk {
        let raizes = sdk::raizes_do_usuario(&modulos);
        sdk::podar(sdk_texto, &raizes, op.por_membro)
    } else {
        (bundle::sdk_sem_export(sdk_texto), 0, 0)
    };
    let sdk_depois = sdk_podado.len();

    let js = bundle::montar(&sdk_podado, &modulos, &entrada);
    debug_assert!(
        // Determinismo é requisito (`docs/PESQUISA-OTIMIZACAO.md` §11): o
        // `dartforge serve` recarrega por geração, e geração que muda à toa é
        // um defeito que o usuário vê. Em depuração, confere de graça que o
        // arquivo não depende da ordem interna de nada.
        bundle::montar(&sdk_podado, &modulos, &entrada) == js,
        "a montagem não é determinística"
    );
    Producao {
        js,
        modulos: modulos.len(),
        ciclos,
        sdk_antes,
        sdk_depois,
        sdk_unidades: unidades,
        sdk_vivas: vivas,
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn emitido() -> dartforge_emit_js::Emitido {
        dartforge_emit_js::Emitido {
            modulos: vec![
                (
                    "main.js".into(),
                    concat!(
                        "var L$main = Object.create(dart.library);\n",
                        "export { L$main as main };\n",
                        "import { core, dart } from './dart_sdk.js';\n",
                        "import { util as L$util } from './util.js';\n",
                        "L$main.main = function main() { core.print(L$util.dobro(21)); };\n",
                    )
                    .into(),
                ),
                (
                    "util.js".into(),
                    concat!(
                        "var L$util = Object.create(dart.library);\n",
                        "export { L$util as util };\n",
                        "import { dart } from './dart_sdk.js';\n",
                        "L$util.dobro = function dobro(x) { return x * 2; };\n",
                    )
                    .into(),
                ),
                ("preambulo.js".into(), "if (typeof self === 'undefined') globalThis.self = globalThis;\n".into()),
            ],
            entrada: "import { main as m } from './main.js';\nm.main();\n".into(),
        }
    }

    const SDK: &str = concat!(
        "var core = Object.create(dart.library);\n",
        "export { dart, core };\n",
        "core.print = function print(o) { console.log(o); };\n",
        "core.Morta = class Morta {};\n",
    );

    #[test]
    fn monta_um_arquivo_na_ordem_topologica() {
        let p = montar(&emitido(), SDK, Opcoes { podar_sdk: false, por_membro: false });
        assert!(p.ciclos.is_empty());
        assert_eq!(p.modulos, 2);
        // `util.js` não importa ninguém, então vem antes de `main.js`.
        let a = p.js.find("L$util.dobro").unwrap();
        let b = p.js.find("L$main.main").unwrap();
        assert!(a < b, "dependência tem de vir antes de quem a importa");
        // Nada de `import`/`export` no arquivo final.
        assert!(!p.js.contains("\nimport "), "sobrou import");
        assert!(!p.js.contains("\nexport "), "sobrou export");
        // Os namespaces são içados; os corpos ficam em IIFE.
        assert!(p.js.contains("var L$util = Object.create(dart.library);\n"));
        assert!(p.js.contains("(function () {"));
        assert!(p.js.trim_end().ends_with("L$main.main();"));
    }

    /// As bibliotecas continuam **separadas** depois do empacotamento: é a
    /// regra do topo de `docs/JS-PRODUCAO.md` (compartilhar representação,
    /// sim; unificar identidade de biblioteca, não).
    #[test]
    fn empacotar_nao_funde_bibliotecas() {
        let p = montar(&emitido(), SDK, Opcoes { podar_sdk: false, por_membro: false });
        assert_eq!(p.js.matches("Object.create(dart.library)").count(), 3, "core + as duas do usuário");
    }

    /// Determinismo (`docs/PESQUISA-OTIMIZACAO.md` §11): duas montagens das
    /// mesmas entradas dão o mesmo arquivo, byte a byte.
    #[test]
    fn montagem_e_deterministica() {
        let op = Opcoes::default();
        let a = montar(&emitido(), SDK, op);
        let b = montar(&emitido(), SDK, op);
        assert_eq!(a.js, b.js);
        assert_eq!(a.sdk_depois, b.sdk_depois);
    }

    /// A poda tira do runtime o que o programa não alcança, e mantém o que
    /// alcança — no arquivo único, não num `dart_sdk.js` ao lado.
    #[test]
    fn poda_o_runtime_embutido() {
        let com = montar(&emitido(), SDK, Opcoes { podar_sdk: true, por_membro: false });
        let sem = montar(&emitido(), SDK, Opcoes { podar_sdk: false, por_membro: false });
        assert!(com.js.contains("core.print = function"), "o que o programa usa fica");
        assert!(!com.js.contains("core.Morta"), "o que ele não usa sai");
        assert!(com.sdk_depois < sem.sdk_depois);
    }
}

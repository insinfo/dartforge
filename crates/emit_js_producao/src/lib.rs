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

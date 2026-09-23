//! Perfil de **produção** do backend JavaScript: um arquivo, com só o que o
//! mundo fechado alcança. O plano inteiro, com a referência estudada e as
//! medições que o motivam, está em `docs/JS-PRODUCAO.md`.
//!
//! Duas podas, uma para cada natureza de código (`docs/JS-PRODUCAO.md` §1.2):
//!
//! * **o código do usuário e dos pacotes** é podado **antes** da emissão, pelo
//!   mundo fechado calculado sobre o modelo de elementos (`crates/mundo`); o
//!   emissor de desenvolvimento (`crates/emit_js`) recebe o mundo como filtro
//!   e não emite o que está morto. Sem filtro ele é byte a byte o de sempre;
//! * **o `dart_sdk.js`**, que já vem compilado pelo DDC, é podado sobre o
//!   texto (`sdk.rs`), com as raízes lidas do texto do usuário já podado.
//!
//! O texto emitido é conferido contra o mundo (`verificar.rs`): o que o texto
//! cita e o mundo podou volta como raiz, e o mundo é recalculado.

pub mod alcance;
pub mod bundle;
pub mod filtro;
pub mod sdk;
pub mod varredura;
pub mod verificar;

use std::time::{Duration, Instant};

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
    /// Bytes dos módulos do usuário e dos pacotes que entraram no arquivo.
    pub usuario: usize,
    /// O mundo fechado do usuário, quando calculado.
    pub mundo: Option<RelatorioMundo>,
}

/// Números do mundo fechado do usuário (`docs/PESQUISA-OTIMIZACAO.md` §10:
/// um relatório, não só um número).
#[derive(Debug, Clone, Default)]
pub struct RelatorioMundo {
    pub estat: dartforge_mundo::Estatisticas,
    /// Cálculo do mundo (todas as rodadas).
    pub tempo_mundo: Duration,
    /// Emissão com o filtro (todas as rodadas).
    pub tempo_emissao: Duration,
    /// Verificador do texto (todas as rodadas) — custo do verificador sempre ligado.
    pub tempo_verificacao: Duration,
    /// Rodadas mundo→emissão→verificação até o ponto fixo (1 = nada faltou).
    pub rodadas: usize,
    /// O que o verificador achou faltando (lacunas da análise).
    pub curas: Vec<String>,
    /// Referências pendentes sem elemento correspondente (deveria ser vazio).
    pub sem_elemento: Vec<String>,
    /// Inconsistências da conferência a seco (`DARTFORGE_JSPROD_CONFERIR=1`).
    pub inconsistencias: Option<usize>,
}

/// Opções do perfil.
#[derive(Clone, Copy)]
pub struct Opcoes {
    /// Podar o `dart_sdk.js` pelo alcance (etapa 2 do plano).
    pub podar_sdk: bool,
    /// Partir as classes do `dart_sdk.js` por membro (etapa 4 do plano).
    pub por_membro: bool,
    /// Podar o código do usuário pelo mundo fechado (etapa 5 do plano).
    pub podar_usuario: bool,
    /// Modo verificador: o que o mundo diz morto é emitido como *stub* que
    /// denuncia a chamada (`DARTFORGE-PODADO: …` no stderr, saída 97).
    pub stub: bool,
}

impl Default for Opcoes {
    fn default() -> Self {
        let stub = std::env::var("DARTFORGE_JSPROD_VERIFICAR").is_ok_and(|v| v == "stub");
        Opcoes { podar_sdk: true, por_membro: true, podar_usuario: true, stub }
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
    let sdk_texto = std::fs::read_to_string(dart_sdk_js).map_err(|e| format!("{}: {e}", dart_sdk_js.display()))?;
    let ((emitido, rel), _) = dartforge_emit_js::compilar_com(entrada, sdk_lib, packages, |a| emitir_com_mundo(a, &sdk_texto, op))?;
    let mut p = montar(&emitido, &sdk_texto, op);
    p.mundo = rel;
    Ok(p)
}

/// Raízes do mundo do usuário vistas do JS: `main` e os nomes que o
/// `dart_sdk.js` chama por string (`sdk::seletores_dinamicos`).
fn raizes(a: &dartforge_emit_js::Analise<'_>, sdk_texto: &str) -> dartforge_mundo::Raizes {
    use dartforge_elements::model::Element;
    let mut r = dartforge_mundo::Raizes::default();
    if let (Some(lib), Some(sym)) = (a.program.entry, a.interner.lookup("main")) {
        if let Some(Element::Function(f)) = a.program.library(lib).declared.get(&sym).and_then(|b| b.getter) {
            r.funcoes.push(f);
        }
    }
    r.seletores = sdk::seletores_dinamicos(sdk_texto);
    r
}

/// Mundo fechado → emissão filtrada → verificação do texto, até o ponto fixo.
pub fn emitir_com_mundo(a: &dartforge_emit_js::Analise<'_>, sdk_texto: &str, op: Opcoes) -> Result<(dartforge_emit_js::Emitido, Option<RelatorioMundo>), String> {
    if !op.podar_usuario {
        return Ok((a.emitir(None)?, None));
    }
    let entrada = dartforge_mundo::Entrada { program: a.program, interner: a.interner, table: a.table, outline: a.outline, bodies: a.bodies };
    let mut rel = RelatorioMundo::default();
    let t = Instant::now();
    let mut raizes = raizes(a, sdk_texto);
    rel.tempo_mundo += t.elapsed();
    // `DARTFORGE_JSPROD_RODADAS=1` mostra as lacunas da análise sem curá-las.
    let max_rodadas: usize = std::env::var("DARTFORGE_JSPROD_RODADAS").ok().and_then(|v| v.parse().ok()).unwrap_or(8).max(1);
    for rodada in 1..=max_rodadas {
        let t = Instant::now();
        let mundo = dartforge_mundo::calcular(entrada, &raizes);
        rel.tempo_mundo += t.elapsed();
        let ad = filtro::Adaptador { mundo: &mundo, program: a.program, stub: op.stub };
        let t = Instant::now();
        let emitido = a.emitir(Some(&ad))?;
        rel.tempo_emissao += t.elapsed();
        let t = Instant::now();
        // Identificador JS → biblioteca, para traduzir `L$x.Nome` em elemento.
        let libs = verificar::bibliotecas_do_texto(&emitido.modulos, a.program);
        let faltas = verificar::conferir(&emitido.modulos, &libs, a.program, a.interner, &mundo);
        rel.tempo_verificacao += t.elapsed();
        rel.rodadas = rodada;
        rel.sem_elemento = faltas.sem_elemento.clone();
        if faltas.vazia() || rodada == max_rodadas {
            if !faltas.vazia() {
                rel.curas.extend(faltas.descricao.iter().cloned());
                rel.curas.push(format!("ponto fixo não atingido em {max_rodadas} rodada(s)"));
            }
            rel.estat = mundo.estat.clone();
            if std::env::var("DARTFORGE_JSPROD_CONFERIR").is_ok_and(|v| v != "0") {
                let inc = dartforge_mundo::conferir(entrada, &raizes, &mundo);
                for i in inc.iter().take(10) {
                    eprintln!("[jsprod] inconsistência do mundo: {i:?}");
                }
                rel.inconsistencias = Some(inc.len());
            }
            if let Ok(alvo) = std::env::var("DARTFORGE_JSPROD_POR") {
                explicar(a, &mundo, &alvo);
            }
            return Ok((emitido, Some(rel)));
        }
        rel.curas.extend(faltas.descricao.iter().cloned());
        raizes.funcoes.extend(faltas.funcoes);
        raizes.variaveis.extend(faltas.variaveis);
        raizes.classes_tipo.extend(faltas.classes);
        raizes.classes_instanciadas.extend(faltas.classes_instanciadas);
        raizes.tearoffs.extend(faltas.tearoffs);
        raizes.seletores.extend(faltas.seletores);
    }
    unreachable!()
}

/// `DARTFORGE_JSPROD_POR=Nome`: por que a classe (ou função) `Nome` está no
/// arquivo — a cadeia de causas até a raiz.
fn explicar(a: &dartforge_emit_js::Analise<'_>, mundo: &dartforge_mundo::Mundo, alvo: &str) {
    use dartforge_elements::model::{ClassId, FunctionElementId};
    use dartforge_mundo::Causa;
    let p = a.program;
    let nome_fn = |f: FunctionElementId| {
        let func = p.function(f);
        let c = func.class.map(|c| format!("{}.", a.interner.resolve(p.class(c).name))).unwrap_or_default();
        format!("{}::{c}{}", p.library(func.library).uri, a.interner.resolve(func.name))
    };
    let mut inicio: Vec<Causa> = Vec::new();
    for (i, c) in p.classes.iter().enumerate() {
        if a.interner.resolve(c.name) == alvo && !p.library(c.library).is_sdk {
            inicio.push(Causa::Classe(ClassId(i as u32)));
        }
    }
    for (i, f) in p.functions.iter().enumerate() {
        if a.interner.resolve(f.name) == alvo && !p.library(f.library).is_sdk && mundo.funcao(FunctionElementId(i as u32)) {
            inicio.push(Causa::Funcao(FunctionElementId(i as u32)));
        }
    }
    for c in inicio {
        let mut cadeia: Vec<String> = Vec::new();
        let mut cur = Some(c);
        while let Some(x) = cur {
            if cadeia.len() > 40 {
                break;
            }
            let (txt, prox) = match x {
                Causa::Raiz => ("(raiz)".to_string(), None),
                Causa::Funcao(f) => (nome_fn(f), mundo.causa_funcao(f)),
                Causa::Classe(k) => (format!("classe {} [{:?}]", a.interner.resolve(p.class(k).name), mundo.classe(k)), mundo.causa_classe(k)),
                Causa::Variavel(v) => (format!("variável {}", a.interner.resolve(p.variable(v).name)), mundo.causa_variavel(v)),
            };
            cadeia.push(txt);
            cur = prox;
        }
        eprintln!("[jsprod] {}", cadeia.join(" <- "));
    }
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
    let usuario = modulos.iter().map(|m| m.corpo.len() + m.namespaces.iter().map(|n| n.len() + 1).sum::<usize>()).sum();

    let preambulo = if op.stub { bundle::PREAMBULO_STUB } else { "" };
    let js = bundle::montar(&sdk_podado, &modulos, &entrada, preambulo);
    debug_assert!(
        // Determinismo é requisito (`docs/PESQUISA-OTIMIZACAO.md` §11): o
        // `dartforge serve` recarrega por geração, e geração que muda à toa é
        // um defeito que o usuário vê. Em depuração, confere de graça que o
        // arquivo não depende da ordem interna de nada.
        bundle::montar(&sdk_podado, &modulos, &entrada, preambulo) == js,
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
        usuario,
        mundo: None,
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
        let p = montar(&emitido(), SDK, Opcoes { podar_sdk: false, por_membro: false, podar_usuario: false, stub: false });
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
        let p = montar(&emitido(), SDK, Opcoes { podar_sdk: false, por_membro: false, podar_usuario: false, stub: false });
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
        let com = montar(&emitido(), SDK, Opcoes { podar_sdk: true, por_membro: false, podar_usuario: false, stub: false });
        let sem = montar(&emitido(), SDK, Opcoes { podar_sdk: false, por_membro: false, podar_usuario: false, stub: false });
        assert!(com.js.contains("core.print = function"), "o que o programa usa fica");
        assert!(!com.js.contains("core.Morta"), "o que ele não usa sai");
        assert!(com.sdk_depois < sem.sdk_depois);
    }
}

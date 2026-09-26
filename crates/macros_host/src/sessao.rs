//! A sessão de macros de uma compilação: as três fases, na ordem do CFE
//! 3.6.2, com o programa recarregado entre elas (docs/MACROS-PROTOCOLO.md
//! §4).
//!
//! * fase 1 (tipos): todas as aplicações; o texto parcial entra no programa
//!   antes da fase 2;
//! * fase 2 (declarações): cada resultado não vazio entra no programa antes
//!   da aplicação seguinte (o CFE cria uma biblioteca de augmentation por
//!   resultado: a próxima macro vê o que a anterior declarou);
//! * fase 3 (definições): todas; o texto final entra no programa.
//!
//! O texto de cada biblioteca é **um só**, montado de todos os resultados na
//! ordem de execução (Regras 1–4), e vive em memória como
//! `<biblioteca>.macro.dart` (`elements/src/gerado.rs`), anexado pelo
//! carregador depois das partes. É o mesmo texto que a materialização grava
//! (docs/MACROS-COMPATIBILIDADE.md).
//!
//! Com um [`CacheDeMacros`] ([`aplicar_incremental`]), cada `(aplicação,
//! fase)` é revalidada antes de ir ao executor, e o executor só é iniciado
//! se alguma aplicação precisar de fato rodar (`crate::cache`). Com
//! [`CarregarPorDiferenca`], cada recarga reaproveita as unidades já
//! analisadas e só analisa de novo a augmentation que mudou.
use crate::aplicacoes::{Alvo, Aplicacao, detectar, ordem};
use crate::cache::{
    CacheDeMacros, ChaveDeInstancia, Digest, Gravador, IdentidadeDaAplicacao, Revalidacao, digest_do_pedido, gravar,
    identidade_da_implementacao, podar, revalidar,
};
use crate::consultas::{Consultor, ResolvedorDoPrograma};
use crate::executor::{ErroDeConsulta, ExecutorMacros, Fase, PedidoDeExecucao, ServicoDeConsultas};
use crate::modelo::{Chave, Tabela, Vista};
use crate::montagem::{Forma, Resultado, montar};
use dartforge_diagnostics::Diagnostic;
use dartforge_elements::gerado::{Construtor, Geracao};
use dartforge_elements::model::Program;
use dartforge_elements::unidades::CacheUnidades;
use dartforge_intern::Interner;
use serde_json::{Value, json};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

static SESSOES: AtomicUsize = AtomicUsize::new(0);

/// Quantas sessões de macros este processo abriu — o portão de custo zero
/// exige 0 num programa sem aplicação de macro (como
/// `dartforge_build::instancias`).
pub fn sessoes() -> usize {
    SESSOES.load(Ordering::Relaxed)
}

/// Recarrega o programa com as fontes geradas dadas.
pub type Carregar<'c> = dyn FnMut(&mut Interner, Option<Arc<Geracao>>) -> (Program, Vec<Diagnostic>) + 'c;

/// Recarrega o programa com as fontes geradas dadas, reaproveitando as
/// unidades guardadas no [`CacheUnidades`] (o de `dartforge_elements`, o
/// mesmo da sessão residente do `dartforge dev`): a sessão de macros guarda
/// ali as unidades do programa anterior, tira a augmentation de macro (que
/// mudou) e o carregador só analisa de novo o que não está guardado. O
/// `Interner` é sempre o mesmo da sessão, condição do cache de unidades.
pub type CarregarPorDiferenca<'c> =
    dyn FnMut(&mut Interner, Option<Arc<Geracao>>, &mut CacheUnidades) -> (Program, Vec<Diagnostic>) + 'c;

/// O texto montado de uma biblioteca.
#[derive(Debug, Clone)]
pub struct TextoGerado {
    pub biblioteca: String,
    /// `<biblioteca>.macro.dart`, ao lado dela.
    pub caminho: PathBuf,
    pub texto: String,
}

pub struct Saida {
    pub program: Program,
    pub geracao: Option<Arc<Geracao>>,
    pub textos: Vec<TextoGerado>,
    /// Execuções de fato enviadas ao executor (as reaproveitadas do cache
    /// não contam).
    pub macros_executadas: usize,
    /// Avisos e informações reportados pelas macros.
    pub avisos: Vec<Diagnostic>,
    /// As expansões, na ordem em que entraram no programa — executadas ou
    /// reaproveitadas. A compilação incremental tem de repetir a da limpa.
    pub expansoes: Vec<Expansao>,
    /// Onde a sessão gastou o tempo (docs/MACROS-PROTOCOLO.md §8).
    pub medicao: Medicao,
}

/// Uma aplicação de macro numa fase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expansao {
    pub aplicacao: IdentidadeDaAplicacao,
    pub fase: Fase,
    /// Veio do cache, sem falar com o executor.
    pub reutilizada: bool,
}

/// O tempo de uma sessão de macros, separado por etapa, e os contadores que
/// explicam cada parcela. Mede desempenho, não correção: nenhum resultado
/// depende destes números.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Medicao {
    /// Handshake e `macro.instanciar`.
    pub inicio: Duration,
    /// O modelo do pedido `macro.executar` (alvo e pré-busca dos membros).
    pub modelo: Duration,
    /// `macro.executar` até o `macro.resultado`, **sem** o tempo servindo
    /// consultas: o executor rodando a macro, mais o transporte.
    pub execucao: Duration,
    /// Servindo `macro.consulta` no hospedeiro.
    pub consultas: Duration,
    /// Revalidando o cache: impressão digital do pedido e as consultas
    /// gravadas refeitas contra o programa corrente.
    pub revalidacao: Duration,
    /// Montagem dos textos de augmentation (todas as vezes).
    pub montagem: Duration,
    /// Recarga do programa com a augmentation (todas as vezes).
    pub recarga: Duration,
    pub execucoes: usize,
    pub n_consultas: usize,
    /// Expansões que vieram do cache.
    pub reutilizadas: usize,
    /// Consultas refeitas na revalidação.
    pub consultas_revalidadas: usize,
    /// Expansões que rodaram de novo e deram o mesmo resultado de antes (as
    /// seguintes continuam valendo: o programa que elas veem não muda).
    pub reexecutadas_iguais: usize,
    /// O executor foi iniciado nesta sessão.
    pub executor_iniciado: bool,
    /// Quantas vezes a montagem rodou e quantos resultados ela percorreu no
    /// total: a remontagem acumulada da fase 2 faz este número crescer com o
    /// quadrado das aplicações.
    pub montagens: usize,
    pub resultados_montados: usize,
    pub recargas: usize,
    /// Unidades que as recargas reaproveitaram em vez de analisar de novo.
    pub unidades_reaproveitadas: usize,
}

impl Medicao {
    /// Uma linha legível, para relatórios e para o teste de medição.
    pub fn resumo(&self) -> String {
        let ms = |d: Duration| d.as_secs_f64() * 1000.0;
        format!(
            "início {:.1} ms{}; modelo {:.1} ms; execução {:.1} ms ({} execuções); consultas {:.1} ms ({}); \
             revalidação {:.1} ms ({} reaproveitadas, {} consultas refeitas, {} reexecutadas iguais); \
             montagem {:.1} ms ({} montagens, {} resultados); recarga {:.1} ms ({} recargas, {} unidades reaproveitadas)",
            ms(self.inicio),
            if self.executor_iniciado { "" } else { " (executor não iniciado)" },
            ms(self.modelo),
            ms(self.execucao),
            self.execucoes,
            ms(self.consultas),
            self.n_consultas,
            ms(self.revalidacao),
            self.reutilizadas,
            self.consultas_revalidadas,
            self.reexecutadas_iguais,
            ms(self.montagem),
            self.montagens,
            self.resultados_montados,
            ms(self.recarga),
            self.recargas,
            self.unidades_reaproveitadas
        )
    }
}

/// Mede o tempo servindo consultas em torno de outro serviço.
struct Cronometrado<'s> {
    interno: &'s mut dyn ServicoDeConsultas,
    tempo: Duration,
    n: usize,
}

impl ServicoDeConsultas for Cronometrado<'_> {
    fn consultar(&mut self, tipo: &str, args: &Value) -> Result<Value, ErroDeConsulta> {
        let t = Instant::now();
        let r = self.interno.consultar(tipo, args);
        self.tempo += t.elapsed();
        self.n += 1;
        r
    }
}

/// A macro implementa a interface da fase para o tipo do alvo? (a resposta
/// do `macro.instanciar` traz os nomes das interfaces).
fn aplica_na_fase(interfaces: &[String], alvo: &Alvo, fase: Fase) -> bool {
    let tipo = match alvo {
        Alvo::Biblioteca(_) => "Library",
        Alvo::Declaracao(Chave::Tipo { .. }) => "Class",
        Alvo::Declaracao(Chave::Metodo { .. }) => "Method",
        Alvo::Declaracao(Chave::Construtor { .. }) => "Constructor",
        Alvo::Declaracao(Chave::Campo { .. }) => "Field",
        Alvo::Declaracao(Chave::FuncaoDeTopo { .. }) => "Function",
        Alvo::Declaracao(Chave::VariavelDeTopo { .. }) => "Variable",
        _ => return false,
    };
    let sufixo = match fase {
        Fase::Tipos => "TypesMacro",
        Fase::Declaracoes => "DeclarationsMacro",
        Fase::Definicoes => "DefinitionMacro",
    };
    // Mixin e enum também são `Chave::Tipo`; o executor escolhe o método
    // pelo tipo real do alvo.
    let alternativas: &[&str] = match tipo {
        "Class" => &["Mixin", "Enum"],
        _ => &[],
    };
    interfaces
        .iter()
        .any(|i| *i == format!("{tipo}{sufixo}") || alternativas.iter().any(|a| *i == format!("{a}{sufixo}")))
}

fn nao_vazio(r: &Resultado) -> bool {
    !(r.valores_de_enum.is_empty()
        && r.extends.is_empty()
        && r.interfaces.is_empty()
        && r.biblioteca.is_empty()
        && r.mixins.is_empty()
        && r.tipos.is_empty())
}

fn diagnostico(app: &Aplicacao, msg: &str) -> Diagnostic {
    let classe = app.macro_.rsplit('#').next().unwrap_or("");
    Diagnostic::new(format!("{}: aplicação de @{classe}: {msg}", app.unidade), app.span)
}

/// Os textos montados de `resultados` (agrupados por biblioteca, na ordem
/// de execução) e a geração que os contém, mais as fontes de `base`.
fn gerar(
    resultados: &[(String, Resultado)],
    tabela: &mut Tabela,
    program: &Program,
    interner: &Interner,
    base: Option<&Arc<Geracao>>,
) -> Result<(Vec<TextoGerado>, Arc<Geracao>), String> {
    let mut por_biblioteca: Vec<(String, Vec<Resultado>)> = Vec::new();
    for (lib, r) in resultados {
        match por_biblioteca.iter_mut().find(|(l, _)| l == lib) {
            Some((_, v)) => v.push(r.clone()),
            None => por_biblioteca.push((lib.clone(), vec![r.clone()])),
        }
    }
    let vista = Vista { program, interner };
    let celula = RefCell::new(tabela);
    let resolvedor = ResolvedorDoPrograma { vista: &vista, tabela: &celula };
    let mut textos = Vec::new();
    for (lib, rs) in por_biblioteca {
        let texto = montar(&rs, &resolvedor, Forma::BibliotecaDeAugmentation(&lib))?;
        let lib_id = vista.biblioteca_por_uri(&lib).ok_or_else(|| format!("biblioteca {lib} sumiu do programa"))?;
        let principal = program.library(lib_id).units.first().and_then(|u| program.unit(*u).path.clone());
        let principal = principal.ok_or_else(|| format!("biblioteca {lib} sem arquivo"))?;
        let caminho = dartforge_elements::load::caminho_da_augmentation_de_macro(&principal);
        textos.push(TextoGerado { biblioteca: lib, caminho, texto });
    }
    let mut c = Construtor::nova();
    if let Some(b) = base {
        for (p, f) in b.iter() {
            c.por(p.clone(), f.conteudo.clone(), f.gerador, f.entradas.to_vec());
        }
    }
    for t in &textos {
        c.por(t.caminho.clone(), t.texto.as_str(), "macros", vec![]);
    }
    let id = base.map_or(0, |b| b.id) + 1;
    let g = c.concluir(id).map_err(|e| e.join("; "))?;
    Ok((textos, g))
}

/// Aplicações que ainda exigem execução; bibliotecas com augmentation
/// materializada já têm seus membros no programa carregado.
pub fn aplicacoes_pendentes(program: &Program, interner: &Interner) -> Vec<Aplicacao> {
    detectar(&Vista { program, interner })
        .into_iter()
        .filter(|a| !ja_materializada(program, &a.biblioteca))
        .collect()
}

/// Aplica as macros de `program` com `executor`. Sem aplicação, devolve o
/// programa como veio — sem abrir sessão (custo zero). Com erro (executor
/// indisponível, diagnóstico de erro de uma macro, texto que não compila),
/// devolve os diagnósticos, cada um apontando a anotação.
///
/// Sem cache e com recarga completa; [`aplicar_incremental`] é a forma com
/// cache de expansões e recarga por diferença.
pub fn aplicar(
    program: Program,
    interner: &mut Interner,
    base: Option<Arc<Geracao>>,
    carregar: &mut Carregar<'_>,
    executor: &mut dyn ExecutorMacros,
) -> Result<Saida, Vec<Diagnostic>> {
    let mut cache = CacheDeMacros::desligado();
    sessao(program, interner, base, Recarga::Completa(carregar), executor, &mut cache)
}

/// Como [`aplicar`], com a recarga por diferença ([`CarregarPorDiferenca`])
/// e, se dado, o cache de expansões: o que a compilação anterior executou e
/// continua valendo é reaproveitado sem falar com o executor, que só é
/// iniciado se alguma aplicação precisar rodar. O resultado — textos, ordem
/// das expansões e diagnósticos — é o de uma compilação limpa.
///
/// # Erros
///
/// Os mesmos de [`aplicar`].
pub fn aplicar_incremental(
    program: Program,
    interner: &mut Interner,
    base: Option<Arc<Geracao>>,
    carregar: &mut CarregarPorDiferenca<'_>,
    executor: &mut dyn ExecutorMacros,
    cache: Option<&mut CacheDeMacros>,
) -> Result<Saida, Vec<Diagnostic>> {
    let mut desligado;
    let cache = match cache {
        Some(c) => c,
        None => {
            desligado = CacheDeMacros::desligado();
            &mut desligado
        }
    };
    sessao(program, interner, base, Recarga::PorDiferenca(carregar, CacheUnidades::nova()), executor, cache)
}

/// Como a sessão recarrega o programa.
enum Recarga<'r, 'c> {
    /// Carga inteira a cada vez.
    Completa(&'r mut Carregar<'c>),
    /// Reaproveitando as unidades do programa anterior.
    PorDiferenca(&'r mut CarregarPorDiferenca<'c>, CacheUnidades),
}

impl Recarga<'_, '_> {
    /// Troca `program` pelo programa com a geração `g`; `textos` são as
    /// augmentations de macro, que mudaram e não podem ser reaproveitadas.
    fn recarregar(
        &mut self,
        program: &mut Program,
        interner: &mut Interner,
        g: Arc<Geracao>,
        textos: &[TextoGerado],
        medicao: &mut Medicao,
    ) -> Vec<Diagnostic> {
        let (p, d) = match self {
            Recarga::Completa(carregar) => carregar(interner, Some(g)),
            Recarga::PorDiferenca(carregar, unidades) => {
                unidades.recolher(std::mem::take(program));
                for t in textos {
                    unidades.invalidar(&t.caminho);
                }
                let r = carregar(interner, Some(g), unidades);
                medicao.unidades_reaproveitadas += unidades.reaproveitadas;
                r
            }
        };
        *program = p;
        d
    }
}

/// O executor, iniciado só quando alguma aplicação precisa dele.
struct ExecutorPreguicoso<'e> {
    executor: &'e mut dyn ExecutorMacros,
    iniciado: bool,
    /// Uma instância por (macro, construtor, argumentos), como o CFE.
    instancias: HashMap<ChaveDeInstancia, (u64, Vec<String>)>,
}

impl ExecutorPreguicoso<'_> {
    /// Handshake e uma instância por chave distinta, na ordem de `apps` (a
    /// mesma sequência de mensagens de sempre numa compilação limpa).
    fn ligar(&mut self, apps: &[Aplicacao], chaves: &[ChaveDeInstancia], medicao: &mut Medicao) -> Result<(), Vec<Diagnostic>> {
        if self.iniciado {
            return Ok(());
        }
        let t = Instant::now();
        self.iniciado = true;
        medicao.executor_iniciado = true;
        if let Err(e) = self.executor.iniciar() {
            self.encerrar();
            return Err(apps.iter().map(|a| diagnostico(a, &e)).collect());
        }
        for (a, k) in apps.iter().zip(chaves) {
            if self.instancias.contains_key(k) {
                continue;
            }
            match self.executor.instanciar(&a.macro_, &a.construtor, &a.argumentos) {
                Ok(x) => {
                    self.instancias.insert(k.clone(), x);
                }
                Err(e) => {
                    self.encerrar();
                    return Err(vec![diagnostico(a, &e)]);
                }
            }
        }
        medicao.inicio += t.elapsed();
        Ok(())
    }

    fn encerrar(&mut self) {
        if self.iniciado {
            self.executor.encerrar();
            self.iniciado = false;
        }
    }
}

/// A sessão: as três fases sobre `program`, com `cache` (desligado numa
/// compilação sem cache).
fn sessao(
    program: Program,
    interner: &mut Interner,
    base: Option<Arc<Geracao>>,
    mut recarga: Recarga<'_, '_>,
    executor: &mut dyn ExecutorMacros,
    cache: &mut CacheDeMacros,
) -> Result<Saida, Vec<Diagnostic>> {
    // Biblioteca que já inclui a augmentation materializada (`import augment
    // 'x.macro.dart'` ou `part`, gravada por `dartforge macros
    // --materializar`) não roda as macros de novo: como um `.g.dart` no disco.
    let apps = aplicacoes_pendentes(&program, interner);
    if apps.is_empty() {
        return Ok(Saida {
            program,
            geracao: base,
            textos: Vec::new(),
            macros_executadas: 0,
            avisos: Vec::new(),
            expansoes: Vec::new(),
            medicao: Medicao::default(),
        });
    }
    SESSOES.fetch_add(1, Ordering::Relaxed);
    let mut medicao = Medicao::default();
    let CacheDeMacros { ativo, identidade_do_executor, tabela, estaticos, interfaces: interfaces_gravadas, registros } = cache;
    let ativo = *ativo;

    // A identidade estável de cada aplicação e a da instância dela; a
    // implementação entra pelo fonte do fecho da biblioteca da macro, lido
    // do programa ainda sem nenhuma augmentation de macro.
    let identidades = IdentidadeDaAplicacao::de_todas(&apps);
    let mut implementacoes: HashMap<&str, Digest> = HashMap::new();
    let chaves: Vec<ChaveDeInstancia> = apps
        .iter()
        .map(|a| {
            let uri = a.macro_.split_once('#').map_or(a.macro_.as_str(), |(u, _)| u);
            let implementacao = if ativo {
                *implementacoes.entry(uri).or_insert_with(|| identidade_da_implementacao(&program, uri))
            } else {
                [0; 32]
            };
            ChaveDeInstancia {
                implementacao,
                macro_: a.macro_.clone(),
                construtor: a.construtor.clone(),
                argumentos: a.argumentos.to_string(),
            }
        })
        .collect();

    let mut executor = ExecutorPreguicoso { executor, iniciado: false, instancias: HashMap::new() };
    let mut resultados: Vec<(String, Resultado)> = Vec::new();
    let mut erros: Vec<Diagnostic> = Vec::new();
    let mut avisos: Vec<Diagnostic> = Vec::new();
    let mut expansoes: Vec<Expansao> = Vec::new();
    let mut usadas: HashSet<(IdentidadeDaAplicacao, Fase)> = HashSet::new();
    let mut executadas = 0usize;
    let mut program = program;
    let mut geracao = base.clone();

    let mut recarregar = |program: &mut Program,
                          geracao: &mut Option<Arc<Geracao>>,
                          interner: &mut Interner,
                          tabela: &mut Tabela,
                          resultados: &[(String, Resultado)],
                          medicao: &mut Medicao|
     -> Result<Vec<TextoGerado>, Vec<Diagnostic>> {
        let t = Instant::now();
        let (textos, g) = gerar(resultados, tabela, program, interner, base.as_ref())
            .map_err(|e| vec![Diagnostic::new(format!("montagem da augmentation de macro: {e}"), dartforge_diagnostics::Span { start: 0, end: 0 })])?;
        medicao.montagem += t.elapsed();
        medicao.montagens += 1;
        medicao.resultados_montados += resultados.len();
        let t = Instant::now();
        let d = recarga.recarregar(program, interner, g.clone(), &textos, medicao);
        medicao.recarga += t.elapsed();
        medicao.recargas += 1;
        if !d.is_empty() {
            return Err(d);
        }
        *geracao = Some(g);
        Ok(textos)
    };

    let mut textos = Vec::new();
    for fase in Fase::TODAS {
        let sequencia = ordem(fase, &apps, &Vista { program: &program, interner });
        let mut mudou = false;
        for i in sequencia {
            let app = &apps[i];
            // As interfaces decidem as fases: do executor, se já está
            // iniciado; senão, do cache; senão, é hora de iniciá-lo.
            let interfaces = match executor.instancias.get(&chaves[i]) {
                Some((_, itf)) => itf.clone(),
                None => match interfaces_gravadas.get(&chaves[i]).filter(|_| ativo) {
                    Some(itf) => itf.clone(),
                    None => {
                        executor.ligar(&apps, &chaves, &mut medicao)?;
                        if ativo {
                            for (k, (_, itf)) in &executor.instancias {
                                interfaces_gravadas.insert(k.clone(), itf.clone());
                            }
                        }
                        executor.instancias[&chaves[i]].1.clone()
                    }
                },
            };
            if !aplica_na_fase(&interfaces, &app.alvo, fase) {
                continue;
            }
            let chave_do_registro = (identidades[i].clone(), fase);
            let resultado = {
                let vista = Vista { program: &program, interner };
                let t = Instant::now();
                let (alvo, modelo) = match alvo_e_modelo(&vista, tabela, &app.alvo) {
                    Ok(x) => x,
                    Err(e) => {
                        erros.push(diagnostico(app, &e));
                        continue;
                    }
                };
                medicao.modelo += t.elapsed();
                let pedido = ativo.then(|| {
                    let t = Instant::now();
                    let d = digest_do_pedido(identidade_do_executor, &chaves[i], fase, &alvo, &modelo);
                    medicao.revalidacao += t.elapsed();
                    d
                });
                // Revalidação: as consultas gravadas refeitas contra o
                // programa corrente.
                let reaproveitado = match &pedido {
                    Some(d) => {
                        let t = Instant::now();
                        let mut consultor = Consultor { vista: &vista, tabela: &mut *tabela, estaticos: &mut *estaticos };
                        let r = revalidar(registros, &chave_do_registro, d, &mut consultor);
                        medicao.revalidacao += t.elapsed();
                        match r {
                            Revalidacao::Valido { resultado, refeitas } => {
                                medicao.consultas_revalidadas += refeitas;
                                Some(*resultado)
                            }
                            Revalidacao::ConsultaMudou { refeitas } => {
                                medicao.consultas_revalidadas += refeitas;
                                None
                            }
                            Revalidacao::Ausente | Revalidacao::PedidoMudou => None,
                        }
                    }
                    None => None,
                };
                usadas.insert(chave_do_registro.clone());
                match reaproveitado {
                    Some(r) => {
                        medicao.reutilizadas += 1;
                        expansoes.push(Expansao { aplicacao: identidades[i].clone(), fase, reutilizada: true });
                        Ok(r)
                    }
                    None => {
                        executor.ligar(&apps, &chaves, &mut medicao)?;
                        let instancia = executor.instancias[&chaves[i]].0;
                        let pedido_de_execucao = PedidoDeExecucao { instancia, fase, alvo, modelo };
                        let mut consultor = Consultor { vista: &vista, tabela: &mut *tabela, estaticos: &mut *estaticos };
                        let mut cronometrado = Cronometrado { interno: &mut consultor, tempo: Duration::ZERO, n: 0 };
                        let mut gravador = Gravador::novo(&mut cronometrado, ativo);
                        executadas += 1;
                        let t = Instant::now();
                        let r = executor.executor.executar(&pedido_de_execucao, &mut gravador);
                        let decorrido = t.elapsed();
                        let consultas = std::mem::take(&mut gravador.consultas);
                        drop(gravador);
                        medicao.execucao += decorrido.saturating_sub(cronometrado.tempo);
                        medicao.consultas += cronometrado.tempo;
                        medicao.n_consultas += cronometrado.n;
                        if let (Ok(r), Some(d)) = (&r, pedido)
                            && gravar(registros, chave_do_registro, d, consultas, r)
                        {
                            medicao.reexecutadas_iguais += 1;
                        }
                        expansoes.push(Expansao { aplicacao: identidades[i].clone(), fase, reutilizada: false });
                        r
                    }
                }
            };
            let r = match resultado {
                Ok(r) => r,
                Err(e) => {
                    erros.push(diagnostico(app, &e));
                    continue;
                }
            };
            relatar(app, &r, &mut erros, &mut avisos);
            if nao_vazio(&r) {
                resultados.push((app.biblioteca.clone(), r));
                mudou = true;
                // Fase 2: a próxima aplicação vê o que esta declarou.
                if fase == Fase::Declaracoes && erros.is_empty() {
                    textos = match recarregar(&mut program, &mut geracao, interner, tabela, &resultados, &mut medicao) {
                        Ok(textos) => textos,
                        Err(diagnosticos) => {
                            executor.encerrar();
                            return Err(diagnosticos);
                        }
                    };
                    mudou = false;
                }
            }
        }
        if !erros.is_empty() {
            executor.encerrar();
            return Err(erros);
        }
        if mudou {
            textos = match recarregar(&mut program, &mut geracao, interner, tabela, &resultados, &mut medicao) {
                Ok(textos) => textos,
                Err(diagnosticos) => {
                    executor.encerrar();
                    return Err(diagnosticos);
                }
            };
        }
    }
    executor.encerrar();
    if ativo {
        podar(registros, interfaces_gravadas, &usadas, &chaves.iter().cloned().collect());
    }
    medicao.execucoes = executadas;
    Ok(Saida { program, geracao, textos, macros_executadas: executadas, avisos, expansoes, medicao })
}

/// O alvo em JSON e o modelo da pré-busca: a biblioteca do alvo e, para uma
/// classe, os membros dela (o `@JsonCodable` não faz ida e volta).
fn alvo_e_modelo(v: &Vista<'_>, t: &mut Tabela, alvo: &Alvo) -> Result<(Value, Value), String> {
    match alvo {
        Alvo::Biblioteca(uri) => {
            let lib = v.biblioteca_por_uri(uri).ok_or_else(|| format!("biblioteca {uri} não encontrada"))?;
            let b = v.biblioteca_json(t, lib);
            Ok((b.clone(), json!({"bibliotecas": [b]})))
        }
        Alvo::Declaracao(c) => {
            let d = v.declaracao_json(t, c).ok_or_else(|| format!("alvo {} não suportado ou não encontrado", c.nome()))?;
            let lib = match c {
                Chave::Tipo { lib, .. } | Chave::Metodo { lib, .. } | Chave::Campo { lib, .. } | Chave::Construtor { lib, .. }
                | Chave::FuncaoDeTopo { lib, .. } | Chave::VariavelDeTopo { lib, .. } => lib,
                _ => return Err(format!("alvo {} ainda não suportado", c.nome())),
            };
            let lib_id = v.biblioteca_por_uri(lib).ok_or_else(|| format!("biblioteca {lib} não encontrada"))?;
            let bibliotecas = json!([v.biblioteca_json(t, lib_id)]);
            let mut membros = serde_json::Map::new();
            let dono = match c {
                Chave::Tipo { .. } => Some(c.clone()),
                Chave::Metodo { lib, dono, .. } | Chave::Campo { lib, dono, .. } | Chave::Construtor { lib, dono, .. } => {
                    Some(Chave::Tipo { lib: lib.clone(), nome: dono.clone() })
                }
                _ => None,
            };
            if let Some(dono) = dono {
                if let Some(cid) = v.classe(&dono) {
                    let id = t.id(dono);
                    membros.insert(
                        id.to_string(),
                        json!({
                            "campos": v.membros_json(t, cid, "campos"),
                            "metodos": v.membros_json(t, cid, "metodos"),
                            "construtores": v.membros_json(t, cid, "construtores"),
                        }),
                    );
                }
            }
            Ok((d, json!({"bibliotecas": bibliotecas, "membros": membros})))
        }
    }
}

/// Diagnósticos e exceção de um resultado, apontando a anotação.
fn relatar(app: &Aplicacao, r: &Resultado, erros: &mut Vec<Diagnostic>, avisos: &mut Vec<Diagnostic>) {
    for d in &r.diagnosticos {
        let texto = d["mensagem"]["texto"].as_str().unwrap_or("");
        let correcao = d["correcao"].as_str().map(|c| format!(" ({c})")).unwrap_or_default();
        let contexto: Vec<&str> = d["contexto"].as_array().map(|l| l.iter().filter_map(|m| m["texto"].as_str()).collect()).unwrap_or_default();
        let mut msg = format!("{texto}{correcao}");
        for c in contexto {
            msg.push_str("\n  ");
            msg.push_str(c);
        }
        match d["severidade"].as_str() {
            Some("error") => erros.push(diagnostico(app, &msg)),
            _ => avisos.push(diagnostico(app, &msg)),
        }
    }
    if let Some(e) = &r.excecao {
        let tipo = e["tipo"].as_str().unwrap_or("");
        let msg = e["mensagem"].as_str().unwrap_or("");
        erros.push(diagnostico(app, &format!("exceção ({tipo}): {msg}")));
    }
}

/// A biblioteca `uri` já tem, entre as unidades carregadas do disco, a
/// augmentation materializada `<biblioteca>.macro.dart`?
fn ja_materializada(program: &Program, uri: &str) -> bool {
    let Some(lib) = program.libraries.iter().find(|l| l.uri == uri) else { return false };
    let Some(principal) = lib.units.first().and_then(|u| program.unit(*u).path.clone()) else { return false };
    let alvo = dartforge_elements::gerado::chave(&dartforge_elements::load::caminho_da_augmentation_de_macro(&principal));
    lib.units.iter().skip(1).any(|u| program.unit(*u).path.as_deref().map(dartforge_elements::gerado::chave).as_ref() == Some(&alvo))
}

//! Hot reload de comportamento: identidade estável, implementação por geração.
//!
//! O desenho é o do incremento 28 de `PLANO.md`, e a peça central é a separação
//! entre **quem é** uma função e **o que ela faz agora**:
//!
//! * a **entrada estável** é um trampolim que nunca é descarregado. Ela carrega o
//!   nome que o emissor produziu (`dartforge_entry`, `df_fn_0`, `df_method_0_0`)
//!   e é o único endereço que sai da sessão;
//! * a **implementação** é versionada por geração (`df_fn_0$gen3`) e vive sob um
//!   `ResourceTracker` próprio;
//! * a ligação entre as duas é uma **célula de ponteiro** pertencente ao Rust,
//!   publicada na `JITDylib` como símbolo absoluto de dado. O trampolim lê essa
//!   célula a cada chamada; recarregar é trocar o conteúdo dela.
//!
//! ```text
//!   caller  ──►  @df_fn_0            (trampoline, permanente)
//!                    │ load  @__dfslot$df_fn_0
//!                    ▼
//!                  célula ──► @df_fn_0$gen1   (geração 1, retida)
//!                         └─► @df_fn_0$gen2   (geração 2, published)
//! ```
//!
//! # Por que não `LLVMOrcCreateLocalIndirectStubsManager`
//!
//! `PLANO.md` aponta `LLVMOrcCreateLocalIndirectStubsManager` e
//! `LLVMOrcCreateLocalLazyCallThroughManager` como as primitivas prontas para
//! isso, e `llvm-sys 221.1.0` **expõe as duas**. Elas não servem, e a razão é
//! verificável na própria lista de símbolos que o `llvm-sys` publica: da classe
//! `IndirectStubsManager` a API C exporta apenas
//! `LLVMOrcCreateLocalIndirectStubsManager` e
//! `LLVMOrcDisposeIndirectStubsManager`. Não há entrada C para `createStub` nem
//! para **`updatePointer`**, que é exatamente a operação de uma recarga: apontar
//! um stub existente para outra implementação. O único consumidor exposto é
//! `LLVMOrcLazyReexports`, que devolve uma unidade de materialização de *aliases
//! preguiçosos*: o stub é criado na primeira chamada, resolvido uma vez e o
//! ponteiro passa a valer para sempre. Redefinir o mesmo alias numa segunda
//! geração não é possível — o nome já está definido na `JITDylib`, e definição
//! duplicada é erro.
//!
//! Sobrou o caminho que o próprio plano prevê como alternativa: **tabela de
//! ponteiros mutáveis com salto indireto**. Aqui ela não é assembly escrito à
//! mão, e sim IR gerado pela própria sessão — o trampolim é uma função LLVM de
//! três instruções (`load`, `call`, `ret`), e portanto vale em qualquer alvo que
//! o LLVM suporte, sem um emissor por arquitetura.
//!
//! O custo é um `load` e um quadro de pilha por chamada de função recarregável.
//! Ele não existe no caminho não recarregável ([`JitSession::add_ir_module`]),
//! nem no AOT.
//!
//! # Gerações e `JITDylib`
//!
//! `PLANO.md` pede «um `JITDylib`/tracker próprio» por recarga. O tracker é
//! próprio de fato. O `JITDylib` **não**: a API C cria dylibs
//! (`LLVMOrcExecutionSessionCreateJITDylib`) mas não expõe nenhum
//! `SetLinkOrder`, então uma dylib nova nasce sem ordem de ligação e não
//! alcançaria os símbolos de runtime nem os trampolins — o código da geração
//! não ligaria. A separação de nomes que a dylib daria é obtida pelo sufixo de
//! geração, e o descarregamento por geração é do `ResourceTracker`, que é quem o
//! LLVM oferece para isso.
//!
//! # Retenção de memória
//!
//! Uma geração é descarregada na publicação seguinte, quando nenhuma entrada
//! estável a alcança mais (J02). Ver [`JitSession::hot_reload`].

use std::collections::{BTreeMap, HashMap, HashSet};
use std::ffi::CString;
use std::sync::atomic::AtomicUsize;
use std::time::{Duration, Instant};

use crate::ffi::{self, FunctionSignature};
use crate::{ENTRY_SYMBOL, JitError, JitSession};

/// Prefixo das células de ponteiro publicadas como símbolos absolutos de dado.
///
/// `$` não aparece em nenhum nome do emissor (`crates/emit_native` usa `df_*` e
/// `dartforge_*`), então não há como uma célula colidir com uma função do
/// programa. Também é aceito em identificadores de IR sem citação.
const SLOT_PREFIX: &str = "__dfslot$";

/// Custo de um ciclo de recarga, separado por etapa.
///
/// As duas primeiras etapas do ciclo que `PLANO.md` exige — análise do Dart novo
/// e geração do IR — **não** aparecem aqui, e a omissão é deliberada: este crate
/// não tem front-end. Ele recebe IR textual, exatamente como o driver AOT, e
/// quem compila o Dart é o chamador (`dartforge reload`, ou o teste de medição),
/// que cronometra as suas próprias fases e as soma a estas. A tabela completa do
/// ciclo está em `docs/JIT.md`.
///
/// Cada intervalo é cronometrado no próprio trecho, nunca por subtração, como
/// manda `docs/DESEMPENHO.md`. `total` cobre a chamada inteira e é pelo menos a
/// soma das etapas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HotReloadReport {
    /// Número da geração publicada por esta recarga; a primeira é 1.
    pub generation: u32,
    /// Análise do IR textual novo (`LLVMParseIRInContext2`).
    pub parse_ir: Duration,
    /// Leitura da impressão digital do contrato e comparação com a versão viva.
    pub contract: Duration,
    /// Versionamento das implementações e entrega do módulo à `LLJIT`.
    pub add_module: Duration,
    /// Criação das células e do módulo de trampolins das entradas novas.
    pub stubs: Duration,
    /// Ligação em memória: materialização das implementações desta geração.
    pub link: Duration,
    /// Publicação: troca dos ponteiros das entradas estáveis.
    pub publish: Duration,
    /// Aposentadoria do código antigo: o descarregamento das gerações que
    /// nenhuma entrada estável alcança mais.
    pub retire: Duration,
    /// Espera pelo ponto seguro do programa em execução (recarga ao vivo):
    /// do pedido até a publicação rodar na thread do programa. Zero quando
    /// a publicação roda direto (nenhum programa em execução).
    pub safepoint_wait: Duration,
    /// Chamada completa de [`JitSession::hot_reload`].
    pub total: Duration,
    /// Bytes de IR textual analisados, para separar tempo de tamanho.
    pub ir_bytes: usize,
    /// Entradas estáveis que esta geração passou a implementar.
    pub entries: usize,
    /// Entradas estáveis criadas agora, que não existiam antes.
    pub new_entries: usize,
    /// J04: funções do programa cujo código é o da implementação publicada e
    /// que por isso esta geração não compilou (continuam nela).
    pub kept: usize,
    /// Gerações retidas em memória depois desta recarga, incluindo ela: a
    /// publicada e as de funções que sumiram do código novo.
    pub retained_generations: usize,
    /// A recarga promoveu um módulo simples a recarregável.
    ///
    /// Acontece na primeira recarga de um módulo incorporado por
    /// [`JitSession::add_ir_module`], e é o único caso com janela não
    /// transacional. Ver [`JitSession::hot_reload`].
    pub promoted: bool,
}

/// Uma entrada estável: nome permanente, assinatura fixa, ponteiro mutável.
pub(crate) struct Entry {
    /// Contrato de chamada, lido do IR da geração que criou a entrada.
    signature: FunctionSignature,
    /// Célula lida pelo trampolim. `Box` porque o endereço tem de ser estável.
    ///
    /// `AtomicUsize` não está aqui por concorrência — a sessão é de uma thread
    /// só —, e sim para que a troca do ponteiro seja Rust seguro, sem `unsafe`
    /// nem `UnsafeCell` fora de `ffi.rs`. O código gerado lê a célula com um
    /// `load` comum do mesmo tamanho de palavra.
    slot: Box<AtomicUsize>,
    /// Geração cuja implementação está publicada nesta entrada.
    generation: u32,
}

/// Um módulo recarregável: uma identidade, várias gerações.
pub(crate) struct Reloadable {
    /// Nome do módulo, como o chamador o informou.
    pub(crate) name: String,
    /// Número da última geração publicada.
    generation: u32,
    /// Entradas estáveis, indexadas pelo nome do símbolo do emissor.
    pub(crate) entries: BTreeMap<String, Entry>,
    /// Layout nominal das classes da versão viva: `(class_id, fields)`.
    layouts: Vec<(i64, i64)>,
    /// Nomes das classes registradas pela versão viva: `(class_id, name_str)`.
    class_names: Vec<(i64, String)>,
    /// Os campos de cada classe da versão viva, pelo nome (J03).
    fields: Vec<crate::migration::Layout>,
    /// J03: a entrada estável de uma função cuja assinatura mudou —
    /// `(name_str do emitter, fn_signature)` → `name_str$sigK`. A entrada com o nome
    /// original fica com o corpo antigo (os tear-offs antigos a chamam).
    aliases: BTreeMap<(String, String), String>,
    /// Globais mutáveis da geração ativa, reiniciadas em `run_entry`/`run_main`.
    pub(crate) globals: Vec<ffi::MutableGlobal>,
    /// Rastreadores das gerações ainda alcançáveis, com o número de cada:
    /// a publicada e as que alguma entrada estável ainda implementa (uma
    /// função que sumiu do código novo). As outras são descarregadas na
    /// publicação (J02; ver [`JitSession::hot_reload`]).
    generations: Vec<(u32, ffi::ResourceTracker)>,
    /// Rastreadores dos módulos de trampolim; nunca descarregados.
    stubs: Vec<ffi::ResourceTracker>,
    /// J04: a impressão digital e a assinatura da implementação publicada de
    /// cada entrada estável; a geração seguinte declara, em vez de compilar,
    /// a função cujo código é o mesmo (`crate::delta`).
    published_list: HashMap<String, crate::delta::Published>,
}

/// Endereço de uma entrada estável, com a assinatura que a sessão registrou.
///
/// É o que permite provar a propriedade central do hot reload sem `unsafe` no
/// chamador: o endereço é obtido **antes** da recarga e continua correto depois
/// dela, porque é o endereço do trampolim, não o do corpo.
///
/// ```no_run
/// let mut session = dartforge_jit::JitSession::new()?;
/// session.add_reloadable_module("app", "define i64 @df_fn_0() { ret i64 1 }\n")?;
/// let input = session.stable_entry("df_fn_0")?;
/// assert_eq!(input.call(&session)?, 1);
/// session.hot_reload("app", "define i64 @df_fn_0() { ret i64 2 }\n")?;
/// // O same endereço, captured_value before da edição, já executa o body new.
/// assert_eq!(input.call(&session)?, 2);
/// # Ok::<(), dartforge_jit::JitError>(())
/// ```
#[derive(Debug, Clone)]
pub struct StableEntry {
    name: String,
    pub(crate) address: u64,
    signature: FunctionSignature,
    session: u64,
}

impl StableEntry {
    /// Nome do símbolo, tal como o emissor o produziu.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Assinatura registrada, em texto de LLVM IR (`i64 (i64)`).
    pub fn signature(&self) -> String {
        self.signature.text()
    }

    /// Chama a entrada sem argumentos, exigindo assinatura `i64 ()`.
    ///
    /// # Erros
    /// Assinatura diferente de `i64 ()`, ou entrada de outra sessão.
    pub fn call(&self, session: &JitSession) -> Result<i64, JitError> {
        self.invoke(session, None)
    }

    /// Chama a entrada com um argumento, exigindo assinatura `i64 (i64)`.
    ///
    /// # Erros
    /// Assinatura diferente de `i64 (i64)`, ou entrada de outra sessão.
    pub fn call_with(&self, session: &JitSession, argument: i64) -> Result<i64, JitError> {
        self.invoke(session, Some(argument))
    }

    /// Chama uma entrada sem argumentos e sem retorno, como `dartforge_entry`.
    pub fn call_void(&self, session: &JitSession) -> Result<(), JitError> {
        if session.id != self.session {
            return Err(JitError {
                stage: "lookup",
                message: format!("a entrada estável {} pertence a outra sessão JIT", self.name),
            });
        }
        session.lljit.call_stable_void(self.address, &self.signature).map_err(|detail| {
            JitError::new("execute", &format!("não foi possível chamar a entrada estável {}", self.name), detail)
        })
    }

    /// Chama a entrada `i32 ()` do programa com SDK da fonte.
    pub fn call_i32(&self, session: &JitSession) -> Result<i32, JitError> {
        if session.id != self.session {
            return Err(JitError {
                stage: "lookup",
                message: format!("a entrada estável {} pertence a outra sessão JIT", self.name),
            });
        }
        session.lljit.call_stable_i32(self.address, &self.signature).map_err(|detail| {
            JitError::new("execute", &format!("não foi possível chamar a entrada estável {}", self.name), detail)
        })
    }

    /// Confere a sessão de origem e delega a chamada conferida à fronteira FFI.
    fn invoke(&self, session: &JitSession, argument: Option<i64>) -> Result<i64, JitError> {
        if session.id != self.session {
            return Err(JitError {
                stage: "lookup",
                message: format!(
                    "a entrada estável {} pertence a outra sessão JIT",
                    self.name
                ),
            });
        }
        session
            .lljit
            .call_stable(self.address, &self.signature, argument)
            .map_err(|detail| {
                JitError::new(
                    "lookup",
                    &format!("não foi possível chamar a entrada estável {}", self.name),
                    detail,
                )
            })
    }
}

impl JitSession {
    /// Incorpora um módulo **recarregável**, criando as entradas estáveis.
    ///
    /// É a forma recomendada de abrir um laço de desenvolvimento: a geração 1 já
    /// nasce atrás dos trampolins, e a partir daí toda recarga é transacional do
    /// começo ao fim — nada é descarregado nem redefinido antes de a nova versão
    /// estar materializada e pronta.
    ///
    /// Comparado a [`JitSession::add_ir_module`], este caminho cobra um `load` e
    /// um quadro de pilha por chamada entre funções do programa, que é o preço da
    /// identidade estável. Quem só quer executar uma vez deve usar o outro.
    ///
    /// # Erros
    /// Os mesmos de [`JitSession::hot_reload`]. Falha também se já existir um
    /// módulo recarregável com este nome.
    ///
    /// ```no_run
    /// let mut session = dartforge_jit::JitSession::new()?;
    /// let report_data =
    ///     session.add_reloadable_module("app", "define void @dartforge_entry() { ret void }\n")?;
    /// assert_eq!(report_data.generation, 1);
    /// assert!(!report_data.promoted);
    /// # Ok::<(), dartforge_jit::JitError>(())
    /// ```
    pub fn add_reloadable_module(
        &mut self,
        name: &str,
        ir: &str,
    ) -> Result<HotReloadReport, JitError> {
        if self.reloadables.iter().any(|module| module.name == name) {
            return Err(JitError {
                stage: "add-module",
                message: format!(
                    "já existe um módulo recarregável chamado '{name}'; use hot_reload"
                ),
            });
        }
        self.install_generation(name, ir, None)
    }

    /// Publica uma versão nova do módulo `name`, preservando o estado da sessão.
    ///
    /// # As três fases
    ///
    /// 1. **Preparar**, sem tocar na aplicação em execução: analisar o IR novo,
    ///    ler a impressão digital do contrato e compará-la com a da versão viva,
    ///    versionar as implementações (`f` → `f$genN`) e entregar o módulo à
    ///    `LLJIT` sob um `ResourceTracker` novo. Nada aqui altera o que está
    ///    executando; qualquer falha devolve `Err` e a versão anterior continua
    ///    valendo, bit a bit.
    /// 2. **Publicar**: materializar as implementações novas (é aqui que o código
    ///    nativo é gerado e ligado em memória) e só então trocar os ponteiros das
    ///    entradas estáveis. A troca é a única operação visível ao código que
    ///    executa, e é uma escrita de palavra por entrada.
    /// 3. **Aposentar** o código antigo: descarregar as gerações que nenhuma
    ///    entrada estável alcança mais. Ver a política de retenção abaixo.
    ///
    /// # Regra de visibilidade
    ///
    /// Chamadas já iniciadas terminam no corpo antigo; chamadas novas usam a
    /// implementação nova, inclusive quando partem de um quadro de pilha de uma
    /// geração antiga, porque as chamadas entre funções do programa também passam
    /// pelos trampolins. Substituir quadros ativos está fora de escopo: uma
    /// função que esteja no meio de um laço termina o laço no corpo em que
    /// entrou.
    ///
    /// # Retenção de memória (J02)
    ///
    /// Uma geração é descarregada assim que é seguro:
    ///
    /// * **nenhum quadro** dela existe: a publicação roda sem programa em
    ///   execução, ou no ponto seguro de evento do isolado principal com os
    ///   demais isolados parados no deles (`dartforge_parar_isolados`) — em
    ///   nenhum deles há quadro Dart na pilha; `hot_reload` toma `&mut self`,
    ///   então nada da sessão executa em paralelo;
    /// * **nenhum endereço** dela está guardado: toda referência a função do
    ///   programa, também a de uma closure ou de um tear-off, é a do
    ///   trampolim estável; o runtime copia as tabelas de métodos e esquece os
    ///   descritores de área da geração anterior na publicação; as globais
    ///   mutáveis foram copiadas para as da geração nova;
    /// * **nenhuma entrada estável** a implementa: uma função que sumiu do
    ///   código novo continua chamando o corpo antigo (um tear-off antigo a
    ///   alcança), e a geração dela fica retida até outra voltar a
    ///   implementá-la.
    ///
    /// A memória de código fica, portanto, na geração publicada mais as das
    /// funções sumidas, e não cresce com o número de recargas.
    /// `HotReloadReport::retained_generations` relata quantas ficaram. Uma
    /// geração que falhou antes de ser publicada também é removida: nenhum
    /// endereço dela chegou a nenhuma célula.
    ///
    /// # Escopo da versão 1
    ///
    /// Só mudanças de corpo com contrato compatível. Assinatura diferente,
    /// número de campos diferente numa classe já construída, função que
    /// desaparece e função variádica são recusadas na etapa `contract`, com o
    /// nome do símbolo na mensagem. A sessão não é alterada nesses casos.
    ///
    /// A identidade vem do **nome do símbolo emitido**, e os nomes de
    /// `crates/emit_native` são posicionais (`df_fn_0`, ...). Inserir ou
    /// reordenar declarações no Dart renumera os símbolos, e a recarga passa a
    /// comparar contratos de funções diferentes. O limite está em `docs/JIT.md`.
    ///
    /// # Erros
    /// `parse-ir` para IR inválido; `contract` para mudança incompatível;
    /// `add-module`, `link` ou `publish` para falhas da `LLJIT`; `poisoned`
    /// quando uma recarga anterior falhou dentro da janela de promoção e a
    /// sessão precisa ser reiniciada.
    ///
    /// ```no_run
    /// let mut session = dartforge_jit::JitSession::new()?;
    /// session.add_reloadable_module("app", "define void @dartforge_entry() { ret void }\n")?;
    /// let report_data = session.hot_reload("app", "define void @dartforge_entry() { ret void }\n")?;
    /// assert_eq!(report_data.generation, 2);
    /// # Ok::<(), dartforge_jit::JitError>(())
    /// ```
    pub fn hot_reload(&mut self, name: &str, fresh_ir: &str) -> Result<HotReloadReport, JitError> {
        if self.poisoned.is_none()
            && !self.reloadables.iter().any(|module| module.name == name)
            && !self.modules.iter().any(|module| !module.removed && module.name == name)
        {
            return Err(JitError {
                stage: "contract",
                message: format!(
                    "nenhum módulo chamado '{name}' está carregado nesta sessão; use add_reloadable_module para a primeira geração"
                ),
            });
        }
        self.install_generation(name, fresh_ir, None)
    }

    /// [`JitSession::hot_reload`] com a publicação entregue a `safepoint`,
    /// que a roda na thread do programa em execução, entre dois eventos do
    /// laço do isolado principal (`crate::alive`), e só retorna depois.
    pub(crate) fn hot_reload_at_safepoint(
        &mut self,
        name: &str,
        fresh_ir: &str,
        safepoint: &mut dyn FnMut(ffi::Task),
    ) -> Result<HotReloadReport, JitError> {
        if !self.reloadables.iter().any(|module| module.name == name) {
            return Err(JitError {
                stage: "contract",
                message: format!("nenhum módulo recarregável chamado '{name}' está carregado nesta sessão"),
            });
        }
        self.install_generation(name, fresh_ir, Some(safepoint))
    }

    /// Resolve uma entrada estável e devolve seu endereço com a assinatura.
    ///
    /// O endereço é do trampolim, que nunca é descarregado, então o valor
    /// devolvido continua válido depois de qualquer número de recargas — é isso
    /// que um callback registrado pela aplicação guarda.
    ///
    /// # Erros
    /// `lookup` quando nenhum módulo recarregável da sessão define esse nome, ou
    /// quando a `LLJIT` não consegue resolvê-lo.
    pub fn stable_entry(&self, name: &str) -> Result<StableEntry, JitError> {
        let entry = self
            .reloadables
            .iter()
            .find_map(|module| module.entries.get(name))
            .ok_or_else(|| JitError {
                stage: "lookup",
                message: format!("nenhum módulo recarregável desta sessão define {name}"),
            })?;
        if entry.generation == 0 {
            return Err(JitError {
                stage: "lookup",
                message: format!(
                    "a entrada estável {name} existe, mas nenhuma geração a implementou:                      a recarga que a criou falhou antes de publicar o ponteiro"
                ),
            });
        }
        let signature = entry.signature.clone();
        Ok(StableEntry {
            name: name.to_owned(),
            address: self.lookup(name)?,
            signature,
            session: self.id,
        })
    }

    /// Nomes das entradas estáveis de um módulo recarregável, em ordem.
    ///
    /// Vazio quando o módulo não existe ou não é recarregável.
    pub fn stable_entries(&self, name: &str) -> Vec<&str> {
        self.reloadables
            .iter()
            .find(|module| module.name == name)
            .map(|module| module.entries.keys().map(String::as_str).collect())
            .unwrap_or_default()
    }

    /// Geração publicada de um módulo recarregável, se houver alguma.
    ///
    /// `None` quando o módulo não existe e também quando ele existe apenas com
    /// entradas órfãs, isto é, quando nenhuma geração dele chegou a ser
    /// publicada — ver [`JitSession::stable_entry`].
    pub fn generation(&self, name: &str) -> Option<u32> {
        self.reloadables
            .iter()
            .find(|module| module.name == name)
            .map(|module| module.generation)
            .filter(|generation| *generation > 0)
    }

    /// Gerações retidas em memória por todos os módulos recarregáveis.
    ///
    /// Cresce em um por recarga bem-sucedida e nunca diminui, pela política de
    /// retenção documentada em [`JitSession::hot_reload`].
    pub fn retained_generations(&self) -> usize {
        self.reloadables
            .iter()
            .map(|module| module.generations.len())
            .sum()
    }

    /// Implementa as três fases para uma geração nova de `name`.
    ///
    /// Serve tanto à primeira geração ([`JitSession::add_reloadable_module`])
    /// quanto às recargas ([`JitSession::hot_reload`]); a diferença está apenas
    /// em o que existe antes.
    fn install_generation(
        &mut self,
        name: &str,
        ir: &str,
        safepoint: Option<&mut dyn FnMut(ffi::Task)>,
    ) -> Result<HotReloadReport, JitError> {
        let started = Instant::now();
        if let Some(reason) = self.poisoned.clone() {
            return Err(JitError {
                stage: "poisoned",
                message: reason,
            });
        }

        // ── Fase 1: preparar, sem tocar no que está executando ──────────────
        // J04: só o que mudou é compilado. A função cujo código é o da
        // implementação publicada vira declaração, e as chamadas a ela caem no
        // trampolim, que continua apontando para a implementação viva; os
        // registros da geração são sempre dela.
        let empties_list = HashMap::new();
        let live_list = self.reloadables.iter().find(|m| m.name == name).map_or(&empties_list, |m| &m.published_list);
        let (generation_ir, kept_fns, mut new_items) =
            match crate::delta::delta(ir, live_list, &[AREA_PREPARATION, PROGRAM_REGISTRY, RTI_START]) {
                Some(d) => (std::borrow::Cow::Owned(d.ir), d.kept_fns, d.new_items),
                None => (std::borrow::Cow::Borrowed(ir), Vec::new(), HashMap::new()),
            };
        let phase = Instant::now();
        let parsed = ffi::parse_module(name, &generation_ir)
            .map_err(|detail| JitError::new("parse-ir", "IR inválido", detail))?;
        self.check_target(&parsed)?;
        let parse_ir = phase.elapsed();

        let phase = Instant::now();
        let mut signatures = parsed.signatures();
        // J03: uma função cuja assinatura mudou ganha outra entrada estável
        // (`name_str$sigK`, a mesma para a mesma assinatura nas gerações
        // seguintes); a entrada original continua com o corpo antigo, como
        // uma função que sumiu. Renomear a definição leva junto todos os usos
        // dela no módulo novo (tabelas de métodos, closures, chamadas).
        let mut new_aliases: Vec<((String, String), String)> = Vec::new();
        let mut renamed: Vec<(String, String)> = Vec::new();
        if let Some(index) = self.reloadables.iter().position(|m| m.name == name) {
            let modulo = &self.reloadables[index];
            let mut renames = Vec::new();
            for s in &signatures {
                let text = s.text();
                let Some(current) = modulo.entries.get(&s.name).filter(|e| e.generation > 0) else { continue };
                if current.signature.text() == text {
                    continue;
                }
                let key = (s.name.clone(), text);
                let target = match modulo.aliases.get(&key) {
                    Some(a) => a.clone(),
                    None => {
                        let k = modulo.aliases.keys().filter(|(n, _)| *n == s.name).count() + 1;
                        let a = format!("{}$sig{k}", s.name);
                        new_aliases.push((key, a.clone()));
                        a
                    }
                };
                renames.push((s.name.clone(), target));
            }
            for (de, to) in &renames {
                if !parsed.rename_function(de, to) {
                    return Err(JitError {
                        stage: "contract",
                        message: format!(
                            "a assinatura de {de} mudou e a entrada nova {to} não pôde ser criada; \
                             a recarga é recusada — reinicie a sessão"
                        ),
                    });
                }
            }
            if !renames.is_empty() {
                signatures = parsed.signatures();
            }
            renamed = renames;
        }
        let mut layouts = parsed.class_layouts();
        // As alocações das funções mantidas não estão no módulo; o texto delas
        // é o da geração viva, então o layout que alocam é o registrado.
        if !kept_fns.is_empty()
            && let Some(live_one) = self.reloadables.iter().find(|m| m.name == name)
        {
            for &(class, n) in &live_one.layouts {
                if !layouts.iter().any(|&(c, _)| c == class) {
                    layouts.push((class, n));
                }
            }
        }
        for (de, to) in &renamed {
            if let Some(p) = new_items.remove(de) {
                new_items.insert(to.clone(), p);
            }
        }
        let class_names = parsed.class_names();
        let fields = crate::migration::ir_layouts(ir);
        let references = parsed.declarations();
        let existing = self.reloadables.iter().position(|m| m.name == name);
        let plain = self.plain_module_index(name, existing.is_some());
        // Um símbolo de outro módulo já ocupa o nome que viraria trampolim.
        // Detectar a colisão aqui é obrigatório na promoção: depois que o
        // módulo simples é removido, uma falha ao publicar o trampolim deixa
        // a sessão envenenada e destrói a versão que ainda funcionava.
        let occupied: HashSet<&str> = self.modules.iter().enumerate()
            .filter(|(index, module)| !module.removed && Some(*index) != plain)
            .flat_map(|(_, module)| module.signatures.iter().map(|s| s.name.as_str()))
            .chain(self.reloadables.iter().enumerate()
                .filter(|(index, _)| Some(*index) != existing)
                .flat_map(|(_, module)| module.entries.keys().map(String::as_str)))
            .collect();
        if let Some(collision) = signatures.iter().find(|s| occupied.contains(s.name.as_str())) {
            return Err(JitError {
                stage: "contract",
                message: format!(
                    "a função {} já pertence a outro módulo da sessão; a recarga não pode publicar uma segunda entrada com esse nome",
                    collision.name
                ),
            });
        }
        let previous: Vec<FunctionSignature> = match (existing, plain) {
            // Só as entradas que alguma geração de fato implementou entram na
            // comparação; uma entrada órfã (célula publicada, implementação
            // nenhuma) não é uma versão em execução.
            (Some(index), _) => self.reloadables[index]
                .entries
                .values()
                .filter(|entry| entry.generation > 0)
                .map(|entry| entry.signature.clone())
                .collect(),
            (None, Some(index)) => self.modules[index].signatures.clone(),
            (None, None) => Vec::new(),
        };
        let previous_layouts: &[(i64, i64)] = match (existing, plain) {
            (Some(index), _) => &self.reloadables[index].layouts,
            (None, Some(index)) => &self.modules[index].layouts,
            (None, None) => &[],
        };
        let previous_names: &[(i64, String)] = match (existing, plain) {
            (Some(index), _) => &self.reloadables[index].class_names,
            (None, Some(index)) => &self.modules[index].class_names,
            (None, None) => &[],
        };
        let previous_fields: &[crate::migration::Layout] = match (existing, plain) {
            (Some(index), _) => &self.reloadables[index].fields,
            (None, Some(index)) => &self.modules[index].fields,
            (None, None) => &[],
        };
        // J03: a classe cujos campos as duas versões descrevem pelo nome migra
        // (ou é recusada com o motivo); as outras ficam com a contagem.
        let migrated: HashSet<i64> = previous_fields
            .iter()
            .filter(|l| fields.iter().any(|n| n.class == l.class))
            .map(|l| l.class)
            .collect();
        check_contract(&previous, previous_layouts, &signatures, &layouts, &migrated).map_err(|message| {
            JitError {
                stage: "contract",
                message,
            }
        })?;
        check_class_names(previous_names, &class_names).map_err(|message| JitError { stage: "contract", message })?;
        let class_name = |id: i64| {
            class_names
                .iter()
                .find(|(c, _)| *c == id)
                .map_or_else(|| format!("a classe de id {id}"), |(_, n)| format!("a classe {n}"))
        };
        let plan = crate::migration::plan_fn(previous_fields, &fields, &class_name)
            .map_err(|message| JitError { stage: "contract", message })?;
        let encoded_plan = (!plan.is_empty()).then(|| crate::migration::encode(&plan));
        // A publicação refaz os registros do programa (tabelas de métodos,
        // regras da RTI) quando já houve uma geração: o programa em execução
        // tem os da anterior. As funções de registro são entradas da própria
        // geração; o runtime que as recebe é o da sessão.
        let records = if existing.is_some() {
            let has = |name_str: &str| signatures.iter().any(|s| s.name == name_str && s.ret == "void" && s.params.is_empty());
            let area = has(AREA_PREPARATION);
            let register = has(PROGRAM_REGISTRY);
            let rti = has(RTI_START);
            if area || register || rti || encoded_plan.is_some() {
                let runtime = match &self.sdk_dll {
                    Some(dll) => ffi::ReloadRuntime::from_library(dll)
                        .map_err(|detail| JitError::new("contract", "o runtime da sessão não publica gerações", detail))?,
                    None => ffi::ReloadRuntime::embedded(),
                };
                if encoded_plan.is_some() && !runtime.migrates() {
                    return Err(JitError {
                        stage: "contract",
                        message: "os campos de uma classe mudaram e o runtime da sessão não migra instâncias \
                                  (biblioteca do SDK anterior a J03); a recarga é recusada — reinicie a sessão"
                            .to_owned(),
                    });
                }
                Some((runtime, area, register, rti))
            } else {
                None
            }
        } else {
            None
        };
        // Dados externos não aparecem em `declarations()`, que percorre só
        // funções. Resolver aqui também detecta dados ausentes antes da janela
        // destrutiva da promoção. Um símbolo já publicado pela LLJIT, inclusive
        // por outro módulo ou pela DLL do SDK, pode ser usado normalmente.
        for global in parsed.external_globals() {
            self.lljit.lookup(&global).map_err(|detail| JitError::new(
                "contract",
                &format!("a global externa {global} não está disponível nesta sessão"),
                detail,
            ))?;
        }
        let mut known: Vec<&str> = signatures.iter().map(|s| s.name.as_str()).collect();
        // Outros módulos ainda residentes também são fonte válida de símbolos: uma
        // sessão pode ter o programa e uma biblioteca em módulos separados, como
        // no teste `two_modules_share_one_session`.
        // O módulo que a promoção vai descarregar fica de fora: os nomes dele
        // deixam de existir justamente para dar lugar aos trampolins.
        known.extend(
            self.modules
                .iter()
                .enumerate()
                .filter(|(index, module)| !module.removed && Some(*index) != plain)
                .flat_map(|(_, module)| module.signatures.iter().map(|s| s.name.as_str())),
        );
        // A geração inicial só publica os exports que usava. Uma edição pode
        // chamar outro membro do SDK: conferir a lista da DLL antes de mudar
        // qualquer célula, e publicar somente os novos nomes.
        let new_sdk = if let Some(dll) = &self.sdk_dll {
            let mut requests: Vec<String> = references
                .iter()
                .filter(|reference| {
                    !self.is_known_external(reference)
                        && !self.sdk_externals.contains(*reference)
                        && !known.contains(&reference.as_str())
                        && !self.reloadables.iter().any(|m| m.entries.contains_key(*reference))
                })
                .cloned()
                .collect();
            requests.sort();
            requests.dedup();
            ffi::Lljit::exported_symbols_in_dll(dll, &requests)
                .map_err(|detail| JitError::new("sdk", "não foi possível ler as exportações do SDK", detail))?
        } else {
            Vec::new()
        };
        let mut external = self.sdk_externals.clone();
        external.extend(new_sdk.iter().cloned());
        check_references(&references, &known, &self.reloadables, &external, self.sdk_dll.is_none()).map_err(|message| JitError {
            stage: "contract",
            message,
        })?;
        if !new_sdk.is_empty() {
            let dll = self.sdk_dll.as_ref().expect("novos exports exigem DLL do SDK");
            let published_set = self.lljit.define_symbols_from_dll(dll, &new_sdk, false)
                .map_err(|detail| JitError::new("sdk", "não foi possível publicar novos exports do SDK", detail))?;
            self.sdk_externals.extend(published_set);
        }
        let contract = phase.elapsed();

        let generation = existing.map_or(1, |index| self.reloadables[index].generation + 1);
        let previous_globals = existing.map(|index| self.reloadables[index].globals.clone()).unwrap_or_default();
        let suffix = format!("$gen{generation}");
        // As implementações têm nomes estáveis únicos entre módulos; as
        // globais `@dfg_*` do emissor não. A LLJIT usa uma JITDylib única,
        // então o dado precisa carregar também a identidade do módulo.
        let module_index = existing.unwrap_or(self.reloadables.len());
        let global_suffix = format!("$module{module_index}{suffix}");
        let phase = Instant::now();
        let globals = parsed.version_mutable_globals(&global_suffix)
            .map_err(|detail| JitError::new("globais", "a geração tem estado que a sessão não sabe reiniciar", detail))?;
        for next in &globals {
            if !program_static(&next.logical_name) {
                continue;
            }
            if let Some(previous) = previous_globals.iter().find(|g| g.logical_name == next.logical_name)
                && previous.size != next.size
            {
                return Err(JitError::new("contract", "o layout de um estático mudou", format!(
                    "{}: {} para {} bytes", next.logical_name, previous.size, next.size
                )));
            }
        }
        let published = parsed.version_definitions(&suffix);
        let tracker = self.lljit.create_tracker();
        self.lljit
            .add_module(&tracker, parsed.into_thread_safe())
            .map_err(|detail| {
                JitError::new("add-module", "a LLJIT recusou o módulo da geração", detail)
            })?;
        let add_module = phase.elapsed();

        // ── Fase 2: publicar ────────────────────────────────────────────────
        // A ordem é o que dá a garantia transacional. A promoção de um módulo
        // simples é a única operação destrutiva, e vem depois de todo o trabalho
        // que pode falhar por causa do código novo.
        let phase = Instant::now();
        let mut promoted = false;
        let fresh: Vec<FunctionSignature> = match existing {
            // Uma entrada órfã conta como existente: a célula e o trampolim dela
            // já estão na `JITDylib`, e publicá-los de novo seria definição
            // duplicada. Ela só precisa receber o ponteiro.
            Some(index) => published
                .iter()
                .filter(|s| !self.reloadables[index].entries.contains_key(&s.name))
                .cloned()
                .collect(),
            None => published.clone(),
        };
        // `plain` só é `Some` no caminho de promoção: quando existe um módulo
        // recarregável com este nome, `plain_module_index` devolve `None`.
        if let Some(index) = plain {
            // Ponto destrutivo, e único: os nomes estáveis ainda pertencem ao
            // módulo simples, e o módulo de trampolins não pode redefini-los.
            // Descarregar aqui é seguro — `&mut self` garante que nada da sessão
            // está executando — mas abre a janela documentada em `hot_reload`:
            // uma falha entre este ponto e o fim da publicação deixa a sessão sem
            // os nomes estáveis, e ela é envenenada.
            self.modules[index].tracker.remove().map_err(|detail| {
                JitError::new(
                    "resource",
                    "não foi possível descarregar o módulo simples anterior",
                    detail,
                )
            })?;
            self.modules[index].removed = true;
            promoted = true;
        }
        let mut slots: BTreeMap<String, Box<AtomicUsize>> = BTreeMap::new();
        let mut stub_tracker: Option<ffi::ResourceTracker> = None;
        if !fresh.is_empty() {
            let mut pairs = Vec::with_capacity(fresh.len());
            for signature in &fresh {
                let cell = Box::new(AtomicUsize::new(0));
                let symbol = CString::new(format!("{SLOT_PREFIX}{}", signature.name))
                    .map_err(|_| JitError {
                        stage: "publish",
                        message: format!("o nome {} não sobrevive à fronteira C", signature.name),
                    })
                    .map_err(|error| self.poison_if(promoted, error))?;
                pairs.push((symbol, cell.as_ref() as *const AtomicUsize as u64));
                slots.insert(signature.name.clone(), cell);
            }
            self.lljit
                .define_data_symbols(&pairs)
                .map_err(|detail| {
                    JitError::new(
                        "publish",
                        "não foi possível publicar as células das entradas estáveis",
                        detail,
                    )
                })
                .map_err(|error| self.poison_if(promoted, error))?;
            let stub_ir = stub_module_ir(&fresh);
            let module = ffi::parse_ir(&format!("{name}$trampolins"), &stub_ir)
                .map_err(|detail| {
                    JitError::new("publish", "o módulo de trampolins não é IR válido", detail)
                })
                .map_err(|error| self.poison_if(promoted, error))?;
            let created = self.lljit.create_tracker();
            self.lljit
                .add_module(&created, module)
                .map_err(|detail| {
                    JitError::new("publish", "a LLJIT recusou o módulo de trampolins", detail)
                })
                .map_err(|error| self.poison_if(promoted, error))?;
            stub_tracker = Some(created);
        }
        let stubs = phase.elapsed();

        // Ligação em memória: resolver cada implementação materializa o código
        // nativo desta geração e liga todas as suas referências.
        let phase = Instant::now();
        let mut addresses = Vec::with_capacity(published.len());
        for signature in &published {
            let versioned = format!("{}{suffix}", signature.name);
            match self.lljit.lookup(&versioned) {
                Ok(address) => addresses.push(address),
                Err(detail) => {
                    let error = JitError::new(
                        "link",
                        &format!("não foi possível ligar a implementação {versioned}"),
                        detail,
                    );
                    // Rollback: a geração não foi publicada, nenhuma célula
                    // aponta para ela e nada pode tê-la chamado. Descarregar é
                    // demonstravelmente seguro, e é o que mantém a versão
                    // anterior intacta.
                    let _ = tracker.remove();
                    // As células e os trampolins das entradas novas, esses,
                    // ficam: um nome definido na `JITDylib` não pode ser
                    // redefinido, então esquecê-los impediria a próxima recarga
                    // de criar as mesmas entradas. Ficam registrados como
                    // órfãos — sem implementação — e a recarga seguinte
                    // reaproveita a célula em vez de publicar outra.
                    let orphans = std::mem::take(&mut slots);
                    self.register_unimplemented(name, orphans, &published, stub_tracker.take());
                    return Err(self.poison_if(promoted, error));
                }
            }
        }
        // O código da geração nova já foi ligado, mas as entradas estáveis
        // ainda chamam a antiga. Preserve apenas os estáticos do programa;
        // caches de seletor guardam endereços da geração anterior e devem
        // começar vazios. A cópia em si é da publicação: o programa pode
        // estar escrevendo neles até o ponto seguro.
        let mut copies = Vec::new();
        for next in &globals {
            if !program_static(&next.logical_name) {
                continue;
            }
            let Some(previous) = previous_globals.iter().find(|g| g.logical_name == next.logical_name) else {
                continue;
            };
            match self.lljit.global_copy(previous, next) {
                Ok(copy) => copies.push(copy),
                Err(detail) => {
                    let error = JitError::new("link", "não foi possível preservar um estático", detail);
                    let _ = tracker.remove();
                    let orphans = std::mem::take(&mut slots);
                    self.register_unimplemented(name, orphans, &published, stub_tracker.take());
                    return Err(self.poison_if(promoted, error));
                }
            }
        }
        // Os trampolins das funções de registro, resolvidos antes da
        // publicação (a tarefa só leva endereços).
        let records = match records {
            Some((runtime, area, register, rti)) => {
                let addr_value = |session: &Self, name_str: &str, request: bool| -> Result<Option<u64>, JitError> {
                    if request { session.lookup(name_str).map(Some) } else { Ok(None) }
                };
                Some((
                    runtime,
                    addr_value(self, AREA_PREPARATION, area)?,
                    addr_value(self, PROGRAM_REGISTRY, register)?,
                    addr_value(self, RTI_START, rti)?,
                ))
            }
            None => None,
        };
        let link = phase.elapsed();

        // Publicação: a troca dos ponteiros. Daqui em diante toda chamada nova
        // chega à implementação desta geração.
        let phase = Instant::now();
        let index = self.index_or_create(name);
        let module = &mut self.reloadables[index];
        // As entradas novas entram já registradas (a célula ainda nula:
        // nenhum código publicado as chama); a tarefa só leva endereços.
        let mut swaps = Vec::with_capacity(published.len());
        for (signature, address) in published.iter().zip(&addresses) {
            if let Some(slot) = slots.remove(&signature.name) {
                module.entries.insert(
                    signature.name.clone(),
                    Entry { signature: signature.clone(), slot, generation: 0 },
                );
            }
            let entry = module
                .entries
                .get(&signature.name)
                .expect("a verificação de contrato garante a entrada existente");
            swaps.push((ffi::Cell::de(&entry.slot), *address as usize));
        }
        // Com um programa em execução, a tarefa roda no ponto seguro do
        // isolado principal e para antes os demais isolados no ponto seguro
        // deles: nenhum quadro Dart de nenhum isolado vê a troca pela metade.
        let task: ffi::Task = Box::new(move || {
            let stopped_flag = records.map(|(runtime, ..)| (runtime, runtime.stop_isolates()));
            for copy in copies {
                copy.apply();
            }
            for (cell_ref, implementation) in swaps {
                cell_ref.publicar(implementation);
            }
            if let Some((runtime, area, register, rti)) = records {
                // J03: os objetos vivos de cada isolado passam ao layout novo
                // ao publicar (o deste agora; os parados, ao serem liberados).
                if let Some(plan) = &encoded_plan {
                    runtime.set_migration(plan);
                }
                runtime.publish_records(area, register, rti);
                if let Some((_, p)) = stopped_flag {
                    runtime.release_isolates(p, area, register, rti);
                }
            }
        });
        let wait_entry = Instant::now();
        let safepoint_wait = match safepoint {
            Some(execute_fn) => {
                execute_fn(task);
                wait_entry.elapsed()
            }
            None => {
                task();
                Duration::ZERO
            }
        };
        let module = &mut self.reloadables[index];
        for signature in &published {
            if let Some(entry) = module.entries.get_mut(&signature.name) {
                entry.generation = generation;
            }
        }
        module.generation = generation;
        module.layouts = layouts;
        module.class_names = class_names;
        module.fields = fields;
        module.aliases.extend(new_aliases);
        module.published_list.extend(new_items);
        module.globals = globals;
        module.generations.push((generation, tracker));
        if let Some(created) = stub_tracker {
            module.stubs.push(created);
        }
        let publish = phase.elapsed().saturating_sub(safepoint_wait);

        // Fase 3: aposentar. Depois da publicação nenhum quadro de uma geração
        // anterior existe (a tarefa rodou no ponto seguro de evento de todos
        // os isolados, ou sem programa em execução), toda chamada passa pelos
        // trampolins e o runtime não guarda endereço da memória de uma
        // geração (tabelas de métodos copiadas, descritores de área
        // esquecidos na publicação). Só uma entrada estável cuja função sumiu
        // do código novo ainda aponta para o corpo antigo: a geração dela fica.
        let phase = Instant::now();
        let module = &mut self.reloadables[index];
        let reached_fns: HashSet<u32> = module.entries.values().map(|e| e.generation).collect();
        let mut retained_list = Vec::with_capacity(module.generations.len());
        for (g, resource_tracker) in std::mem::take(&mut module.generations) {
            if g == generation || reached_fns.contains(&g) {
                retained_list.push((g, resource_tracker));
            } else if resource_tracker.remove().is_err() {
                // O LLVM não descarregou: a geração fica retida, como antes.
                retained_list.push((g, resource_tracker));
            }
        }
        module.generations = retained_list;
        let retire = phase.elapsed();

        // Materializa os trampolins agora, para que o custo apareça no relatório
        // desta recarga e não na primeira chamada da aplicação.
        if published.iter().any(|s| s.name == ENTRY_SYMBOL) {
            self.lookup(ENTRY_SYMBOL)?;
        }

        Ok(HotReloadReport {
            generation,
            parse_ir,
            contract,
            add_module,
            stubs,
            link,
            publish,
            retire,
            safepoint_wait,
            total: started.elapsed(),
            ir_bytes: generation_ir.len(),
            entries: published.len(),
            new_entries: fresh.len(),
            kept: kept_fns.len(),
            retained_generations: self.retained_generations(),
            promoted,
        })
    }

    /// Índice do módulo recarregável `name`, criando-o vazio se não existir.
    fn index_or_create(&mut self, name: &str) -> usize {
        if let Some(index) = self.reloadables.iter().position(|m| m.name == name) {
            return index;
        }
        self.reloadables.push(Reloadable {
            name: name.to_owned(),
            generation: 0,
            entries: BTreeMap::new(),
            layouts: Vec::new(),
            class_names: Vec::new(),
            fields: Vec::new(),
            aliases: BTreeMap::new(),
            globals: Vec::new(),
            generations: Vec::new(),
            stubs: Vec::new(),
            published_list: HashMap::new(),
        });
        self.reloadables.len() - 1
    }

    /// Registra entradas cuja célula e trampolim existem sem implementação.
    ///
    /// Acontece quando a ligação da geração que as criou falhou. O nome já está
    /// definido na `JITDylib` e lá vai ficar, porque redefinir não é permitido;
    /// guardá-lo aqui é o que permite à recarga seguinte reaproveitar a célula.
    /// A entrada nasce com `generation` zero, e [`JitSession::stable_entry`] a
    /// recusa enquanto estiver nesse estado — o trampolim dela salta por um
    /// ponteiro nulo.
    fn register_unimplemented(
        &mut self,
        name: &str,
        slots: BTreeMap<String, Box<AtomicUsize>>,
        published: &[FunctionSignature],
        stub: Option<ffi::ResourceTracker>,
    ) {
        if slots.is_empty() && stub.is_none() {
            return;
        }
        let index = self.index_or_create(name);
        let module = &mut self.reloadables[index];
        for (name_str, slot) in slots {
            if let Some(signature) = published.iter().find(|s| s.name == name_str) {
                module.entries.insert(
                    name_str,
                    Entry {
                        signature: signature.clone(),
                        slot,
                        generation: 0,
                    },
                );
            }
        }
        if let Some(stub) = stub {
            module.stubs.push(stub);
        }
    }

    /// Índice do módulo simples que a recarga substitui, se houver.
    ///
    /// A identidade é o nome informado na carga; um nome diferente nunca pode
    /// selecionar e descarregar o único módulo ativo por acidente.
    fn plain_module_index(&self, name: &str, reloadable_exists: bool) -> Option<usize> {
        if reloadable_exists {
            return None;
        }
        self.modules.iter().rposition(|module| !module.removed && module.name == name)
    }

    /// Envenena a sessão quando a falha aconteceu dentro da janela de promoção.
    ///
    /// Fora dela o erro é apenas um erro: a versão anterior continua publicada e
    /// a sessão segue utilizável. Dentro dela os nomes estáveis já foram
    /// descarregados e não há versão boa para voltar, então a sessão passa a
    /// recusar recargas com um diagnóstico que diz o que fazer — reiniciar —, em
    /// vez de deixar o chamador descobrir por corrupção.
    fn poison_if(&mut self, promoted: bool, error: JitError) -> JitError {
        if !promoted {
            return error;
        }
        let message = format!(
            "a promoção do módulo a recarregável falhou em '{}' ({}); \
             os nomes estáveis já haviam sido descarregados e esta sessão precisa ser reiniciada",
            error.stage, error.message
        );
        self.poisoned = Some(message.clone());
        JitError {
            stage: "poisoned",
            message,
        }
    }
}

/// Somente estado do programa atravessa gerações; caches do código não.
fn program_static(name: &str) -> bool {
    name.starts_with("dfg.") || name.starts_with("dfg_") || name == "df_statics"
}

/// Compara a impressão digital do contrato entre a versão viva e a nova.
///
/// # Erros
/// Devolve a explicação em português da primeira incompatibilidade encontrada,
/// sempre nomeando o símbolo ou a classe, para que o diagnóstico substitua a
/// corrupção silenciosa.
fn check_contract(
    previous: &[FunctionSignature],
    previous_layouts: &[(i64, i64)],
    new: &[FunctionSignature],
    layouts: &[(i64, i64)],
    migrated: &HashSet<i64>,
) -> Result<(), String> {
    if let Some(variadic) = new.iter().find(|signature| signature.var_arg) {
        return Err(format!(
            "a função {} é variádica, e o contrato do emissor nativo não prevê variádicas",
            variadic.name
        ));
    }
    for old in previous {
        // Uma função que sumiu do código novo continua com o corpo antigo: o
        // código novo não a chama, e uma closure ou um tear-off antigo que
        // ainda a alcance executa a versão em que foi criado — como na VM.
        match new.iter().find(|signature| signature.name == old.name) {
            None => {}
            Some(new) if new.text() != old.text() => {
                return Err(format!(
                    "a assinatura de {} mudou de {} para {}; \
                     mudança de contrato de chamada exige reiniciar a sessão",
                    old.name,
                    old.text(),
                    new.text()
                ));
            }
            Some(_) => {}
        }
    }
    // Uma classe descrita pelo nome dos campos nas duas versões migra
    // (`migracao.rs`); só a contagem não diz o que corresponde a quê.
    for (class, fields) in previous_layouts.iter().filter(|(c, _)| !migrated.contains(c)) {
        if let Some((_, new_ones)) = layouts
            .iter()
            .find(|(candidate, new_ones)| candidate == class && new_ones != fields)
        {
            return Err(format!(
                "a classe de id {class} tinha {fields} campos e passou a ter {new_ones}; \
                 os objetos já vivos no heap gerenciado mantêm o layout antigo, então a recarga \
                 é recusada — reinicie a sessão"
            ));
        }
    }
    Ok(())
}

/// As funções que a publicação de uma geração chama: a área de globais com
/// o layout dela e os registros que ela refaz.
const AREA_PREPARATION: &str = "df.preparar_area";
const PROGRAM_REGISTRY: &str = "df.registrar.programa";
const RTI_START: &str = "dartforge_rti_iniciar";

/// Os ids de classe do programa são posicionais: uma edição que insere ou
/// reordena classes troca a classe de um id que já tem objetos no heap.
///
/// # Erros
/// Nomeia o id, a classe da versão viva e a do código novo.
fn check_class_names(previous: &[(i64, String)], new: &[(i64, String)]) -> Result<(), String> {
    for (id, old) in previous {
        if let Some((_, new)) = new.iter().find(|(candidate, new)| candidate == id && new != old) {
            return Err(format!(
                "a classe de id {id} era {old} e passou a ser {new} (classes inseridas ou reordenadas); \
                 os objetos já vivos no heap têm o id antigo, então a recarga é recusada — reinicie a sessão"
            ));
        }
    }
    Ok(())
}

/// Confere que toda referência externa do código novo é resolvível.
///
/// Feito **antes** de qualquer efeito, e não deixado para a falha de ligação, por
/// dois motivos: a mensagem fica melhor (o nome que falta, em vez do diagnóstico
/// do ORC) e a promoção de um módulo simples deixa de arriscar a janela
/// destrutiva por causa de um símbolo ausente — o caso comum de um `@Native` sem
/// objeto ligado.
///
/// # Erros
/// Nomeia o primeiro símbolo que a sessão não sabe definir.
fn check_references(
    references: &[String],
    defined: &[&str],
    reloadables: &[Reloadable],
    sdk_externals: &HashSet<String>,
    embedded_runtime: bool,
) -> Result<(), String> {
    for reference in references {
        if (embedded_runtime && ffi::is_known_external(reference))
            || crate::CRT_SYMBOLS.contains(&reference.as_str())
            || reference.starts_with("llvm.")
            || sdk_externals.contains(reference)
            || defined.contains(&reference.as_str())
            || reloadables
                .iter()
                .any(|module| module.entries.get(reference).is_some_and(|entry| entry.generation > 0))
        {
            continue;
        }
        return Err(format!(
            "o código novo chama {reference}, que esta sessão não define; \
             o JIT publica a tabela do runtime, a CRT listada, as exportações carregadas do SDK \
             e as entradas estáveis já criadas"
        ));
    }
    Ok(())
}

/// Gera o IR do módulo de trampolins das entradas informadas.
///
/// Cada entrada vira uma função com o nome estável, que lê a célula e repassa a
/// chamada. O `call` é comum, e não `musttail`: um `musttail` imporia restrições
/// de ABI por alvo (e falha de compilação onde não for possível) para economizar
/// um quadro de pilha que a sessão pode pagar. O quadro extra é o custo
/// declarado da identidade estável.
fn stub_module_ir(entries: &[FunctionSignature]) -> String {
    let mut ir = String::from("; DartForge hot reload: trampolins das entradas estáveis\n");
    for entry in entries {
        ir.push_str(&format!(
            "@{SLOT_PREFIX}{name} = external global ptr\n",
            name = entry.name
        ));
    }
    for entry in entries {
        let arguments = entry
            .params
            .iter()
            .enumerate()
            .map(|(index, kind)| format!("{kind} %a{index}"))
            .collect::<Vec<_>>()
            .join(", ");
        ir.push_str(&format!(
            "define {ret} @{name}({declaration}) {{\nentry:\n  \
             %alvo = load ptr, ptr @{SLOT_PREFIX}{name}\n",
            ret = entry.ret,
            name = entry.name,
            declaration = entry.declaration_params(),
        ));
        if entry.ret == "void" {
            ir.push_str(&format!("  call void %alvo({arguments})\n  ret void\n}}\n"));
        } else {
            ir.push_str(&format!(
                "  %r = call {ret} %alvo({arguments})\n  ret {ret} %r\n}}\n",
                ret = entry.ret
            ));
        }
    }
    ir
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Constrói uma assinatura para os testes de texto e de contrato.
    fn signature(name: &str, ret: &str, params: &[&str]) -> FunctionSignature {
        FunctionSignature {
            name: name.to_owned(),
            ret: ret.to_owned(),
            params: params.iter().map(|p| (*p).to_owned()).collect(),
            var_arg: false,
        }
    }

    /// O trampolim devolve o valor da implementação e repassa os argumentos.
    #[test]
    fn stub_module_carries_the_signature() {
        let ir = stub_module_ir(&[
            signature("dartforge_entry", "void", &[]),
            signature("df_fn_1", "i64", &["i64", "i64"]),
        ]);
        assert!(ir.contains("@__dfslot$dartforge_entry = external global ptr"));
        assert!(ir.contains("define void @dartforge_entry() {"));
        assert!(ir.contains("call void %alvo()"));
        assert!(ir.contains("define i64 @df_fn_1(i64 %a0, i64 %a1) {"));
        assert!(ir.contains("%r = call i64 %alvo(i64 %a0, i64 %a1)"));
        assert!(ir.contains("ret i64 %r"));
    }

    /// Mudança de assinatura é recusada nomeando o símbolo e as duas formas.
    #[test]
    fn contract_rejects_a_changed_signature() {
        let before = vec![signature("df_fn_0", "i64", &[])];
        let after = vec![signature("df_fn_0", "i64", &["i64"])];
        let error = check_contract(&before, &[], &after, &[], &HashSet::new()).unwrap_err();
        assert!(
            error.contains("a assinatura de df_fn_0 mudou de i64 () para i64 (i64)"),
            "{error}"
        );
        assert!(error.contains("reiniciar a sessão"), "{error}");
    }

    /// Função que desaparece mantém o corpo antigo: não é recusa.
    #[test]
    fn contract_keeps_a_vanished_function() {
        let before = vec![signature("df_fn_0", "i64", &[])];
        assert!(check_contract(&before, &[], &[], &[], &HashSet::new()).is_ok());
    }

    /// Um id de classe que passa a ser de outra classe é recusado.
    #[test]
    fn class_names_reject_a_renumbered_class() {
        let before = vec![(922, "Contador".to_owned())];
        let error = check_class_names(&before, &[(922, "Novo".to_owned()), (923, "Contador".to_owned())]).unwrap_err();
        assert!(error.contains("era Contador e passou a ser Novo"), "{error}");
        assert!(check_class_names(&before, &[(922, "Contador".to_owned()), (923, "Novo".to_owned())]).is_ok());
    }

    /// Mudança no número de campos de uma classe já construída é recusada.
    #[test]
    fn contract_rejects_a_changed_class_layout() {
        let error = check_contract(&[], &[(0, 1)], &[], &[(0, 2)], &HashSet::new()).unwrap_err();
        assert!(
            error.contains("a classe de id 0 tinha 1 campos e passou a ter 2"),
            "{error}"
        );
        let same = check_contract(&[], &[(0, 1)], &[], &[(0, 1), (1, 3)], &HashSet::new());
        assert!(same.is_ok());
    }

    /// Corpo diferente com a mesma assinatura é o caso aceito da versão 1.
    #[test]
    fn contract_accepts_a_body_change() {
        let before = vec![
            signature("df_fn_0", "i64", &[]),
            signature("dartforge_entry", "void", &[]),
        ];
        let after = before.clone();
        assert!(check_contract(&before, &[(0, 2)], &after, &[(0, 2)], &HashSet::new()).is_ok());
    }

    /// Referência externa desconhecida é recusada antes de qualquer efeito.
    #[test]
    fn references_must_be_resolvable() {
        let error = check_references(
            &["dartforge_print_i64".to_owned(), "minha_ffi".to_owned()],
            &["df_fn_0"],
            &[],
            &HashSet::new(),
            true,
        )
        .unwrap_err();
        assert!(error.contains("minha_ffi"), "{error}");
        assert!(
            check_references(
                &["dartforge_print_i64".to_owned(), "df_fn_0".to_owned()],
                &["df_fn_0"],
                &[],
                &HashSet::new(),
                true,
            )
            .is_ok()
        );
    }

    #[test]
    fn references_accept_exports_of_the_sdk_loaded_in_this_session() {
        let mut external = HashSet::new();
        external.insert("df.sdk_teste".to_owned());
        assert!(check_references(&["df.sdk_teste".to_owned()], &[], &[], &external, false).is_ok());
        let error = check_references(&["df.sdk_ausente".to_owned()], &[], &[], &external, false).unwrap_err();
        assert!(error.contains("df.sdk_ausente"), "{error}");
        let error = check_references(&["dartforge_object_new".to_owned()], &[], &[], &external, false).unwrap_err();
        assert!(error.contains("dartforge_object_new"), "{error}");
    }

    /// Um trampolim criado por uma recarga que falhou tem célula nula. Ele só
    /// pode satisfazer referências externas depois de uma geração publicá-lo.
    #[test]
    fn references_reject_unimplemented_trampoline() {
        let mut entries = BTreeMap::new();
        entries.insert("df_fn_1".to_owned(), Entry {
            signature: signature("df_fn_1", "i64", &[]),
            slot: Box::new(AtomicUsize::new(0)),
            generation: 0,
        });
        let mut modules = vec![Reloadable {
            name: "app".to_owned(),
            generation: 1,
            entries,
            layouts: Vec::new(),
            class_names: Vec::new(),
            fields: Vec::new(),
            aliases: BTreeMap::new(),
            globals: Vec::new(),
            generations: Vec::new(),
            stubs: Vec::new(),
            published_list: HashMap::new(),
        }];
        let reference = ["df_fn_1".to_owned()];
        let error = check_references(&reference, &[], &modules, &HashSet::new(), true).unwrap_err();
        assert!(error.contains("df_fn_1"), "{error}");

        modules[0].entries.get_mut("df_fn_1").unwrap().generation = 2;
        assert!(check_references(&reference, &[], &modules, &HashSet::new(), true).is_ok());
    }

    /// Ciclo completo sobre IR direto, sem passar pelo front-end Dart.
    ///
    /// Vive aqui, e não em `tests/hot_reload.rs`, porque não precisa do
    /// compilador: exercita o mecanismo — trampolim, célula, geração — com o
    /// mínimo de IR possível, e continua valendo mesmo que o emissor mude.
    #[test]
    #[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
    fn full_cycle_with_direct_ir() {
        let ir = |value: i64| format!("define i64 @df_fn_0() {{\n  ret i64 {value}\n}}\n");
        let mut session = JitSession::new().expect("sessão");
        let first_one = session
            .add_reloadable_module("app", &ir(1))
            .expect("geração 1");
        assert_eq!(
            (first_one.generation, first_one.entries, first_one.new_entries),
            (1, 1, 1)
        );
        assert_eq!(session.stable_entries("app"), vec!["df_fn_0"]);

        let input = session.stable_entry("df_fn_0").expect("entrada estável");
        assert_eq!(input.call(&session).unwrap(), 1);

        let second_one = session.hot_reload("app", &ir(2)).expect("geração 2");
        assert_eq!((second_one.generation, second_one.new_entries), (2, 0));
        assert_eq!(input.call(&session).unwrap(), 2);
        // A geração 1 não é mais alcançável: descarregada.
        assert_eq!(session.retained_generations(), 1);

        // Chamar com argumento uma entrada `i64 ()` é erro, não ABI errada.
        let error = input.call_with(&session, 7).unwrap_err();
        assert_eq!(error.stage, "lookup");
    }

    /// Promoção: um módulo simples ganha trampolins na primeira recarga.
    ///
    /// Antes da promoção não há entrada estável, e um endereço obtido da versão
    /// simples é o do corpo — por isso a promoção é a única operação destrutiva
    /// do hot reload, e por isso ela é medida e relatada em `promoted`.
    #[test]
    #[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
    fn simple_module_promotion() {
        let ir = |value: i64| format!("define i64 @df_fn_0() {{\n  ret i64 {value}\n}}\n");
        let mut session = JitSession::new().expect("sessão");
        session.add_ir_module("app", &ir(1)).expect("módulo simples");
        assert!(session.stable_entry("df_fn_0").is_err());

        let report_data = session.hot_reload("app", &ir(2)).expect("promoção");
        assert!(report_data.promoted);
        assert_eq!((report_data.generation, report_data.new_entries), (1, 1));
        assert_eq!(session.module_names(), vec!["app"]);

        let input = session.stable_entry("df_fn_0").expect("entrada estável");
        assert_eq!(input.call(&session).unwrap(), 2);
        // Da segunda recarga em diante já não há nada destrutivo a fazer.
        let report_data = session.hot_reload("app", &ir(3)).expect("geração 2");
        assert!(!report_data.promoted);
        assert_eq!(input.call(&session).unwrap(), 3);
    }

    /// Contrato conferido na promoção usa a impressão digital do módulo simples.
    #[test]
    #[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
    fn promotion_rejects_incompatible_contract_without_destroying_anything() {
        let mut session = JitSession::new().expect("sessão");
        session
            .add_ir_module("app", "define i64 @df_fn_0() {\n  ret i64 1\n}\n")
            .expect("módulo simples");
        let error = session
            .hot_reload("app", "define i64 @df_fn_0(i64 %a0) {\n  ret i64 %a0\n}\n")
            .expect_err("assinatura alterada tem de ser recusada");
        assert_eq!(error.stage, "contract");
        // O módulo simples continua carregado: a recusa vem antes da promoção.
        assert_eq!(
            session.lookup("df_fn_0").map(|address| address != 0),
            Ok(true)
        );
        assert_eq!(session.retained_generations(), 0);
    }

    /// Variádicas ficam fora do contrato do emissor nativo.
    #[test]
    fn contract_rejects_variadics() {
        let mut variadic_one = signature("df_fn_0", "i64", &["i64"]);
        variadic_one.var_arg = true;
        let error = check_contract(&[], &[], &[variadic_one], &[], &HashSet::new()).unwrap_err();
        assert!(error.contains("variádica"), "{error}");
    }
}

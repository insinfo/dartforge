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
//!   chamador  ──►  @df_fn_0            (trampolim, permanente)
//!                    │ load  @__dfslot$df_fn_0
//!                    ▼
//!                  célula ──► @df_fn_0$gen1   (geração 1, retida)
//!                         └─► @df_fn_0$gen2   (geração 2, publicada)
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
//! Gerações antigas **não são liberadas**. Ver [`JitSession::hot_reload`].

use std::collections::{BTreeMap, HashSet};
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
    /// Aposentadoria do código antigo; zero enquanto a política é de retenção.
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
    /// Gerações retidas em memória depois desta recarga, incluindo ela.
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
    /// Layout nominal das classes da versão viva: `(class_id, campos)`.
    layouts: Vec<(i64, i64)>,
    /// Nomes das classes registradas pela versão viva: `(class_id, nome)`.
    class_names: Vec<(i64, String)>,
    /// Globais mutáveis da geração ativa, reiniciadas em `run_entry`/`run_main`.
    pub(crate) globals: Vec<ffi::MutableGlobal>,
    /// Rastreadores das gerações, **retidos** até o encerramento da sessão.
    generations: Vec<ffi::ResourceTracker>,
    /// Rastreadores dos módulos de trampolim; nunca descarregados.
    stubs: Vec<ffi::ResourceTracker>,
}

/// Endereço de uma entrada estável, com a assinatura que a sessão registrou.
///
/// É o que permite provar a propriedade central do hot reload sem `unsafe` no
/// chamador: o endereço é obtido **antes** da recarga e continua correto depois
/// dela, porque é o endereço do trampolim, não o do corpo.
///
/// ```no_run
/// let mut sessao = dartforge_jit::JitSession::new()?;
/// sessao.add_reloadable_module("app", "define i64 @df_fn_0() { ret i64 1 }\n")?;
/// let entrada = sessao.stable_entry("df_fn_0")?;
/// assert_eq!(entrada.call(&sessao)?, 1);
/// sessao.hot_reload("app", "define i64 @df_fn_0() { ret i64 2 }\n")?;
/// // O mesmo endereço, capturado antes da edição, já executa o corpo novo.
/// assert_eq!(entrada.call(&sessao)?, 2);
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
    /// let mut sessao = dartforge_jit::JitSession::new()?;
    /// let relatorio =
    ///     sessao.add_reloadable_module("app", "define void @dartforge_entry() { ret void }\n")?;
    /// assert_eq!(relatorio.generation, 1);
    /// assert!(!relatorio.promoted);
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
    /// 3. **Aposentar** o código antigo — que nesta versão **não acontece**. Ver
    ///    a política de retenção abaixo.
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
    /// # Retenção de memória, declarada
    ///
    /// Nenhuma geração é liberada antes do fim da sessão. Provar que é seguro
    /// descarregar uma geração exigiria saber que nenhuma das suas funções está
    /// em nenhum quadro de pilha de nenhuma thread, e a sessão não tem essa
    /// informação: o código gerado não publica safepoints e o LLVM não verifica
    /// nada disso. Descarregar sem essa prova transformaria código em memória
    /// liberada debaixo de um `call` em andamento.
    ///
    /// O custo é **linear no número de recargas**: cada recarga retém o código
    /// nativo e as constantes daquela geração. Um laço de desenvolvimento longo
    /// cresce em memória até o processo ser reiniciado. Esse é o compromisso
    /// aceito para a versão 1, e está registrado em `docs/JIT.md`.
    ///
    /// A única remoção que acontece é a de uma geração que **falhou antes de ser
    /// publicada**: essa é demonstravelmente segura, porque nenhum endereço dela
    /// chegou a nenhuma célula e nada pode tê-la chamado.
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
    /// let mut sessao = dartforge_jit::JitSession::new()?;
    /// sessao.add_reloadable_module("app", "define void @dartforge_entry() { ret void }\n")?;
    /// let relatorio = sessao.hot_reload("app", "define void @dartforge_entry() { ret void }\n")?;
    /// assert_eq!(relatorio.generation, 2);
    /// # Ok::<(), dartforge_jit::JitError>(())
    /// ```
    pub fn hot_reload(&mut self, name: &str, novo_ir: &str) -> Result<HotReloadReport, JitError> {
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
        self.install_generation(name, novo_ir, None)
    }

    /// [`JitSession::hot_reload`] com a publicação entregue a `ponto_seguro`,
    /// que a roda na thread do programa em execução, entre dois eventos do
    /// laço do isolado principal (`crate::vivo`), e só retorna depois.
    pub(crate) fn hot_reload_no_ponto_seguro(
        &mut self,
        name: &str,
        novo_ir: &str,
        ponto_seguro: &mut dyn FnMut(ffi::Tarefa),
    ) -> Result<HotReloadReport, JitError> {
        if !self.reloadables.iter().any(|module| module.name == name) {
            return Err(JitError {
                stage: "contract",
                message: format!("nenhum módulo recarregável chamado '{name}' está carregado nesta sessão"),
            });
        }
        self.install_generation(name, novo_ir, Some(ponto_seguro))
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
        ponto_seguro: Option<&mut dyn FnMut(ffi::Tarefa)>,
    ) -> Result<HotReloadReport, JitError> {
        let started = Instant::now();
        if let Some(motivo) = self.poisoned.clone() {
            return Err(JitError {
                stage: "poisoned",
                message: motivo,
            });
        }

        // ── Fase 1: preparar, sem tocar no que está executando ──────────────
        let phase = Instant::now();
        let parsed = ffi::parse_module(name, ir)
            .map_err(|detail| JitError::new("parse-ir", "IR inválido", detail))?;
        self.check_target(&parsed)?;
        let parse_ir = phase.elapsed();

        let phase = Instant::now();
        let signatures = parsed.signatures();
        let layouts = parsed.class_layouts();
        let class_names = parsed.class_names();
        let references = parsed.declarations();
        let existing = self.reloadables.iter().position(|m| m.name == name);
        let plain = self.plain_module_index(name, existing.is_some());
        // Um símbolo de outro módulo já ocupa o nome que viraria trampolim.
        // Detectar a colisão aqui é obrigatório na promoção: depois que o
        // módulo simples é removido, uma falha ao publicar o trampolim deixa
        // a sessão envenenada e destrói a versão que ainda funcionava.
        let ocupados: HashSet<&str> = self.modules.iter().enumerate()
            .filter(|(index, module)| !module.removed && Some(*index) != plain)
            .flat_map(|(_, module)| module.signatures.iter().map(|s| s.name.as_str()))
            .chain(self.reloadables.iter().enumerate()
                .filter(|(index, _)| Some(*index) != existing)
                .flat_map(|(_, module)| module.entries.keys().map(String::as_str)))
            .collect();
        if let Some(colisao) = signatures.iter().find(|s| ocupados.contains(s.name.as_str())) {
            return Err(JitError {
                stage: "contract",
                message: format!(
                    "a função {} já pertence a outro módulo da sessão; a recarga não pode publicar uma segunda entrada com esse nome",
                    colisao.name
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
        check_contract(&previous, previous_layouts, &signatures, &layouts).map_err(|message| {
            JitError {
                stage: "contract",
                message,
            }
        })?;
        check_class_names(previous_names, &class_names).map_err(|message| JitError { stage: "contract", message })?;
        // A publicação refaz os registros do programa (tabelas de métodos,
        // regras da RTI) quando já houve uma geração: o programa em execução
        // tem os da anterior. As funções de registro são entradas da própria
        // geração; o runtime que as recebe é o da sessão.
        let registros = if existing.is_some() {
            let tem = |nome: &str| signatures.iter().any(|s| s.name == nome && s.ret == "void" && s.params.is_empty());
            let area = tem(PREPARO_DA_AREA);
            let registrar = tem(REGISTRO_DO_PROGRAMA);
            let rti = tem(INICIO_DA_RTI);
            if area || registrar || rti {
                let runtime = match &self.sdk_dll {
                    Some(dll) => ffi::RuntimeDaRecarga::da_biblioteca(dll)
                        .map_err(|detail| JitError::new("contract", "o runtime da sessão não publica gerações", detail))?,
                    None => ffi::RuntimeDaRecarga::embutido(),
                };
                Some((runtime, area, registrar, rti))
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
        let novos_sdk = if let Some(dll) = &self.sdk_dll {
            let mut pedidos: Vec<String> = references
                .iter()
                .filter(|reference| {
                    !self.is_known_external(reference)
                        && !self.externos_do_sdk.contains(*reference)
                        && !known.contains(&reference.as_str())
                        && !self.reloadables.iter().any(|m| m.entries.contains_key(*reference))
                })
                .cloned()
                .collect();
            pedidos.sort();
            pedidos.dedup();
            ffi::Lljit::exported_symbols_in_dll(dll, &pedidos)
                .map_err(|detail| JitError::new("sdk", "não foi possível ler as exportações do SDK", detail))?
        } else {
            Vec::new()
        };
        let mut externos = self.externos_do_sdk.clone();
        externos.extend(novos_sdk.iter().cloned());
        check_references(&references, &known, &self.reloadables, &externos, self.sdk_dll.is_none()).map_err(|message| JitError {
            stage: "contract",
            message,
        })?;
        if !novos_sdk.is_empty() {
            let dll = self.sdk_dll.as_ref().expect("novos exports exigem DLL do SDK");
            let publicados = self.lljit.define_symbols_from_dll(dll, &novos_sdk, false)
                .map_err(|detail| JitError::new("sdk", "não foi possível publicar novos exports do SDK", detail))?;
            self.externos_do_sdk.extend(publicados);
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
            if !static_do_programa(&next.logical_name) {
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
                    let orfas = std::mem::take(&mut slots);
                    self.register_unimplemented(name, orfas, &published, stub_tracker.take());
                    return Err(self.poison_if(promoted, error));
                }
            }
        }
        // O código da geração nova já foi ligado, mas as entradas estáveis
        // ainda chamam a antiga. Preserve apenas os estáticos do programa;
        // caches de seletor guardam endereços da geração anterior e devem
        // começar vazios. A cópia em si é da publicação: o programa pode
        // estar escrevendo neles até o ponto seguro.
        let mut copias = Vec::new();
        for next in &globals {
            if !static_do_programa(&next.logical_name) {
                continue;
            }
            let Some(previous) = previous_globals.iter().find(|g| g.logical_name == next.logical_name) else {
                continue;
            };
            match self.lljit.copia_de_global(previous, next) {
                Ok(copia) => copias.push(copia),
                Err(detail) => {
                    let error = JitError::new("link", "não foi possível preservar um estático", detail);
                    let _ = tracker.remove();
                    let orfas = std::mem::take(&mut slots);
                    self.register_unimplemented(name, orfas, &published, stub_tracker.take());
                    return Err(self.poison_if(promoted, error));
                }
            }
        }
        // Os trampolins das funções de registro, resolvidos antes da
        // publicação (a tarefa só leva endereços).
        let registros = match registros {
            Some((runtime, area, registrar, rti)) => {
                let endereco = |sessao: &Self, nome: &str, pedido: bool| -> Result<Option<u64>, JitError> {
                    if pedido { sessao.lookup(nome).map(Some) } else { Ok(None) }
                };
                Some((
                    runtime,
                    endereco(self, PREPARO_DA_AREA, area)?,
                    endereco(self, REGISTRO_DO_PROGRAMA, registrar)?,
                    endereco(self, INICIO_DA_RTI, rti)?,
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
        let mut trocas = Vec::with_capacity(published.len());
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
            trocas.push((ffi::Celula::de(&entry.slot), *address as usize));
        }
        // Com um programa em execução, a tarefa roda no ponto seguro do
        // isolado principal e para antes os demais isolados no ponto seguro
        // deles: nenhum quadro Dart de nenhum isolado vê a troca pela metade.
        let tarefa: ffi::Tarefa = Box::new(move || {
            let parada = registros.map(|(runtime, ..)| (runtime, runtime.parar_isolados()));
            for copia in copias {
                copia.aplicar();
            }
            for (celula, implementacao) in trocas {
                celula.publicar(implementacao);
            }
            if let Some((runtime, area, registrar, rti)) = registros {
                runtime.publicar_registros(area, registrar, rti);
                if let Some((_, p)) = parada {
                    runtime.liberar_isolados(p, area, registrar, rti);
                }
            }
        });
        let espera = Instant::now();
        let safepoint_wait = match ponto_seguro {
            Some(executar) => {
                executar(tarefa);
                espera.elapsed()
            }
            None => {
                tarefa();
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
        module.globals = globals;
        module.generations.push(tracker);
        if let Some(created) = stub_tracker {
            module.stubs.push(created);
        }
        let publish = phase.elapsed().saturating_sub(safepoint_wait);

        // Fase 3: aposentar. A política de retenção não libera nada, e o campo
        // existe para que o relatório não esconda a etapa que falta.
        let retire = Duration::ZERO;

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
            ir_bytes: ir.len(),
            entries: published.len(),
            new_entries: fresh.len(),
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
            globals: Vec::new(),
            generations: Vec::new(),
            stubs: Vec::new(),
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
        for (nome, slot) in slots {
            if let Some(signature) = published.iter().find(|s| s.name == nome) {
                module.entries.insert(
                    nome,
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
fn static_do_programa(name: &str) -> bool {
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
            Some(novo) if novo.text() != old.text() => {
                return Err(format!(
                    "a assinatura de {} mudou de {} para {}; \
                     mudança de contrato de chamada exige reiniciar a sessão",
                    old.name,
                    old.text(),
                    novo.text()
                ));
            }
            Some(_) => {}
        }
    }
    for (class, fields) in previous_layouts {
        if let Some((_, novos)) = layouts
            .iter()
            .find(|(candidate, novos)| candidate == class && novos != fields)
        {
            return Err(format!(
                "a classe de id {class} tinha {fields} campos e passou a ter {novos}; \
                 os objetos já vivos no heap gerenciado mantêm o layout antigo, então a recarga \
                 é recusada — reinicie a sessão"
            ));
        }
    }
    Ok(())
}

/// As funções que a publicação de uma geração chama: a área de globais com
/// o layout dela e os registros que ela refaz.
const PREPARO_DA_AREA: &str = "df.preparar_area";
const REGISTRO_DO_PROGRAMA: &str = "df.registrar.programa";
const INICIO_DA_RTI: &str = "dartforge_rti_iniciar";

/// Os ids de classe do programa são posicionais: uma edição que insere ou
/// reordena classes troca a classe de um id que já tem objetos no heap.
///
/// # Erros
/// Nomeia o id, a classe da versão viva e a do código novo.
fn check_class_names(previous: &[(i64, String)], new: &[(i64, String)]) -> Result<(), String> {
    for (id, antigo) in previous {
        if let Some((_, novo)) = new.iter().find(|(candidato, novo)| candidato == id && novo != antigo) {
            return Err(format!(
                "a classe de id {id} era {antigo} e passou a ser {novo} (classes inseridas ou reordenadas); \
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
    externos_do_sdk: &HashSet<String>,
    runtime_embutido: bool,
) -> Result<(), String> {
    for reference in references {
        if (runtime_embutido && ffi::is_known_external(reference))
            || crate::CRT_SYMBOLS.contains(&reference.as_str())
            || reference.starts_with("llvm.")
            || externos_do_sdk.contains(reference)
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
        let antes = vec![signature("df_fn_0", "i64", &[])];
        let depois = vec![signature("df_fn_0", "i64", &["i64"])];
        let erro = check_contract(&antes, &[], &depois, &[]).unwrap_err();
        assert!(
            erro.contains("a assinatura de df_fn_0 mudou de i64 () para i64 (i64)"),
            "{erro}"
        );
        assert!(erro.contains("reiniciar a sessão"), "{erro}");
    }

    /// Função que desaparece mantém o corpo antigo: não é recusa.
    #[test]
    fn contract_keeps_a_vanished_function() {
        let antes = vec![signature("df_fn_0", "i64", &[])];
        assert!(check_contract(&antes, &[], &[], &[]).is_ok());
    }

    /// Um id de classe que passa a ser de outra classe é recusado.
    #[test]
    fn class_names_reject_a_renumbered_class() {
        let antes = vec![(922, "Contador".to_owned())];
        let erro = check_class_names(&antes, &[(922, "Novo".to_owned()), (923, "Contador".to_owned())]).unwrap_err();
        assert!(erro.contains("era Contador e passou a ser Novo"), "{erro}");
        assert!(check_class_names(&antes, &[(922, "Contador".to_owned()), (923, "Novo".to_owned())]).is_ok());
    }

    /// Mudança no número de campos de uma classe já construída é recusada.
    #[test]
    fn contract_rejects_a_changed_class_layout() {
        let erro = check_contract(&[], &[(0, 1)], &[], &[(0, 2)]).unwrap_err();
        assert!(
            erro.contains("a classe de id 0 tinha 1 campos e passou a ter 2"),
            "{erro}"
        );
        let mesmo = check_contract(&[], &[(0, 1)], &[], &[(0, 1), (1, 3)]);
        assert!(mesmo.is_ok());
    }

    /// Corpo diferente com a mesma assinatura é o caso aceito da versão 1.
    #[test]
    fn contract_accepts_a_body_change() {
        let antes = vec![
            signature("df_fn_0", "i64", &[]),
            signature("dartforge_entry", "void", &[]),
        ];
        let depois = antes.clone();
        assert!(check_contract(&antes, &[(0, 2)], &depois, &[(0, 2)]).is_ok());
    }

    /// Referência externa desconhecida é recusada antes de qualquer efeito.
    #[test]
    fn references_must_be_resolvable() {
        let erro = check_references(
            &["dartforge_print_i64".to_owned(), "minha_ffi".to_owned()],
            &["df_fn_0"],
            &[],
            &HashSet::new(),
            true,
        )
        .unwrap_err();
        assert!(erro.contains("minha_ffi"), "{erro}");
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
        let mut externos = HashSet::new();
        externos.insert("df.sdk_teste".to_owned());
        assert!(check_references(&["df.sdk_teste".to_owned()], &[], &[], &externos, false).is_ok());
        let erro = check_references(&["df.sdk_ausente".to_owned()], &[], &[], &externos, false).unwrap_err();
        assert!(erro.contains("df.sdk_ausente"), "{erro}");
        let erro = check_references(&["dartforge_object_new".to_owned()], &[], &[], &externos, false).unwrap_err();
        assert!(erro.contains("dartforge_object_new"), "{erro}");
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
            globals: Vec::new(),
            generations: Vec::new(),
            stubs: Vec::new(),
        }];
        let reference = ["df_fn_1".to_owned()];
        let erro = check_references(&reference, &[], &modules, &HashSet::new(), true).unwrap_err();
        assert!(erro.contains("df_fn_1"), "{erro}");

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
    fn ciclo_completo_com_ir_direto() {
        let ir = |valor: i64| format!("define i64 @df_fn_0() {{\n  ret i64 {valor}\n}}\n");
        let mut sessao = JitSession::new().expect("sessão");
        let primeira = sessao
            .add_reloadable_module("app", &ir(1))
            .expect("geração 1");
        assert_eq!(
            (primeira.generation, primeira.entries, primeira.new_entries),
            (1, 1, 1)
        );
        assert_eq!(sessao.stable_entries("app"), vec!["df_fn_0"]);

        let entrada = sessao.stable_entry("df_fn_0").expect("entrada estável");
        assert_eq!(entrada.call(&sessao).unwrap(), 1);

        let segunda = sessao.hot_reload("app", &ir(2)).expect("geração 2");
        assert_eq!((segunda.generation, segunda.new_entries), (2, 0));
        assert_eq!(entrada.call(&sessao).unwrap(), 2);
        assert_eq!(sessao.retained_generations(), 2);

        // Chamar com argumento uma entrada `i64 ()` é erro, não ABI errada.
        let erro = entrada.call_with(&sessao, 7).unwrap_err();
        assert_eq!(erro.stage, "lookup");
    }

    /// Promoção: um módulo simples ganha trampolins na primeira recarga.
    ///
    /// Antes da promoção não há entrada estável, e um endereço obtido da versão
    /// simples é o do corpo — por isso a promoção é a única operação destrutiva
    /// do hot reload, e por isso ela é medida e relatada em `promoted`.
    #[test]
    #[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
    fn promocao_de_modulo_simples() {
        let ir = |valor: i64| format!("define i64 @df_fn_0() {{\n  ret i64 {valor}\n}}\n");
        let mut sessao = JitSession::new().expect("sessão");
        sessao.add_ir_module("app", &ir(1)).expect("módulo simples");
        assert!(sessao.stable_entry("df_fn_0").is_err());

        let relatorio = sessao.hot_reload("app", &ir(2)).expect("promoção");
        assert!(relatorio.promoted);
        assert_eq!((relatorio.generation, relatorio.new_entries), (1, 1));
        assert_eq!(sessao.module_names(), vec!["app"]);

        let entrada = sessao.stable_entry("df_fn_0").expect("entrada estável");
        assert_eq!(entrada.call(&sessao).unwrap(), 2);
        // Da segunda recarga em diante já não há nada destrutivo a fazer.
        let relatorio = sessao.hot_reload("app", &ir(3)).expect("geração 2");
        assert!(!relatorio.promoted);
        assert_eq!(entrada.call(&sessao).unwrap(), 3);
    }

    /// Contrato conferido na promoção usa a impressão digital do módulo simples.
    #[test]
    #[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
    fn promocao_recusa_contrato_incompativel_sem_destruir_nada() {
        let mut sessao = JitSession::new().expect("sessão");
        sessao
            .add_ir_module("app", "define i64 @df_fn_0() {\n  ret i64 1\n}\n")
            .expect("módulo simples");
        let erro = sessao
            .hot_reload("app", "define i64 @df_fn_0(i64 %a0) {\n  ret i64 %a0\n}\n")
            .expect_err("assinatura alterada tem de ser recusada");
        assert_eq!(erro.stage, "contract");
        // O módulo simples continua carregado: a recusa vem antes da promoção.
        assert_eq!(
            sessao.lookup("df_fn_0").map(|address| address != 0),
            Ok(true)
        );
        assert_eq!(sessao.retained_generations(), 0);
    }

    /// Variádicas ficam fora do contrato do emissor nativo.
    #[test]
    fn contract_rejects_variadics() {
        let mut variadica = signature("df_fn_0", "i64", &["i64"]);
        variadica.var_arg = true;
        let erro = check_contract(&[], &[], &[variadica], &[]).unwrap_err();
        assert!(erro.contains("variádica"), "{erro}");
    }
}

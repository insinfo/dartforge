//! Gerador de LLVM IR a partir da HIR nativa.

pub mod abi_c;
pub mod externs;
mod depuracao;
mod raizes;
mod rastro;
pub mod verificar_mapas;
pub mod verificar_sombra;
mod simd;
// As emissões que dependem da representação dos valores do runtime, uma por
// pacote do espaço unificado (docs/NATIVO-ESPACO-UNIFICADO.md §4.2, passo 4).
mod caixas_ir;
mod listas_ir;
mod textos_ir;
mod tipados_ir;

use dartforge_runtime::layout;
/// Maior índice de campo lido em linha (o contrato de layout).
const CAMPOS_EM_LINHA: usize = layout::CAMPOS_EM_LINHA;
/// O maior corpo, em palavras, com alocação em linha (a TLAB de `w` palavras,
/// `layout::contexto::tlab_cursor(w)`): um objeto de até `TLAB_N` campos.
const TLAB_N: i64 = layout::TLAB_N as i64;
/// A máscara de `e_objeto` (`h & (7 | i64::MIN) == 2`).
const MASCARA_DE_OBJETO: i64 = 7 | i64::MIN;
mod seletores;
#[cfg(test)]
mod testes;

use crate::hir::*;
use std::fmt::Write;

pub struct LlvmEmitter<'a> {
    module: &'a Module,
    /// As funções do módulo pelo símbolo (a primeira com cada nome, como o
    /// `find` linear que esta tabela substituiu: ele era feito a cada
    /// `CallStatic` e tornava a emissão quadrática — dezenas de minutos no
    /// new_sali/backend, docs/NATIVO-PROJETOS-REAIS.md).
    funcao_por_simbolo: std::collections::HashMap<&'a str, &'a Function>,
    out: String,
    /// As constantes de string, na ordem de emissão (determinística); o
    /// índice de cada uma é a posição.
    string_constants: Vec<Vec<u8>>,
    /// O índice de cada constante pelo conteúdo: coletar e consultar sem
    /// percorrer a tabela (antes, quadrático no número de literais).
    indice_de_string: std::collections::HashMap<Vec<u8>, usize>,
    /// Tipo de cada valor da funcao sendo emitida, para coercao de operandos.
    tipos: std::collections::HashMap<ValueId, Type>,
    /// Contador dos temporarios de coercao (%c0, %c1, ...), por funcao.
    prox_coercao: u32,
    /// Conversoes que uma entrada de phi exige, atribuidas ao bloco de ORIGEM:
    /// (bloco, nome do temporario, tipo de origem, valor, tipo do phi).
    conv_phi: Vec<(u32, String, Type, ValueId, Type)>,
    /// Tipo guardado por cada `alloca` da função (para o `store`).
    apontado: std::collections::HashMap<ValueId, Type>,
    /// Os `alloca` `Ref` cujo endereço sai da função (a captura por endereço
    /// de uma função local direta, `lower/funcoes_diretas.rs`): moram no
    /// próprio slot do quadro de raízes, e quem recebe o endereço grava
    /// onde o coletor enxerga.
    allocas_no_quadro: std::collections::HashSet<ValueId>,
    /// G: slot de raiz de cada valor `Ref` da função (SSA ou `alloca`).
    slots: std::collections::HashMap<ValueId, usize>,
    /// A função abriu um quadro de raízes (`%gcq`).
    tem_frame: bool,
    /// A função leu `%ctx` (`dartforge_contexto`, o contexto da thread do
    /// runtime) na entrada: exceção pendente e pilha-sombra sem chamada.
    tem_ctx: bool,
    /// O rótulo LLVM em que termina cada bloco da HIR que se divide em
    /// vários (a alocação em linha, [`LlvmEmitter::alocacao_em_linha`]): é
    /// o predecessor que os `phi` dos sucessores nomeiam.
    rotulos_de_saida: std::collections::HashMap<u32, String>,
    /// O rótulo LLVM do trecho em emissão (o do bloco, ou o último que uma
    /// instrução dividida abriu): o predecessor de um `phi` interno.
    rotulo_atual: String,
    // --- P1 (closures, α) ---
    /// Vetores constantes de `i64` (`@df.arr.<k>`): assinaturas e descritores.
    vetores: Vec<Vec<i64>>,
    vetor_de: std::collections::HashMap<Vec<i64>, usize>,
    /// Vetores constantes de endereços de função (`@df.fns.<k>`).
    tabelas_de_funcoes: Vec<Vec<String>>,
    tabela_de_funcoes_de: std::collections::HashMap<Vec<String>, usize>,
    /// Os nomes dos argumentos nomeados dos descritores do módulo: o
    /// descritor só guarda o hash, e o `Invocation` de um `noSuchMethod`
    /// precisa do nome (`dartforge_registrar_nome_de_argumento`).
    nomes_de_argumento: std::collections::BTreeSet<String>,
    // --- P5c (SDK da fonte, δ; `seletores.rs`) ---
    /// Pontos de chamada por seletor já emitidos (um cache cada).
    caches_de_seletor: usize,
    /// O símbolo da função em emissão e quantos caches de seletor ela já
    /// abriu: o nome do slot de um cache é `(função, posição)`, estável
    /// entre gerações de uma recarga (J04).
    funcao_atual: String,
    cache_na_funcao: usize,
    /// O índice de cada nome de slot do layout da geração viva (recarga).
    indice_de_slot: std::collections::HashMap<i64, usize>,
    /// Textos dos seletores, na ordem do primeiro uso.
    nomes_de_seletor: Vec<String>,
    /// Assinatura de cada símbolo chamado, para declarar o que o módulo não
    /// define.
    externos: std::collections::BTreeMap<String, String>,
    /// Símbolos definidos em `comdat` (`seletores::ligacao_de`).
    comdats: Vec<String>,
    /// `DARTFORGE_RASTRO=1` na compilação: cada função conta a entrada e a
    /// saída ao runtime, que mostra a pilha de funções Dart numa exceção
    /// (`DARTFORGE_DEPURAR=1`). Só para depurar o SDK da fonte.
    rastro: Option<usize>,
    nomes_do_rastro: Vec<String>,
    // --- Área de globais por isolado ---
    /// O slot (`i64`) de cada global do módulo na área (`@dfg_…` e a
    /// bandeira `$ok`): os estáticos do Dart são por isolado, e cada
    /// isolado é uma thread com a sua área (`dartforge_area_de_globais`).
    slots_de_global: std::collections::HashMap<String, usize>,
    /// O hash do nome de cada slot, na ordem (o descritor da área): 0 nos
    /// caches de seletor.
    hashes_de_slot: Vec<i64>,
    /// O layout da área da geração em execução (hot reload): os slots que
    /// continuam mantêm o índice, os novos vêm depois (`emit_globais`).
    area_anterior: Option<Vec<i64>>,
    /// O descritor da área sem os nomes dos slots (`[chave, -n]`): o
    /// executável de produção, que nunca recarrega (`com_area_enxuta`).
    area_enxuta: bool,
    /// J05: os metadados de depuração, quando alguma função tem posições.
    depuracao: Option<depuracao::Depuracao>,
    // --- Espaço unificado (docs/NATIVO-ESPACO-UNIFICADO.md) ---
    /// Os literais de string são objetos estáticos do módulo (§2.11):
    /// verdadeiro no AOT e nos módulos do SDK; falso nos módulos do programa no
    /// JIT, cuja memória é liberada (J02).
    objetos_estaticos: bool,
    /// O endereço dos campos de um objeto por chamada a `@df.corpo` em vez
    /// das 15 instruções em linha (o perfil de desenvolvimento do programa,
    /// `-O0`): o IR do new_sali/backend tinha 465 mil cópias da sequência,
    /// 21% das linhas (docs/NATIVO-PROJETOS-REAIS.md, C9). A produção e os
    /// módulos do SDK continuam em linha.
    campos_por_chamada: bool,
    /// A alocação de instância por chamada ao `@df.nova_instancia` (a
    /// produção do programa com `nova_instancia` em
    /// [`LlvmEmitter::ajudantes_fora`]).
    alocacao_fora_de_linha: bool,
    /// Os ajudantes `@df.*` que vão `noinline` no módulo do programa de
    /// produção ([`ajudantes_fora_de_linha`]).
    ajudantes_fora: Vec<String>,
    /// O despacho por seletor fora de linha (`seletor` em
    /// [`LlvmEmitter::ajudantes_fora`]): o ponto de chamada passa o
    /// descritor estático do seletor (`@df.seld.<k>`: hash, nome,
    /// comprimento) ao `@df.seletor_d`, que tem o `@df.seletor` em linha —
    /// três argumentos em vez de cinco em cada um dos ~170 mil pontos do
    /// programa grande.
    seletor_compacto: bool,
    /// `optsize` em cada função do módulo (o programa grande na produção;
    /// o SDK de produção já o tem, `sdk_modulo::com_optsize`).
    otimizar_tamanho: bool,
    /// O módulo é do perfil de produção (`--optimize`): com
    /// [`Self::campos_por_chamada`], o `@df.corpo` vai `noinline` (o
    /// otimizador o poria de volta em linha).
    producao: bool,
    /// As tabelas de métodos são montadas na ligação
    /// (docs/NATIVO-PODA-DE-TABELAS.md §3.2): o módulo só declara a função
    /// de cada tabela, e o conteúdo vai para o resumo (`poda.rs`). É o SDK
    /// de produção.
    tabelas_na_ligacao: bool,
    /// O estado de cada pacote da emissão (§4.2, passo 4); os pacotes o usam
    /// conforme migram.
    #[allow(dead_code)]
    textos: textos_ir::EstadoDeTextos,
    #[allow(dead_code)]
    caixas: caixas_ir::EstadoDeCaixas,
    #[allow(dead_code)]
    listas: listas_ir::EstadoDeListas,
    #[allow(dead_code)]
    tipados: tipados_ir::EstadoDeTipados,
    /// Exceções por tabelas (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13):
    /// a decisão do passe `otimizar::tabelas` para a função em emissão —
    /// quem é `invoke`, os pousos e como cada `Return` sai. `None` no modelo
    /// de sempre (a pendência conferida depois de cada chamada), e o IR é
    /// então o de sempre, byte a byte.
    tab: Option<&'a TabelasDaFuncao>,
    /// `DARTFORGE_EFEITOS=conferir` ([`externs::conferir_efeitos`]): os nomes
    /// das externs conferidas neste módulo, na ordem do primeiro uso (o
    /// índice é o do global `@df.efn.<k>` com o texto).
    externs_conferidas: Vec<String>,
    /// Raízes por mapas de pilha (`--raizes=mapas`,
    /// docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §14.8): os valores SSA `Ref`
    /// vivos através de um ponto de coleta não vão para o quadro da
    /// pilha-sombra; cada um ganha um ponteiro `addrspace(1)` com os mesmos
    /// bits, mantido vivo depois de cada ponto de coleta
    /// (`llvm.fake.use`), e o `rewrite-statepoints-for-gc` do LLVM o põe no
    /// mapa de pilha da chamada. Só os `alloca` `Ref` continuam no quadro.
    mapas: bool,
    /// Com [`Self::mapas`], a análise de raízes da função em emissão.
    raizes_da_funcao: raizes::Raizes,
    /// Com [`Self::mapas`], os valores enraizados da função em emissão.
    enraizados: std::collections::HashSet<ValueId>,
    /// Raízes por mapas: as definições de `@df.vararg.<k>`, os intermediários
    /// das chamadas C variádicas feitas por uma função `gc`
    /// ([`Self::texto_da_chamada_c`]); vão ao módulo depois das funções.
    variadicas: Vec<String>,
    /// O build de conferência do percurso (`DARTFORGE_RAIZES_CONFERIR=1`
    /// com `--raizes=mapas`, docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §15.2,
    /// E2.5): numa função `gc`, o primeiro dos slots de conferência no fim
    /// do quadro de raízes e quantos são. Antes de cada ponto de coleta eles
    /// recebem exatamente os valores vivos nele (zero nos demais), e o
    /// runtime confere que o percurso por mapas visitou todos.
    conferencia: Option<(usize, usize)>,
    /// Raízes por mapas num módulo do JIT (§3.8): o gerenciador de memória do
    /// JIT acha o mapa de cada objeto e o registra; o módulo não chama o
    /// registro nem leva o `-ni:1` (o `LLJIT` recusa um módulo com a camada
    /// de dados diferente da dele, e o JIT não roda otimização de IR).
    mapas_no_jit: bool,
    /// Quantas funções `gc` o módulo emitiu (no Mach-O, o `.no_dead_strip`
    /// do mapa só vai a um módulo que tem mapa).
    funcoes_gc: usize,
    /// O rastro no formato da VM (§13.14, `llvm/rastro.rs`): os rótulos das
    /// chamadas e a tabela da seção `dfpcl`. `None`: sem tabela.
    rastro_vm: Option<rastro::Rastro>,
    /// O módulo vai para o JIT: a seção do rastro de cada objeto é
    /// registrada pelo gerenciador de memória da sessão (`crates/jit`), e o
    /// módulo não tem os símbolos do ligador nem a chamada de registro.
    rastro_no_jit: bool,
}

impl<'a> LlvmEmitter<'a> {
    pub fn new(module: &'a Module) -> Self {
        Self {
            module,
            out: String::new(),
            string_constants: Vec::new(),
            indice_de_string: std::collections::HashMap::new(),
            tipos: std::collections::HashMap::new(),
            prox_coercao: 0,
            conv_phi: Vec::new(),
            apontado: std::collections::HashMap::new(),
            allocas_no_quadro: std::collections::HashSet::new(),
            slots: std::collections::HashMap::new(),
            tem_frame: false,
            tem_ctx: false,
            rotulos_de_saida: std::collections::HashMap::new(),
            rotulo_atual: String::new(),

            vetores: Vec::new(),
            vetor_de: std::collections::HashMap::new(),
            tabelas_de_funcoes: Vec::new(),
            tabela_de_funcoes_de: std::collections::HashMap::new(),
            nomes_de_argumento: std::collections::BTreeSet::new(),
            caches_de_seletor: 0,
            funcao_atual: String::new(),
            cache_na_funcao: 0,
            indice_de_slot: std::collections::HashMap::new(),
            nomes_de_seletor: Vec::new(),
            externos: std::collections::BTreeMap::new(),
            comdats: Vec::new(),
            rastro: std::env::var("DARTFORGE_RASTRO").is_ok_and(|v| v == "1").then_some(0),
            nomes_do_rastro: Vec::new(),
            slots_de_global: std::collections::HashMap::new(),
            hashes_de_slot: Vec::new(),
            area_anterior: None,
            area_enxuta: false,
            // As posições servem às tabelas de linha só com `--depuracao`; sem
            // ela, são só do rastro simbólico (`rastro_vm`).
            depuracao: (module.dwarf && module.functions.iter().any(|f| f.depuracao.is_some())).then(depuracao::Depuracao::nova),
            funcao_por_simbolo: {
                let mut m = std::collections::HashMap::with_capacity(module.functions.len());
                for f in &module.functions {
                    m.entry(f.symbol.as_str()).or_insert(f);
                }
                m
            },
            objetos_estaticos: false,
            campos_por_chamada: false,
            producao: false,
            alocacao_fora_de_linha: false,
            ajudantes_fora: Vec::new(),
            otimizar_tamanho: false,
            seletor_compacto: false,
            tabelas_na_ligacao: false,
            textos: Default::default(),
            caixas: Default::default(),
            listas: Default::default(),
            tipados: Default::default(),
            tab: None,
            externs_conferidas: Vec::new(),
            mapas: false,
            raizes_da_funcao: raizes::Raizes::default(),
            enraizados: std::collections::HashSet::new(),
            variadicas: Vec::new(),
            conferencia: None,
            funcoes_gc: 0,
            mapas_no_jit: false,
            rastro_vm: None,
            rastro_no_jit: false,
        }
    }

    /// Veja [`LlvmEmitter::rastro_no_jit`].
    pub fn com_rastro_no_jit(mut self, sim: bool) -> Self {
        self.rastro_no_jit = sim;
        self
    }

    /// O rastro simbólico (§13.14): os rótulos antes das chamadas e a
    /// tabela da imagem (`alvo::rastro_simbolico`; no JIT, com
    /// [`LlvmEmitter::com_rastro_no_jit`]).
    pub fn com_rastro(mut self, sim: bool) -> Self {
        self.rastro_vm = sim.then(rastro::Rastro::novo);
        self
    }

    /// A chamada de registro dos mapas de pilha da imagem, por formato: a
    /// base da imagem PE (o runtime acha `.dfgcm` e `.llvm_st` pelos
    /// cabeçalhos); os limites da seção `dfgcm` que o `ld.lld` define; no
    /// Mach-O, os do `__llvm_stackmaps` do LLVM.
    fn chamada_de_registro_dos_mapas() -> &'static str {
        match crate::alvo::sistema() {
            crate::alvo::Sistema::Windows => "  call void @dartforge_registrar_mapa(ptr @__ImageBase)\n",
            crate::alvo::Sistema::Linux => "  call void @dartforge_registrar_mapa_secao(ptr @__start_dfgcm, ptr @__stop_dfgcm)\n",
            crate::alvo::Sistema::MacOs => {
                "  call void @dartforge_registrar_mapa_llvm(ptr @\"\\01section$start$__LLVM_STACKMAPS$__llvm_stackmaps\", ptr @\"\\01section$end$__LLVM_STACKMAPS$__llvm_stackmaps\")\n"
            }
        }
    }

    /// O registro dos campos do `dart:async` que o rastro percorre (§13.14).
    fn chamadas_de_registro_dos_campos_do_rastro(&self) -> String {
        let mut s = String::new();
        for (cid, nome, posicao) in &self.module.campos_do_rastro {
            let idx = self.string_const_index(nome.as_bytes()).unwrap_or(0);
            writeln!(s, "  call void @dartforge_registrar_campo_do_rastro(i64 {cid}, ptr @.str.{idx}, i64 {}, i64 {posicao})", nome.len()).unwrap();
        }
        // As entradas de tear-off dos ramos de stream.
        // Só as que sobraram da poda (a entrada some com quem a criava).
        for (simbolo, especie) in &self.module.tearoffs_do_rastro {
            if !self.module.functions.iter().any(|f| f.symbol == *simbolo) {
                continue;
            }
            writeln!(s, "  call void @dartforge_registrar_tearoff_do_rastro(ptr @{simbolo}, i64 {especie})").unwrap();
        }
        s
    }

    /// A chamada de registro da tabela do rastro da imagem (§13.14).
    fn chamada_de_registro_do_rastro(&self) -> Option<String> {
        self.rastro_vm.as_ref()?;
        if self.rastro_no_jit {
            return None;
        }
        let (_, inicio, fim) = rastro::marcadores();
        Some(format!("  call void @dartforge_registrar_rastro(ptr {inicio}, ptr {fim})\n"))
    }

    /// Os literais de string como objetos estáticos do módulo (§2.11 da
    /// especificação do espaço unificado): o AOT e os módulos do SDK.
    /// Veja [`LlvmEmitter::campos_por_chamada`].
    pub fn com_campos_por_chamada(mut self, sim: bool) -> Self {
        self.campos_por_chamada = sim;
        self
    }

    /// Veja [`LlvmEmitter::producao`].
    pub fn com_producao(mut self, sim: bool) -> Self {
        self.producao = sim;
        self
    }

    /// `optsize` em cada função do módulo (veja [`LlvmEmitter::otimizar_tamanho`]).
    pub fn com_otimizar_tamanho(mut self, sim: bool) -> Self {
        self.otimizar_tamanho = sim;
        self
    }

    /// Os ajudantes fora de linha do módulo do programa (veja
    /// [`ajudantes_fora_de_linha`]); um módulo do SDK os ignora.
    pub fn com_ajudantes_fora(mut self, nomes: Vec<String>) -> Self {
        if self.module.biblioteca_sdk {
            return self;
        }
        self.alocacao_fora_de_linha = nomes.iter().any(|n| n == "nova_instancia");
        self.seletor_compacto = nomes.iter().any(|n| n == "seletor");
        // O `@df.seletor` continua `alwaysinline`: só o `@df.seletor_d`
        // (fora de linha) o chama.
        self.ajudantes_fora = nomes.into_iter().filter(|n| n != "seletor").collect();
        self
    }

    pub fn com_objetos_estaticos(mut self, sim: bool) -> Self {
        self.objetos_estaticos = sim;
        self
    }

    /// As tabelas de métodos do módulo ficam para a ligação
    /// (docs/NATIVO-PODA-DE-TABELAS.md §3.2): nem o `$d` nem a função que o
    /// devolve são definidos aqui, só declarados por quem os cita; o
    /// conteúdo vai para o resumo da biblioteca (`poda::resumir`). Só o SDK
    /// de produção.
    /// Veja [`LlvmEmitter::mapas_no_jit`].
    pub fn com_mapas_no_jit(mut self, sim: bool) -> Self {
        self.mapas_no_jit = sim;
        self
    }

    /// Veja [`LlvmEmitter::mapas`].
    pub fn com_raizes_por_mapas(mut self, sim: bool) -> Self {
        self.mapas = sim;
        self
    }

    pub fn com_tabelas_na_ligacao(mut self, sim: bool) -> Self {
        self.tabelas_na_ligacao = sim;
        self
    }

    /// O emissor de uma geração nova de um programa em execução (hot reload
    /// do JIT): `anterior` são os nomes dos slots da área da geração viva
    /// ([`crate::area_do_ir`]). Os globais que continuam mantêm o índice e
    /// os novos vêm depois, então o código das duas gerações lê e grava a
    /// mesma área — os quadros `async` suspensos e as closures que ainda
    /// executam código antigo veem os mesmos estáticos que o código novo.
    pub fn com_area_anterior(mut self, anterior: Option<Vec<i64>>) -> Self {
        self.area_anterior = anterior;
        self
    }

    /// O descritor da área só com a chave e o número de slots, `[chave, -n]`
    /// (o sinal marca a forma enxuta para `dartforge_area_de_globais`): os
    /// hashes dos nomes só servem à recarga (a migração da área e a
    /// extensão do layout), e o executável de produção nunca recarrega. O
    /// JIT e a DLL de desenvolvimento ficam com o descritor completo.
    pub fn com_area_enxuta(mut self, enxuta: bool) -> Self {
        self.area_enxuta = enxuta;
        self
    }

    /// O módulo inteiro. Um módulo com diagnósticos não chega aqui:
    /// `emitir_ir` devolve o erro antes (N1).
    pub fn emit_all(mut self) -> String {
        assert!(self.module.erros.is_empty(), "emit_all com diagnósticos: {:?}", self.module.erros);
        // Coleta literais de strings do módulo para declaração como constantes globais
        self.collect_string_constants();

        // 1. Cabeçalho de target
        self.emit_header();

        // 2. Declarações do runtime
        self.emit_runtime_decls();

        // 3. Constantes de strings globais
        self.emit_string_constants();

        // 4. Classes e vtables
        self.emit_vtables();

        // 4b. Globais do usuário (N6)
        self.emit_globais();

        // 4c. Tabela de código das closures e vetores constantes (P1)
        self.emit_closures();

        // 5. Funções compiladas
        let modulo = self.module;
        if modulo.excecoes_por_tabelas {
            assert!(
                modulo.tabelas.len() == modulo.functions.len(),
                "bug do compilador: as funções do módulo mudaram depois do passe das exceções por tabelas"
            );
        }
        for (k, func) in modulo.functions.iter().enumerate() {
            self.tab = if modulo.excecoes_por_tabelas { modulo.tabelas.get(k) } else { None };
            self.emit_function(func);
        }
        self.tab = None;
        for d in std::mem::take(&mut self.variadicas) {
            self.out.push_str(&d);
        }

        if self.module.biblioteca_sdk {
            // Uma biblioteca do SDK da fonte (P5c): sem entrada nem despacho
            // de `toString`, que são do programa; a função de registro das
            // classes dela, que a entrada do programa chama.
            let reg = self.module.registro.clone().unwrap_or_else(|| "df.registrar".to_string());
            self.emitir_registro(&reg);
            self.emitir_globais_de_seletores();
            self.emitir_descritor_da_area();
            self.out.push_str(OBTER_AREA);
            self.emitir_globais_de_texto();
            self.emitir_declaracoes_externas();
            self.fechar_mapas();
            if let Some(r) = self.rastro_vm.take() {
                r.finalizar(&mut self.out);
            }
            if let Some(d) = self.depuracao.take() {
                d.finalizar(&mut self.out);
            }
            return self.out;
        }

        // 6. Funções de despacho polimórfico
        if self.module.modo_sdk {
            self.emitir_to_string_por_seletor();
        } else {
            self.emit_dispatch_functions();
        }

        // 7. Entrada global @dartforge_entry
        self.emit_entry();

        self.emitir_globais_de_seletores();
        self.emitir_descritor_da_area();
        // A área deste isolado com o layout desta geração, criada (ou
        // estendida) agora: a publicação de uma recarga do JIT a chama no
        // ponto seguro, antes de o código novo executar.
        self.out.push_str(
            "define void @df.preparar_area() {\n  %a = call ptr @dartforge_area_de_globais(ptr @df.area)\n  ret void\n}\n",
        );
        // A área no prólogo de cada função: o id do módulo indexa a tabela
        // desta thread no `Contexto` (deslocamentos 16 e 24); uma entrada
        // ausente vai ao runtime, que a preenche (e dá o id na primeira vez).
        // O índice da área do programa tem nome próprio: o particionamento
        // (`particao.rs`) o promove a externo, e a LTO completa dos módulos
        // do SDK, ao dividir o módulo juntado em partições, promove o
        // `@df.area_id` interno de um deles com o mesmo nome — "duplicate
        // symbol: df.area_id" (docs/NATIVO-PRODUCAO-GRANDE.md §5).
        let obter_area = OBTER_AREA.replace("@df.area_id", "@df.area_id.programa");
        self.out.push_str(&fora_de_linha(&obter_area, &self.ajudantes_fora));
        self.emitir_globais_de_texto();
        self.emitir_declaracoes_externas();
        self.fechar_mapas();
        if let Some(r) = self.rastro_vm.take() {
            r.finalizar(&mut self.out);
        }
        if let Some(d) = self.depuracao.take() {
            d.finalizar(&mut self.out);
        }
        self.out
    }

    /// Raízes por mapas no Mach-O: o `__llvm_stackmaps` fica no formato do
    /// LLVM (o `ld64.lld` não separa os índices do ThinLTO, e o conversor
    /// não roda na LTO do ligador), e nada o referencia — o `-dead_strip` o
    /// descartaria. `.no_dead_strip` marca vivo o átomo local
    /// `__LLVM_StackMaps` que o LLVM emite em cada objeto com mapa, sem
    /// torná-lo global (o Perry faz o mesmo,
    /// `perry-codegen/src/module.rs`). Só num módulo com função `gc`: sem
    /// mapa, o símbolo não existe.
    fn fechar_mapas(&mut self) {
        if self.mapas && !self.mapas_no_jit && self.funcoes_gc > 0 && crate::alvo::sistema() == crate::alvo::Sistema::MacOs {
            self.out.push_str("module asm \".no_dead_strip __LLVM_StackMaps\"\n");
        }
    }

    fn collect_string_constants(&mut self) {
        for func in &self.module.functions {
            for block in &func.blocks {
                for (_, inst, _) in &block.instructions {
                    if let Instruction::Const(Constant::String(_) | Constant::StringWtf8(_)) = inst {
                        let bytes: &[u8] = match inst {
                            Instruction::Const(Constant::String(s)) => s.as_bytes(),
                            Instruction::Const(Constant::StringWtf8(s)) => s,
                            _ => unreachable!(),
                        };
                        Self::registrar_string(&mut self.string_constants, &mut self.indice_de_string, bytes);
                    }
                }
            }
        }
        for class in &self.module.classes {
            Self::registrar_string(&mut self.string_constants, &mut self.indice_de_string, class.name.as_bytes());
        }
        for (_, nome, _) in &self.module.campos_do_rastro {
            Self::registrar_string(&mut self.string_constants, &mut self.indice_de_string, nome.as_bytes());
        }
    }

    /// Acrescenta `bytes` à tabela de constantes, se ainda não estiver.
    fn registrar_string(tabela: &mut Vec<Vec<u8>>, indice: &mut std::collections::HashMap<Vec<u8>, usize>, bytes: &[u8]) {
        if !indice.contains_key(bytes) {
            indice.insert(bytes.to_vec(), tabela.len());
            tabela.push(bytes.to_vec());
        }
    }

    fn emit_header(&mut self) {
        if self.mapas && !self.mapas_no_jit {
            // `-ni:1`: o `addrspace(1)` é de ponteiros não integrais — o
            // otimizador não fabrica `ptrtoint`/`inttoptr` com as raízes.
            self.out.push_str(&crate::alvo::cabecalho_ir().replacen("-S128\"", "-S128-ni:1\"", 1));
        } else {
            self.out.push_str(crate::alvo::cabecalho_ir());
        }
        // Os ids das classes do programa, para a geração seguinte de uma
        // recarga do JIT (J03, `context::ids_do_ir`).
        for (id, lib, classe) in &self.module.ids_do_programa {
            writeln!(self.out, "; df.classe {id} {} {}", crate::context::escapar(lib), crate::context::escapar(classe)).unwrap();
        }
        // O layout dos objetos, para migrar as instâncias vivas (J03):
        // `nome:flags:tipo`, com `a` anulável e `l` late.
        for (id, base, campos) in &self.module.campos_do_programa {
            let lista: Vec<String> = campos
                .iter()
                .map(|c| {
                    let flags = format!("{}{}", if c.anulavel { "a" } else { "" }, if c.late { "l" } else { "" });
                    format!("{}:{flags}:{}", crate::context::escapar(&c.nome), crate::context::escapar(&c.tipo))
                })
                .collect();
            writeln!(self.out, "; df.campos {id} {base} {}", lista.join(",")).unwrap();
        }
    }

    fn emit_runtime_decls(&mut self) {
        self.out.push_str("; Declarações do runtime nativo Rust (tabela em llvm/externs.rs)\n");
        for e in externs::EXTERNS {
            self.out.push_str(e.decl);
            // Raízes por mapas: a extern que não coleta é folha — a chamada
            // a ela não vira ponto de coleta (nem registro no mapa).
            if self.mapas {
                let ef = externs::efeitos_de(e.nome());
                if (!ef.aloca && !ef.chama_dart) || crate::alvo::folha_sabotada(e.nome()) {
                    self.out.push_str(" \"gc-leaf-function\"");
                }
            }
            self.out.push('\n');
        }
        if self.mapas {
            self.out.push_str("declare void @llvm.fake.use(...)\n");
        }
        if self.rastro_vm.is_some() && !self.rastro_no_jit {
            // O começo e o fim da seção do rastro da imagem (§13.14).
            self.out.push_str(rastro::marcadores().0);
        }
        if self.mapas && !self.mapas_no_jit {
            // O registro do mapa da imagem (`dartforge_registrar_mapa*`): a
            // base da imagem PE (o runtime acha `.dfgcm` e `.llvm_st` pelos
            // cabeçalhos); os limites da seção `dfgcm` que o `ld.lld` define;
            // no Mach-O, os do `__llvm_stackmaps` do LLVM.
            self.out.push_str(match crate::alvo::sistema() {
                crate::alvo::Sistema::Windows => "declare void @dartforge_registrar_mapa(ptr)\n@__ImageBase = external constant i8\n",
                crate::alvo::Sistema::Linux => {
                    "declare void @dartforge_registrar_mapa_secao(ptr, ptr)\n@__start_dfgcm = external hidden global i8\n@__stop_dfgcm = external hidden global i8\n"
                }
                crate::alvo::Sistema::MacOs => {
                    "declare void @dartforge_registrar_mapa_llvm(ptr, ptr)\n\
                     @\"\\01section$start$__LLVM_STACKMAPS$__llvm_stackmaps\" = external hidden global i8\n\
                     @\"\\01section$end$__LLVM_STACKMAPS$__llvm_stackmaps\" = external hidden global i8\n"
                }
            });
        }
        // As chamadas nativas com struct por valor copiam os bytes numa
        // temporária da pilha (`llvm/abi_c.rs`).
        let compostas = self.module.functions.iter().any(|f| {
            f.blocks.iter().any(|b| b.instructions.iter().any(|(_, i, _)| matches!(i, Instruction::ChamadaNativaComposta { .. })))
        });
        self.out.push_str(simd::DECLARACOES);
        self.out.push_str("declare i8 @llvm.expect.i8(i8, i8)\n");
        self.out.push_str("declare i1 @llvm.expect.i1(i1, i1)\n");
        for (_, decl) in externs::GLOBAIS {
            self.out.push_str(decl);
            self.out.push('\n');
        }
        // Os ajudantes `@df.*` de cada pacote (§3.5).
        let inicio_dos_ajudantes = self.out.len();
        self.out.push_str(textos_ir::AJUDANTES);
        self.out.push_str(caixas_ir::AJUDANTES);
        self.out.push_str(&listas_ir::ajudantes(self.module.memoria_arc));
        self.out.push_str(tipados_ir::AJUDANTES);
        let classe_do_valor = classe_do_valor(&self.module.cids_do_runtime);
        self.out.push_str(&classe_do_valor);
        self.out.push_str(&ajudantes_do_espaco());
        self.out.push_str(&ajudante_do_corpo(self.campos_por_chamada, self.producao));
        if !self.ajudantes_fora.is_empty() {
            let ajudantes = self.out.split_off(inicio_dos_ajudantes);
            self.out.push_str(&fora_de_linha(&ajudantes, &self.ajudantes_fora));
        }
        if self.mapas {
            let ajudantes = self.out.split_off(inicio_dos_ajudantes);
            self.out.push_str(&ajudantes_folha(&ajudantes));
        }
        if self.alocacao_fora_de_linha {
            self.out.push_str(&nova_instancia());
        }
        if self.seletor_compacto {
            self.out.push_str(SELETOR_POR_DESCRITOR);
        }
        if compostas || !self.module.ffi_callbacks.is_empty() {
            self.out.push_str(
                "declare void @llvm.memcpy.p0.p0.i64(ptr, ptr, i64, i1)\n\
                 declare void @llvm.memset.p0.i64(ptr, i8, i64, i1)\n\
                 declare ptr @llvm.stacksave.p0()\n\
                 declare void @llvm.stackrestore.p0(ptr)\n",
            );
        }
        if self.module.excecoes_por_tabelas {
            self.out.push_str(if cfg!(windows) { EXCECOES_POR_TABELAS } else { EXCECOES_POR_TABELAS_ITANIUM });
        }
        if externs::conferir_efeitos() {
            // A moldura da conferência não coleta: com as raízes por mapas, é
            // folha (sem statepoint em volta dela).
            if self.mapas {
                self.out.push_str(
                    "declare void @dartforge_efeitos_antes(ptr, i64, i64) \"gc-leaf-function\"\ndeclare void @dartforge_efeitos_depois() \"gc-leaf-function\"\n",
                );
            } else {
                self.out.push_str("declare void @dartforge_efeitos_antes(ptr, i64, i64)\ndeclare void @dartforge_efeitos_depois()\n");
            }
        }
        self.out.push('\n');
    }

    fn emit_string_constants(&mut self) {
        if self.string_constants.is_empty() {
            return;
        }
        self.out.push_str("; Constantes de string WTF-8\n");
        for (idx, s) in self.string_constants.iter().enumerate() {
            let bytes = s.as_slice();
            let len = bytes.len();
            let mut escaped = String::new();
            for &b in bytes {
                if (b as char).is_ascii_alphanumeric() || b == b' ' || b == b'_' || b == b'.' {
                    escaped.push(b as char);
                } else {
                    write!(escaped, "\\{:02X}", b).unwrap();
                }
            }
            writeln!(
                self.out,
                "@.str.{idx} = private unnamed_addr constant [{len} x i8] c\"{escaped}\""
            ).unwrap();
        }
        self.out.push('\n');
    }

    fn string_const_index(&self, s: &[u8]) -> Option<usize> {
        self.indice_de_string.get(s).copied()
    }

    fn emit_vtables(&mut self) {
        // vtables globais se houver classes
        for class in &self.module.classes {
            writeln!(self.out, "; VTable da classe {} (id {})", class.name, class.id).unwrap();
        }
    }

    fn emit_function(&mut self, func: &Function) {
        self.funcao_atual.clone_from(&func.symbol);
        self.cache_na_funcao = 0;
        // Tabela de tipos da funcao: sem ela o emissor nao sabe se %v8 e um
        // i1 (resultado de icmp) ou um i64, e imprime "ret i64 %v8" para um
        // valor i1 — modulo inteiro recusado pelo Clang.
        self.tipos.clear();
        self.apontado.clear();
        self.allocas_no_quadro = Self::allocas_ref_que_escapam(func);
        for block in &func.blocks {
            for (vid, inst, _) in &block.instructions {
                if let Instruction::Alloca(t) = inst {
                    self.apontado.insert(*vid, *t);
                }
            }
        }
        self.prox_coercao = 0;
        for (vid, _, ty) in &func.params {
            self.tipos.insert(*vid, *ty);
        }
        for block in &func.blocks {
            for (vid, inst, ty) in &block.instructions {
                self.tipos.insert(*vid, Self::tipo_do_resultado(inst, *ty));
            }
        }

        // Uma entrada de phi nao pode ser convertida onde o phi esta: phi tem de
        // ser a primeira instrucao do bloco. A conversao pertence ao bloco de
        // ORIGEM daquela entrada, emitida logo antes do terminador dele. Aqui
        // so planejamos; a emissao acontece bloco a bloco, mais abaixo.
        self.conv_phi.clear();
        let blocos_existentes: std::collections::HashSet<u32> =
            func.blocks.iter().map(|b| b.id.0).collect();
        for block in &func.blocks {
            for (_, inst, _) in &block.instructions {
                let Instruction::Phi { incoming, ty } = inst else { continue };
                for (origem, op) in incoming {
                    let Operand::Val(v) = op else { continue };
                    if !blocos_existentes.contains(&origem.0) {
                        continue;
                    }
                    let de = self.tipos.get(v).copied().unwrap_or(Type::I64);
                    let igual = de.llvm_ir() == ty.llvm_ir() && (de == Type::F64) == (*ty == Type::F64);
                    if igual {
                        continue;
                    }
                    let ja = self.conv_phi.iter().any(|(b, _, _, vv, para)| {
                        *b == origem.0 && vv == v && para.llvm_ir() == ty.llvm_ir()
                    });
                    if ja {
                        continue;
                    }
                    let nome = format!("%p{}", self.prox_coercao);
                    self.prox_coercao += 1;
                    self.conv_phi.push((origem.0, nome, de, *v, *ty));
                }
            }
        }

        let ret_ty = func.return_ty.llvm_ir();
        let params: Vec<String> = func
            .params
            .iter()
            .map(|(vid, _, ty)| format!("{} %v{}", ty.llvm_ir(), vid.0))
            .collect();
        let params_str = params.join(", ");

        let (ligacao, comdat) = self.ligacao_de(&func.symbol);
        // Uma cópia por imagem (`linkonce_odr`): o rastro não a rotula.
        let compartilhada = !ligacao.is_empty();
        // §7.4, modo sombra: os valores `Ref` da função (os candidatos de
        // `raizes::analisar`) para o conferidor de dominância.
        if !self.mapas && verificar_sombra::ligada() {
            let mut refs: Vec<u32> = func.params.iter().filter(|(_, _, t)| *t == Type::Ref).map(|(v, _, _)| v.0).collect();
            for b in &func.blocks {
                for (v, inst, t) in &b.instructions {
                    if *t == Type::Ref && !matches!(inst, Instruction::Const(Constant::Null) | Instruction::Alloca(_)) {
                        refs.push(v.0);
                    }
                }
            }
            let lista: Vec<String> = refs.iter().map(|v| format!("v{v}")).collect();
            writeln!(self.out, "{} {}", verificar_sombra::MARCA_DE_REFS, lista.join(" ")).unwrap();
        }
        let inicio_da_funcao = self.out.len();
        let mut posicao_escrita: Option<(u32, u32)> = None;
        // A posição de uma instrução copiada pelo inlining (só o rastro).
        let mut embutida_escrita: Option<(u32, u32, u32)> = None;
        let atributos = if self.otimizar_tamanho { " optsize" } else { "" };
        // Exceções por tabelas: a decisão do passe para esta função. Só a
        // função com pouso nomeia a personalidade (é ela que lê a tabela dos
        // pousos); as outras o desenrolamento atravessa.
        let tab = self.tab;
        let tem_pouso = tab.is_some_and(|t| !t.pousos.is_empty());
        let personalidade = if tem_pouso { " personality ptr @dartforge_personalidade" } else { "" };

        // G1/G2 (docs/NATIVO-PLANO.md §6.5): um slot por `alloca` de tipo
        // `Ref`, e os valores SSA `Ref` vivos em algum ponto de coleta, com
        // slot compartilhado entre os que nunca estão vivos juntos
        // (`raizes.rs`).
        let blocos_que_convertem: std::collections::HashSet<u32> = self.conv_phi.iter().map(|(b, ..)| *b).collect();
        let mut analise = raizes::analisar(
            func,
            &self.tipos,
            &|inst| self.pode_coletar(inst),
            &|b| blocos_que_convertem.contains(&b.0),
        );
        // O orçamento do `rewrite-statepoints-for-gc` (§3.4): o passe cresce
        // de forma superlinear com `vivos × pontos de coleta`. Acima do teto
        // a função fica inteira na pilha-sombra, como quadro residual
        // (§3.7): sem `gc`, com os slots de sempre.
        let custo = analise.enraizados.len() as u64 * (analise.vivos_em.len() + analise.vivos_no_fim.len()) as u64;
        let no_orcamento = custo <= crate::alvo::orcamento_dos_mapas();
        if self.mapas && !no_orcamento && std::env::var_os("DARTFORGE_RELATORIO_MAPAS").is_some() {
            eprintln!("dartforge: {} fora do orçamento dos mapas ({custo}): pilha-sombra", func.symbol);
        }
        if self.mapas && no_orcamento {
            // Raízes por mapas: no quadro só ficam os `alloca` `Ref` (o
            // mapa de pilha descreve valores SSA, não memória); os valores
            // SSA enraizados vão para o mapa.
            analise.slots.retain(|v, _| self.apontado.contains_key(v));
            self.enraizados = analise.enraizados.iter().copied().collect();
        } else {
            self.enraizados.clear();
        }
        self.slots = std::mem::take(&mut analise.slots);
        self.raizes_da_funcao = analise;
        // A função com raízes no mapa é uma função `gc`: o
        // `rewrite-statepoints-for-gc` transforma as chamadas dela que podem
        // coletar em statepoints (`crates/llvm`, `gerar`).
        let tem_gc = !self.enraizados.is_empty();
        if tem_gc {
            self.funcoes_gc += 1;
        }
        // O build de conferência: tantos slots a mais quantos o maior
        // conjunto de vivos num ponto de coleta.
        self.conferencia = None;
        if tem_gc && crate::alvo::conferir_raizes() {
            let k = self
                .raizes_da_funcao
                .vivos_em
                .values()
                .chain(self.raizes_da_funcao.vivos_no_fim.values())
                .map(Vec::len)
                .max()
                .unwrap_or(0);
            if k > 0 {
                let primeiro = self.slots.values().max().map_or(0, |m| m + 1);
                self.conferencia = Some((primeiro, k));
            }
        }
        let gc = if tem_gc { " gc \"statepoint-example\"" } else { "" };
        writeln!(self.out, "define {ligacao}{ret_ty} @{}({}){atributos}{comdat}{gc}{personalidade} {{", func.symbol, params_str)
            .unwrap();
        self.tem_frame = !self.slots.is_empty() || self.conferencia.is_some();
        self.rotulos_de_saida = func
            .blocks
            .iter()
            .filter_map(|b| {
                b.instructions.iter().rev().find_map(|(vid, i, _)| {
                    let v = vid.0;
                    if tab.is_some_and(|t| t.invocacoes.contains_key(vid)) {
                        // O caminho normal de um `invoke` ([`Self::tornar_invoke`]).
                        Some((b.id.0, format!("inv{v}.fim")))
                    } else if Self::alocacao_em_linha(i).is_some() && !self.alocacao_fora_de_linha {
                        Some((b.id.0, format!("ao{v}.fim")))
                    } else if self.barreira_em_linha(i) {
                        Some((b.id.0, format!("wb{v}.fim")))
                    } else if !self.objetos_estaticos && matches!(i, Instruction::Const(Constant::String(_) | Constant::StringWtf8(_))) {
                        // Sem os objetos estáticos (o JIT), o literal vai ao
                        // cache do ponto de uso (`textos_ir`), que divide o
                        // bloco; com eles, é uma constante (§2.11).
                        Some((b.id.0, format!("ls{v}.fim")))
                    } else if matches!(i, Instruction::CallRuntime { name, .. } if name == "dartforge_exception_clear") {
                        Some((b.id.0, format!("xc{v}.fim")))
                    } else if Self::rti_avaliar_em_cache(i) {
                        Some((b.id.0, format!("ra{v}.fim")))
                    } else {
                        None
                    }
                })
            })
            .collect();
        self.tem_ctx = self.tem_frame
            || func.blocks.iter().any(|b| {
                b.instructions.iter().any(|(_, i, _)| {
                    matches!(i, Instruction::CallRuntime { name, .. } if name == "dartforge_exception_pending")
                        || Self::usa_contexto(i)
                })
            })
            // O pouso restaura o topo da pilha-sombra e a saída guardada lê
            // a pendência: os dois pelo contexto.
            || tem_pouso
            || tab.is_some_and(|t| t.confere_pilha || t.saidas.values().any(|s| *s == SaidaPorExcecao::Guarda));

        for block in &func.blocks {
            writeln!(self.out, "b{}:", block.id.0).unwrap();
            self.rotulo_atual = format!("b{}", block.id.0);
            // O pouso de um `invoke`: quem desenrolou até aqui deixou a
            // exceção pendente e os quadros de raízes dele (e dos quadros
            // atravessados) ainda encadeados — o topo da pilha-sombra volta
            // a ser o desta função, antes de qualquer outra instrução.
            if tab.is_some_and(|t| t.pousos.contains(&block.id)) {
                if tem_gc {
                    // Numa função `gc` o `invoke` vira statepoint, e o pouso
                    // dele é `token` (o que as relocações do caminho de
                    // exceção referenciam).
                    writeln!(self.out, "  %lpad{} = landingpad token cleanup", block.id.0).unwrap();
                } else {
                    writeln!(self.out, "  %lpad{} = landingpad {{ ptr, i32 }} catch ptr null", block.id.0).unwrap();
                }
                let topo = if self.tem_frame { "%gcq" } else { "%topo0" };
                if !crate::alvo::sabotagem("pouso_sem_topo") {
                    writeln!(self.out, "  store ptr {topo}, ptr %ctxtopo, align 8").unwrap();
                }
                // Raízes por mapas: o que o tratador ainda lê estava vivo
                // através do `invoke`.
                if tem_gc && let Some(vivos) = self.raizes_da_funcao.vivos_na_entrada.get(&block.id).cloned() {
                    self.manter_vivos(&vivos);
                }
            }
            if block.id.0 == 0 {
                self.emit_buffers_de_closure(func);
                // O quadro de raízes também nasce antes de qualquer chamada
                // (N17): o `df.obter_area` abaixo é `alwaysinline` e tem
                // desvios, e um `alloca` depois dele deixava de ser do bloco
                // de entrada — virava alocação dinâmica, com `__chkstk` e o
                // `stacksave` a cada chamada da função, e impedia o inliner
                // de levar o quadro para o bloco de entrada de quem chama.
                // `DARTFORGE_SEM_QUADRO_NA_ENTRADA=1` na compilação volta ao
                // lugar antigo (medida).
                if self.tem_frame && Self::quadro_na_entrada() {
                    let n = self.tamanho_do_quadro();
                    writeln!(self.out, "  %gcq = alloca {{ ptr, i64, [{n} x i64] }}, align 8").unwrap();
                }
                if Self::usa_area(func) {
                    writeln!(self.out, "  %area = call ptr @df.obter_area()").unwrap();
                }
            }
            if block.id.0 == 0
                && let Some(k) = self.rastro.as_mut()
            {
                let n = *k;
                *k += 1;
                self.nomes_do_rastro.push(func.symbol.clone());
                writeln!(self.out, "  call void @dartforge_rastro_entrada(ptr @df.rastro.{n}, i64 {})", func.symbol.len()).unwrap();
            }
            // O contexto da thread (`runtime/src/heap.rs`, `Contexto`): a
            // exceção pendente no deslocamento 0, o topo da pilha-sombra no 8.
            if block.id.0 == 0 && self.tem_ctx {
                writeln!(self.out, "  %ctx = call ptr @dartforge_contexto()").unwrap();
                writeln!(self.out, "  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8").unwrap();
                // Sem quadro de raízes, o topo que o pouso restaura é o da
                // entrada da função.
                if tem_pouso && !self.tem_frame {
                    writeln!(self.out, "  %topo0 = load ptr, ptr %ctxtopo, align 8").unwrap();
                }
                self.emitir_conferencia_da_pilha(func);
            }
            if block.id.0 == 0 && self.tem_frame {
                // O quadro de raízes no stack da função (a pilha-sombra,
                // `QuadroDeRaizes` do runtime): anterior, número de slots e
                // os slots, zerados antes de o runtime encadeá-lo. Cada raiz
                // é um `store` no slot dela (slots compartilhados: o tamanho
                // é o do maior).
                let n = self.tamanho_do_quadro();
                let t = format!("{{ ptr, i64, [{n} x i64] }}");
                if !Self::quadro_na_entrada() {
                    writeln!(self.out, "  %gcq = alloca {t}, align 8").unwrap();
                }
                let cabecalho = self.cabecalho_do_quadro(n);
                // A sabotagem `quadro_sujo` (D5, §7.3): os slots nascem com um
                // valor de handle que não aponta para bloco nenhum; um slot
                // lido antes de escrito cai na validação de handle.
                let slots = if n > 0 && crate::alvo::sabotagem("quadro_sujo") {
                    format!("[{}]", vec!["i64 4098"; n].join(", "))
                } else {
                    "zeroinitializer".to_string()
                };
                writeln!(self.out, "  store {t} {{ ptr null, i64 {cabecalho}, [{n} x i64] {slots} }}, ptr %gcq").unwrap();
                for slot in 0..n {
                    writeln!(self.out, "  %gcs{slot} = getelementptr inbounds {t}, ptr %gcq, i64 0, i32 2, i64 {slot}").unwrap();
                }
                // Encadeia o quadro: `anterior` = topo; topo = quadro.
                writeln!(self.out, "  %gcant = load ptr, ptr %ctxtopo, align 8").unwrap();
                writeln!(self.out, "  store ptr %gcant, ptr %gcq, align 8").unwrap();
                writeln!(self.out, "  store ptr %gcq, ptr %ctxtopo, align 8").unwrap();
                for (vid, _, _) in &func.params {
                    if let Some(&slot) = self.slots.get(vid) {
                        writeln!(self.out, "  store i64 %v{}, ptr %gcs{slot}", vid.0).unwrap();
                    }
                }
                let mut no_quadro: Vec<ValueId> = self.allocas_no_quadro.iter().copied().collect();
                no_quadro.sort_by_key(|v| v.0);
                for vid in no_quadro {
                    let slot = self.slots[&vid];
                    writeln!(self.out, "  %v{} = getelementptr inbounds i8, ptr %gcs{slot}, i64 0", vid.0).unwrap();
                }
            }
            if block.id.0 == 0 {
                for (vid, _, _) in &func.params {
                    if self.enraizados.contains(vid) {
                        self.raiz_no_mapa(vid.0);
                    }
                }
                // A sabotagem `bruto_no_mapa`: um valor que o otimizador não
                // dobra (a carga volátil) posto no mapa como raiz.
                if tem_gc && crate::alvo::sabotagem("bruto_no_mapa") {
                    self.out.push_str(
                        "  %dfsab = alloca i64, align 8\n  store volatile i64 4098, ptr %dfsab, align 8\n  %dfsabv = load volatile i64, ptr %dfsab, align 8\n  %raizsab = inttoptr i64 %dfsabv to ptr addrspace(1)\n",
                    );
                }
            }
            // `phi` tem de ser a primeira instrução do bloco: as raízes dos
            // `phi` saem todas depois do último deles.
            let mut raizes_de_phi: Vec<(usize, u32)> = Vec::new();

            for (vid, inst, ty) in &block.instructions {
                let v = vid.0;
                // J05: a posição do comando, quando muda (`llvm/depuracao.rs`);
                // a de uma instrução copiada pelo inlining da HIR, com o
                // contexto dela, vai num marcador só do rastro (§13.14).
                if let Some(e) = func.depuracao.as_ref().and_then(|d| d.posicoes_embutidas.get(vid)) {
                    if embutida_escrita != Some(*e) {
                        writeln!(self.out, "{}{} {} {}", depuracao::MARCADOR_EMBUTIDO, e.0, e.1, e.2).unwrap();
                        embutida_escrita = Some(*e);
                        posicao_escrita = None;
                    }
                } else if let Some(p) = func.depuracao.as_ref().and_then(|d| d.posicoes.get(vid))
                    && posicao_escrita != Some(*p)
                {
                    writeln!(self.out, "{}{} {}", depuracao::MARCADOR, p.0, p.1).unwrap();
                    posicao_escrita = Some(*p);
                    embutida_escrita = None;
                }
                if !matches!(inst, Instruction::Phi { .. }) && !raizes_de_phi.is_empty() {
                    for (slot, pv) in std::mem::take(&mut raizes_de_phi) {
                        self.raiz_de_phi(slot, pv);
                    }
                }
                // O build de conferência: os vivos deste ponto de coleta,
                // exatamente, nos slots de conferência.
                if self.conferencia.is_some()
                    && let Some(vivos) = self.raizes_da_funcao.vivos_em.get(vid).cloned()
                {
                    self.gravar_conferencia(&vivos);
                }
                let inicio_da_instrucao = self.out.len();
                match inst {
                    Instruction::Const(Constant::Int(n)) => {
                        writeln!(self.out, "  %v{v} = add i64 0, {n}").unwrap();
                    }
                    Instruction::Const(Constant::Double(d)) => {
                        let bits = d.to_bits();
                        writeln!(self.out, "  %v{v} = bitcast i64 {bits} to double").unwrap();
                    }
                    Instruction::Const(Constant::Bool(b)) => {
                        let bit = if *b { 1 } else { 0 };
                        writeln!(self.out, "  %v{v} = add i1 0, {bit}").unwrap();
                    }
                    Instruction::Const(Constant::Null) => {
                        writeln!(self.out, "  %v{v} = add i64 0, 0").unwrap();
                    }
                    Instruction::Const(Constant::String(s)) => self.emitir_const_string(v, s.as_bytes()),
                    Instruction::Const(Constant::StringWtf8(s)) => self.emitir_const_string(v, s),
                    Instruction::Add(a, b) => {
                        let sa = self.coagir(a, Type::I64);
                        let sb = self.coagir(b, Type::I64);
                        writeln!(self.out, "  %v{v} = add i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::Sub(a, b) => {
                        let sa = self.coagir(a, Type::I64);
                        let sb = self.coagir(b, Type::I64);
                        writeln!(self.out, "  %v{v} = sub i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::Mul(a, b) => {
                        let sa = self.coagir(a, Type::I64);
                        let sb = self.coagir(b, Type::I64);
                        writeln!(self.out, "  %v{v} = mul i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::SDiv(a, b) => {
                        let sa = self.coagir(a, Type::I64);
                        let sb = self.coagir(b, Type::I64);
                        writeln!(self.out, "  %v{v} = sdiv i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::SRem(a, b) => {
                        let sa = self.coagir(a, Type::I64);
                        let sb = self.coagir(b, Type::I64);
                        writeln!(self.out, "  %v{v} = srem i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::Shl(a, b) => {
                        let sa = self.coagir(a, Type::I64);
                        let sb = self.coagir(b, Type::I64);
                        writeln!(self.out, "  %v{v} = shl i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::AShr(a, b) => {
                        let sa = self.coagir(a, Type::I64);
                        let sb = self.coagir(b, Type::I64);
                        writeln!(self.out, "  %v{v} = ashr i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::And(a, b) => {
                        let sa = self.coagir(a, Type::I64);
                        let sb = self.coagir(b, Type::I64);
                        writeln!(self.out, "  %v{v} = and i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::Or(a, b) => {
                        let sa = self.coagir(a, Type::I64);
                        let sb = self.coagir(b, Type::I64);
                        writeln!(self.out, "  %v{v} = or i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::Xor(a, b) => {
                        let sa = self.coagir(a, Type::I64);
                        let sb = self.coagir(b, Type::I64);
                        writeln!(self.out, "  %v{v} = xor i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::Neg(a) => {
                        let sa = self.coagir(a, Type::I64);
                        writeln!(self.out, "  %v{v} = sub i64 0, {sa}").unwrap();
                    }
                    Instruction::Not(a) => {
                        let sa = self.coagir(a, Type::I64);
                        writeln!(self.out, "  %v{v} = xor i64 {sa}, -1").unwrap();
                    }
                    Instruction::FAdd(a, b) => {
                        let sa = self.coagir(a, Type::F64);
                        let sb = self.coagir(b, Type::F64);
                        writeln!(self.out, "  %v{v} = fadd double {sa}, {sb}").unwrap();
                    }
                    Instruction::FSub(a, b) => {
                        let sa = self.coagir(a, Type::F64);
                        let sb = self.coagir(b, Type::F64);
                        writeln!(self.out, "  %v{v} = fsub double {sa}, {sb}").unwrap();
                    }
                    Instruction::FMul(a, b) => {
                        let sa = self.coagir(a, Type::F64);
                        let sb = self.coagir(b, Type::F64);
                        writeln!(self.out, "  %v{v} = fmul double {sa}, {sb}").unwrap();
                    }
                    Instruction::FDiv(a, b) => {
                        let sa = self.coagir(a, Type::F64);
                        let sb = self.coagir(b, Type::F64);
                        writeln!(self.out, "  %v{v} = fdiv double {sa}, {sb}").unwrap();
                    }
                    Instruction::FNeg(a) => {
                        let sa = self.coagir(a, Type::F64);
                        writeln!(self.out, "  %v{v} = fneg double {sa}").unwrap();
                    }
                    Instruction::ICmp(op, a, b) => {
                        let op_str = match op {
                            ICmpOp::Eq => "eq",
                            ICmpOp::Ne => "ne",
                            ICmpOp::Slt => "slt",
                            ICmpOp::Sle => "sle",
                            ICmpOp::Sgt => "sgt",
                            ICmpOp::Sge => "sge",
                            ICmpOp::Ult => "ult",
                        };
                        let sa = self.coagir(a, Type::I64);
                        let sb = self.coagir(b, Type::I64);
                        writeln!(self.out, "  %v{v} = icmp {op_str} i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::FCmp(op, a, b) => {
                        let op_str = match op {
                            FCmpOp::Eq => "oeq",
                            FCmpOp::Ne => "one",
                            FCmpOp::Lt => "olt",
                            FCmpOp::Le => "ole",
                            FCmpOp::Gt => "ogt",
                            FCmpOp::Ge => "oge",
                        };
                        let sa = self.coagir(a, Type::F64);
                        let sb = self.coagir(b, Type::F64);
                        writeln!(self.out, "  %v{v} = fcmp {op_str} double {sa}, {sb}").unwrap();
                    }
                    Instruction::LNot(a) => {
                        let sa = self.coagir(a, Type::I1);
                        writeln!(self.out, "  %v{v} = xor i1 {sa}, true").unwrap();
                    }
                    Instruction::IntToDouble(a) => {
                        let sa = self.coagir(a, Type::I64);
                        writeln!(self.out, "  %v{v} = sitofp i64 {sa} to double").unwrap();
                    }
                    Instruction::DoubleToInt(a) => {
                        let sa = self.coagir(a, Type::F64);
                        writeln!(self.out, "  %v{v} = fptosi double {sa} to i64").unwrap();
                    }
                    Instruction::ZExt { op, from, to } => {
                        let sop = self.coagir(op, *from);
                        let f = from.llvm_ir();
                        let t = to.llvm_ir();
                        writeln!(self.out, "  %v{v} = zext {f} {sop} to {t}").unwrap();
                    }
                    Instruction::Trunc { op, from, to } => {
                        let sop = self.coagir(op, *from);
                        let f = from.llvm_ir();
                        let t = to.llvm_ir();
                        writeln!(self.out, "  %v{v} = trunc {f} {sop} to {t}").unwrap();
                    }
                    Instruction::AllocObject { class_id, fields } => {
                        let count = fields.len();
                        writeln!(
                            self.out,
                            "  %v{v} = call i64 @dartforge_object_new(i64 {class_id}, i64 {count})"
                        ).unwrap();
                        // Os campos em linha, pela tabela de campos (o objeto
                        // é novo: nenhum outro código o vê antes disto).
                        if !fields.is_empty() {
                            self.emitir_endereco_dos_campos(v, &format!("%v{v}"));
                        }
                        for (idx, field) in fields.iter().enumerate() {
                            // E1: `is_ref` pela representação do valor.
                            let is_ref = u8::from(self.tipo_de(field) == Type::Ref).to_string();
                            let sf = self.coagir(field, Type::I64);
                            self.emitir_gravacao_de_campo(v, &format!(".{idx}"), idx, &sf, &is_ref);
                        }
                    }
                    // O campo em linha: a palavra de 8 bytes depois do
                    // cabeçalho do corpo do objeto (`heap::Cabecalho`).
                    Instruction::GetField { object, index } if (*index as usize) < CAMPOS_EM_LINHA => {
                        let so = self.coagir(object, Type::I64);
                        self.emitir_endereco_dos_campos(v, &so);
                        writeln!(self.out, "  %fg{v} = getelementptr inbounds i64, ptr %fp{v}, i64 {index}").unwrap();
                        writeln!(self.out, "  %v{v} = load i64, ptr %fg{v}, align 8").unwrap();
                    }
                    Instruction::GetField { object, index } => {
                        let so = self.coagir(object, Type::I64);
                        writeln!(
                            self.out,
                            "  %v{v} = call i64 @dartforge_object_get(i64 {so}, i64 {index})"
                        ).unwrap();
                    }
                    // No ARC a gravação vai ao runtime (`dartforge_object_set`), que conta a troca.
                    Instruction::SetField { object, index, value } if (*index as usize) < CAMPOS_EM_LINHA && !self.module.memoria_arc => {
                        let is_ref = u8::from(self.tipo_de(value) == Type::Ref);
                        let so = self.coagir(object, Type::I64);
                        let sv = self.coagir(value, Type::I64);
                        self.emitir_endereco_dos_campos(v, &so);
                        self.emitir_gravacao_de_campo(v, "", *index, &sv, &is_ref.to_string());
                        if self.barreira_em_linha(inst) {
                            self.emitir_barreira(v, &so, &sv, None);
                        }
                    }
                    Instruction::SetField { object, index, value } => {
                        let is_ref = u8::from(self.tipo_de(value) == Type::Ref);
                        let so = self.coagir(object, Type::I64);
                        let sv = self.coagir(value, Type::I64);
                        writeln!(
                            self.out,
                            "  call void @dartforge_object_set(i64 {so}, i64 {index}, i64 {sv}, i8 {is_ref})"
                        ).unwrap();
                    }
                    Instruction::CallStatic { symbol, args, ret_ty } => {
                        let target_func = self.funcao_por_simbolo.get(symbol.as_str()).copied();
                        let mut args_str: Vec<String> = Vec::with_capacity(args.len());
                        let mut tipos_args = Vec::with_capacity(args.len());
                        for (idx, a) in args.iter().enumerate() {
                            // Função de outro módulo (SDK da fonte): o tipo do
                            // operando, que o lowering já coagiu para a
                            // representação do parâmetro.
                            let alvo = target_func
                                .and_then(|f| f.params.get(idx))
                                .map_or_else(|| self.tipo_de(a), |p| p.2);
                            let alvo = if alvo == Type::Void { Type::I64 } else { alvo };
                            let s = self.coagir(a, alvo);
                            args_str.push(format!("{} {s}", alvo.llvm_ir()));
                            tipos_args.push(alvo);
                        }
                        if target_func.is_none() {
                            self.anotar_externo(symbol, *ret_ty, &tipos_args);
                        }
                        let joined = args_str.join(", ");
                        let r = ret_ty.llvm_ir();
                        if *ret_ty == Type::Void {
                            writeln!(self.out, "  call {r} @{symbol}({joined})").unwrap();
                        } else {
                            writeln!(self.out, "  %v{v} = call {r} @{symbol}({joined})").unwrap();
                        }
                    }
                    // Campo com índice constante: em linha, como `GetField`.
                    Instruction::CallRuntime { name, args, .. }
                        if name == "dartforge_object_get"
                            && matches!(args.get(1), Some((Operand::Constant(Constant::Int(i)), _)) if (0..CAMPOS_EM_LINHA as i64).contains(i)) =>
                    {
                        let Some((Operand::Constant(Constant::Int(i)), _)) = args.get(1) else { unreachable!() };
                        let so = self.coagir(&args[0].0, Type::I64);
                        self.emitir_endereco_dos_campos(v, &so);
                        writeln!(self.out, "  %fg{v} = getelementptr inbounds i64, ptr %fp{v}, i64 {i}").unwrap();
                        writeln!(self.out, "  %v{v} = load i64, ptr %fg{v}, align 8").unwrap();
                    }
                    Instruction::CallRuntime { name, args, .. }
                        if name == "dartforge_object_set"
                            && !self.module.memoria_arc
                            && args.len() == 4
                            && matches!(args.get(1), Some((Operand::Constant(Constant::Int(i)), _)) if (0..CAMPOS_EM_LINHA as i64).contains(i)) =>
                    {
                        let Some((Operand::Constant(Constant::Int(i)), _)) = args.get(1) else { unreachable!() };
                        let so = self.coagir(&args[0].0, Type::I64);
                        let sv = self.coagir(&args[2].0, Type::I64);
                        // `is_ref` é um `bool` do Rust: só 0 ou 1.
                        let is_ref = match &args[3].0 {
                            Operand::Constant(Constant::Int(n)) => u8::from(*n != 0).to_string(),
                            Operand::Constant(Constant::Bool(b)) => u8::from(*b).to_string(),
                            outro => {
                                let x = self.coagir(outro, Type::I64);
                                writeln!(self.out, "  %fb{v} = icmp ne i64 {x}, 0").unwrap();
                                writeln!(self.out, "  %fz{v} = zext i1 %fb{v} to i8").unwrap();
                                format!("%fz{v}")
                            }
                        };
                        self.emitir_endereco_dos_campos(v, &so);
                        self.emitir_gravacao_de_campo(v, "", *i as usize, &sv, &is_ref);
                        if self.barreira_em_linha(inst) {
                            let dinamico = is_ref.starts_with('%').then_some(is_ref.as_str());
                            self.emitir_barreira(v, &so, &sv, dinamico);
                        }
                    }
                    Instruction::CallRuntime { .. } if Self::alocacao_em_linha(inst).is_some() => {
                        let (c, n) = Self::alocacao_em_linha(inst).expect("conferido na guarda");
                        self.emitir_alocacao_em_linha(v, c, n);
                    }
                    Instruction::CallRuntime { name, args, ret_ty }
                        if name == "dartforge_object_new"
                            && matches!(args.first(), Some((Operand::Constant(Constant::Int(c)), _))
                                if self.module.funcoes_de_tabela.contains_key(&(*c as u32))) =>
                    {
                        // SDK da fonte: a primeira alocação registra a tabela
                        // de métodos da classe (`seletores.rs`).
                        let Some((Operand::Constant(Constant::Int(c)), _)) = args.first() else { unreachable!() };
                        let f = self.module.funcoes_de_tabela[&(*c as u32)].clone();
                        self.anotar_externo(&f, Type::Ptr, &[]);
                        let n = self.coagir(&args[1].0, Type::I64);
                        writeln!(self.out, "  %v{v} = call i64 @dartforge_object_new_t(i64 {c}, i64 {n}, ptr @{f})").unwrap();
                        let _ = ret_ty;
                    }
                    Instruction::CallRuntime { name, args, ret_ty }
                        if (name == "dartforge_typed_novo" || name == "dartforge_view_nova" || name == "dartforge_typed_externo")
                            && matches!(args.first(), Some((Operand::Constant(Constant::Int(c)), _))
                                if self.module.funcoes_de_tabela.contains_key(&(*c as u32))) =>
                    {
                        // A primeira lista tipada (ou visão) de uma classe
                        // registra a tabela de métodos dela, como
                        // `dartforge_object_new_t`.
                        let Some((Operand::Constant(Constant::Int(c)), _)) = args.first() else { unreachable!() };
                        let f = self.module.funcoes_de_tabela[&(*c as u32)].clone();
                        self.anotar_externo(&f, Type::Ptr, &[]);
                        let resto: Vec<String> = args.iter().map(|(a, t)| format!("{} {}", t.llvm_ir(), self.coagir(a, *t))).collect();
                        writeln!(self.out, "  %v{v} = call i64 @{name}_t({}, ptr @{f})", resto.join(", ")).unwrap();
                        let _ = ret_ty;
                    }
                    // A exceção pendente: o espelho no contexto da thread.
                    Instruction::CallRuntime { name, .. } if name == "dartforge_exception_pending" && self.tem_ctx => {
                        writeln!(self.out, "  %v{v} = load i8, ptr %ctx, align 8").unwrap();
                    }
                    // O `dartforge_exception_clear` de cada `return` e salto:
                    // sem exceção pendente (o espelho no contexto), não há o
                    // que limpar — a entrada do `catch` e o `finally` já
                    // limparam o rastro junto com a exceção. Só a pendente
                    // (o `finally` que sai por `return`) chama o runtime.
                    Instruction::CallRuntime { name, .. } if name == "dartforge_exception_clear" && self.tem_ctx => {
                        writeln!(self.out, "  %xcp{v} = load i8, ptr %ctx, align 8").unwrap();
                        writeln!(self.out, "  %xcn{v} = icmp ne i8 %xcp{v}, 0").unwrap();
                        writeln!(self.out, "  %xce{v} = call i1 @llvm.expect.i1(i1 %xcn{v}, i1 false)").unwrap();
                        writeln!(self.out, "  br i1 %xce{v}, label %xc{v}.limpar, label %xc{v}.fim").unwrap();
                        writeln!(self.out, "xc{v}.limpar:").unwrap();
                        writeln!(self.out, "  call void @dartforge_exception_clear()").unwrap();
                        if tab.is_some() {
                            // A limpeza que não limpou é o desenrolar do
                            // isolado (`Isolate.exit`, `kill`), que nenhum
                            // tratador segura: no modelo de conferência a
                            // função voltava com ele pendente; aqui ela
                            // desenrola (o `catch` e o `finally` de fora, na
                            // mesma função, não o tratariam mesmo).
                            writeln!(self.out, "  %xcd{v} = load i8, ptr %ctx, align 8").unwrap();
                            writeln!(self.out, "  %xcq{v} = icmp ne i8 %xcd{v}, 0").unwrap();
                            writeln!(self.out, "  br i1 %xcq{v}, label %xc{v}.sai, label %xc{v}.fim").unwrap();
                            writeln!(self.out, "xc{v}.sai:").unwrap();
                            writeln!(self.out, "  call void @df.lancar()").unwrap();
                            writeln!(self.out, "  unreachable").unwrap();
                        } else {
                            writeln!(self.out, "  br label %xc{v}.fim").unwrap();
                        }
                        writeln!(self.out, "xc{v}.fim:").unwrap();
                        self.rotulo_atual = format!("xc{v}.fim");
                    }
                    // O pedido de interrupção (J01): o byte no contexto
                    // (deslocamento 32), lido atômico — outra thread o liga, e
                    // a carga não pode sair do laço.
                    Instruction::CallRuntime { name, .. } if name == "dartforge_interrupcao_pendente" && self.tem_ctx => {
                        writeln!(self.out, "  %ip{v} = getelementptr inbounds i8, ptr %ctx, i64 32").unwrap();
                        writeln!(self.out, "  %iv{v} = load atomic i8, ptr %ip{v} monotonic, align 8").unwrap();
                        // O pedido é raro: o caminho lento fica fora do corpo
                        // do laço.
                        writeln!(self.out, "  %v{v} = call i8 @llvm.expect.i8(i8 %iv{v}, i8 0)").unwrap();
                    }
                    Instruction::CallRuntime { args, .. } if Self::rti_avaliar_em_cache(inst) => {
                        self.emitir_rti_avaliar_em_cache(v, args);
                    }
                    // A classe do receptor: a do objeto do espaço lida no
                    // cabeçalho, em linha (`df.classe`); o resto, o runtime.
                    Instruction::CallRuntime { name, args, .. } if name == "dartforge_value_class" && args.len() == 1 => {
                        let h = self.coagir(&args[0].0, Type::I64);
                        writeln!(self.out, "  %v{v} = call i64 @df.classe(i64 {h})").unwrap();
                    }
                    // `cid <: C` pelo mapa de bits do alvo (`df.subclasse`),
                    // sem chamada depois da primeira consulta.
                    Instruction::CallRuntime { name, args, .. } if name == "dartforge_is_subclass" && args.len() == 2 => {
                        let a = self.coagir(&args[0].0, Type::I64);
                        let b = self.coagir(&args[1].0, Type::I64);
                        writeln!(self.out, "  %v{v} = call i8 @df.subclasse(i64 {a}, i64 {b})").unwrap();
                    }
                    Instruction::CallRuntime { name, args, ret_ty } => {
                        if name.starts_with("dartforge_nativo_") {
                            let tipos: Vec<Type> = args.iter().map(|(_, t)| *t).collect();
                            self.anotar_externo(name, *ret_ty, &tipos);
                        }
                        let mut args_formatted = Vec::new();
                        for (a, ty) in args {
                            let s = self.coagir(a, *ty);
                            let t = ty.llvm_ir();
                            args_formatted.push(format!("{t} {s}"));
                        }
                        let joined = args_formatted.join(", ");
                        let r = ret_ty.llvm_ir();
                        // `DARTFORGE_EFEITOS=conferir`: a extern marcada sem
                        // coletar ou sem lançar roda vigiada pelo runtime.
                        let marcas = if externs::conferir_efeitos() { externs::marcas_a_conferir(name) } else { 0 };
                        if marcas != 0 {
                            let k = match self.externs_conferidas.iter().position(|n| n == name) {
                                Some(k) => k,
                                None => {
                                    self.externs_conferidas.push(name.clone());
                                    self.externs_conferidas.len() - 1
                                }
                            };
                            writeln!(self.out, "  call void @dartforge_efeitos_antes(ptr @df.efn.{k}, i64 {}, i64 {marcas})", name.len()).unwrap();
                        }
                        if *ret_ty == Type::Void {
                            writeln!(self.out, "  call {r} @{name}({joined})").unwrap();
                        } else {
                            writeln!(self.out, "  %v{v} = call {r} @{name}({joined})").unwrap();
                        }
                        if marcas != 0 {
                            writeln!(self.out, "  call void @dartforge_efeitos_depois()").unwrap();
                        }
                    }
                    Instruction::ChamadaNativa { alvo, args, ret } => {
                        // A chamada C: cada argumento convertido ao tipo C
                        // (estreitos com a extensão da ABI), o retorno
                        // estendido de volta explicitamente (o Windows x64
                        // não garante os bits altos de um retorno estreito).
                        let a = self.coagir(alvo, Type::I64);
                        writeln!(self.out, "  %fn{v} = inttoptr i64 {a} to ptr").unwrap();
                        let mut partes = Vec::with_capacity(args.len());
                        for (i, (op, tc)) in args.iter().enumerate() {
                            partes.push(self.argumento_c(v, i, op, *tc));
                        }
                        let lista = partes.join(", ");
                        let conv = match ret {
                            TipoC::I8 | TipoC::I16 | TipoC::I32 => Some(format!("sext {} %nr{v} to i64", ret.llvm())),
                            TipoC::U8 | TipoC::U16 | TipoC::U32 => Some(format!("zext {} %nr{v} to i64", ret.llvm())),
                            TipoC::F32 => Some(format!("fpext float %nr{v} to double")),
                            TipoC::Ptr | TipoC::Handle => Some(format!("ptrtoint ptr %nr{v} to i64")),
                            TipoC::I64 | TipoC::U64 | TipoC::F64 | TipoC::Bool | TipoC::Void => None,
                        };
                        match (ret, conv) {
                            (TipoC::Void, _) => writeln!(self.out, "  call void %fn{v}({lista})").unwrap(),
                            (_, None) => writeln!(self.out, "  %v{v} = call {} %fn{v}({lista})", ret.llvm()).unwrap(),
                            (_, Some(c)) => {
                                writeln!(self.out, "  %nr{v} = call {} %fn{v}({lista})", ret.llvm()).unwrap();
                                writeln!(self.out, "  %v{v} = {c}").unwrap();
                            }
                        }
                    }
                    Instruction::CargaNativa { endereco, indice, tipo } => {
                        // Sem alinhamento suposto (os bytes de uma lista
                        // tipada são de um `Vec<u8>`).
                        let e = self.coagir(endereco, Type::I64);
                        let i = self.coagir(indice, Type::I64);
                        let t = tipo.llvm();
                        let inv = "";
                        writeln!(self.out, "  %cp{v} = inttoptr i64 {e} to ptr").unwrap();
                        writeln!(self.out, "  %cg{v} = getelementptr {t}, ptr %cp{v}, i64 {i}").unwrap();
                        let conv = match tipo {
                            TipoC::I8 | TipoC::I16 | TipoC::I32 => Some(format!("sext {t} %cl{v} to i64")),
                            TipoC::U8 | TipoC::U16 | TipoC::U32 => Some(format!("zext {t} %cl{v} to i64")),
                            TipoC::F32 => Some(format!("fpext float %cl{v} to double")),
                            _ => None,
                        };
                        match conv {
                            Some(c) => {
                                writeln!(self.out, "  %cl{v} = load {t}, ptr %cg{v}, align 1").unwrap();
                                writeln!(self.out, "  %v{v} = {c}").unwrap();
                            }
                            None => writeln!(self.out, "  %v{v} = load {t}, ptr %cg{v}, align 1{inv}").unwrap(),
                        }
                    }
                    Instruction::GravacaoNativa { endereco, indice, tipo, valor } => {
                        let e = self.coagir(endereco, Type::I64);
                        let i = self.coagir(indice, Type::I64);
                        let t = tipo.llvm();
                        let x = match tipo {
                            TipoC::F32 => {
                                let d = self.coagir(valor, Type::F64);
                                writeln!(self.out, "  %gx{v} = fptrunc double {d} to float").unwrap();
                                format!("%gx{v}")
                            }
                            TipoC::F64 => self.coagir(valor, Type::F64),
                            TipoC::I64 | TipoC::U64 => self.coagir(valor, Type::I64),
                            _ => {
                                let n = self.coagir(valor, Type::I64);
                                writeln!(self.out, "  %gx{v} = trunc i64 {n} to {t}").unwrap();
                                format!("%gx{v}")
                            }
                        };
                        writeln!(self.out, "  %gp{v} = inttoptr i64 {e} to ptr").unwrap();
                        writeln!(self.out, "  %gg{v} = getelementptr {t}, ptr %gp{v}, i64 {i}").unwrap();
                        writeln!(self.out, "  store {t} {x}, ptr %gg{v}, align 1").unwrap();
                    }
                    Instruction::ChamadaNativaComposta { alvo, args, ret, destino, variadica } => {
                        self.chamada_nativa_composta(v, alvo, args, ret, destino.as_ref(), *variadica);
                    }
                    Instruction::AllocList { elements } => self.emitir_alloc_list(v, elements),
                    Instruction::AllocRecord { elements } => self.emitir_alloc_record(v, elements),
                    // O `alloca` nasce no bloco de entrada (`emit_buffers_de_closure`).
                    Instruction::Alloca(_) => {}
                    Instruction::Load { ptr, ty } => {
                        let sp = self.operand_str(ptr);
                        writeln!(self.out, "  %v{v} = load {}, ptr {sp}", ty.llvm_ir()).unwrap();
                    }
                    Instruction::Store { ptr, val } => {
                        // O local guarda a representação do seu tipo (R6); o
                        // valor chega já coagido pelo lowering, e aqui só se
                        // acerta a largura. Um ponteiro que não é `alloca`
                        // desta função (o endereço de um local de quem chama
                        // uma função local direta, `funcoes_diretas.rs`)
                        // grava na largura do valor, a do local de lá: um
                        // `bool` gravado como `i64` pisava 7 bytes do quadro
                        // de quem chamou (o `listenerHasError` do
                        // `_Future._propagateToListeners`).
                        let t = match ptr {
                            Operand::Val(p) => self.apontado.get(p).copied().unwrap_or_else(|| match self.tipo_de(val) {
                                Type::I1 => Type::I1,
                                Type::I8 => Type::I8,
                                Type::F64 => Type::F64,
                                // Local SIMD sem caixa de quem chama (16 bytes).
                                t if t.e_vetor() => t,
                                _ => Type::I64,
                            }),
                            _ => Type::I64,
                        };
                        let sp = self.operand_str(ptr);
                        let sv = self.coagir(val, t);
                        writeln!(self.out, "  store {} {sv}, ptr {sp}", t.llvm_ir()).unwrap();
                        // G2: o local `Ref` tem slot próprio, atualizado a
                        // cada gravação — ele vive mais que o SSA que o gravou.
                        if let Operand::Val(pv) = ptr
                            && !self.allocas_no_quadro.contains(pv)
                            && let Some(&slot) = self.slots.get(pv)
                        {
                            writeln!(self.out, "  store i64 {sv}, ptr %gcs{slot}").unwrap();
                        }
                    }
                    Instruction::Box { op, from } if from.e_vetor() => self.emitir_caixa_simd(v, op, *from),
                    Instruction::Unbox { op, to } if to.e_vetor() => self.emitir_descaixa_simd(v, op, *to),
                    Instruction::Simd { op, args } => {
                        let ty = self.tipos.get(vid).copied().unwrap_or(Type::Void);
                        self.emitir_simd(v, *op, args, ty);
                    }
                    Instruction::Box { op, from } => self.emitir_caixa(v, op, *from),
                    Instruction::Unbox { op, to } => self.emitir_descaixa(v, op, *to),
                    Instruction::LShr(a, b) => {
                        let sa = self.coagir(a, Type::I64);
                        let sb = self.coagir(b, Type::I64);
                        writeln!(self.out, "  %v{v} = lshr i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::Bitcast { op, to } => {
                        if *to == Type::F64 {
                            let so = self.coagir(op, Type::I64);
                            writeln!(self.out, "  %v{v} = bitcast i64 {so} to double").unwrap();
                        } else {
                            let so = self.coagir(op, Type::F64);
                            writeln!(self.out, "  %v{v} = bitcast double {so} to i64").unwrap();
                        }
                    }
                    Instruction::LoadGlobal { simbolo, ty } => {
                        self.endereco_do_global(v, simbolo);
                        writeln!(self.out, "  %v{v} = load {}, ptr %ga{v}", ty.llvm_ir()).unwrap();
                    }
                    Instruction::StoreGlobal { simbolo, val, ty, raiz } => {
                        let sv = self.coagir(val, *ty);
                        self.endereco_do_global(v, simbolo);
                        writeln!(self.out, "  store {} {sv}, ptr %ga{v}", ty.llvm_ir()).unwrap();
                        if raiz.is_some() {
                            // A raiz é identificada pelo endereço do slot:
                            // único entre os módulos e os isolados.
                            writeln!(self.out, "  %gr{v} = ptrtoint ptr %ga{v} to i64").unwrap();
                            writeln!(self.out, "  call void @dartforge_gc_global_root(i64 %gr{v}, i64 {sv})").unwrap();
                        }
                    }
                    Instruction::Phi { incoming, ty } => {
                        let t = ty.llvm_ir();
                        // A coercao de uma entrada de `phi` NAO pode ser emitida
                        // aqui: `phi` tem de ser a primeira instrucao do bloco e a
                        // conversao pertence ao bloco de origem. Entao so constantes
                        // sao reescritas no tipo do `phi`; valores que chegam com
                        // largura diferente sao corrigidos na origem, ao terminar
                        // aquele bloco.
                        let in_strs: Vec<String> = incoming
                            .iter()
                            .map(|(b, op)| {
                                let sop = self
                                    .constante_no_tipo(op, *ty)
                                    .or_else(|| match op {
                                        Operand::Val(v) => self
                                            .conv_phi
                                            .iter()
                                            .find(|(bb, _, _, vv, para)| {
                                                *bb == b.0 && vv == v && para.llvm_ir() == ty.llvm_ir()
                                            })
                                            .map(|(_, nome, _, _, _)| nome.clone()),
                                        _ => None,
                                    })
                                    .unwrap_or_else(|| self.operand_str(op));
                                match self.rotulos_de_saida.get(&b.0) {
                                    Some(r) => format!("[ {sop}, %{r} ]"),
                                    None => format!("[ {sop}, %b{} ]", b.0),
                                }
                            })
                            .collect();
                        let joined = in_strs.join(", ");
                        writeln!(self.out, "  %v{v} = phi {t} {joined}").unwrap();
                    }
                    _ if self.emit_closure_inst(v, inst, *ty) => {}
                    _ => {
                        // E3: o verificador da HIR recusa, antes da emissão,
                        // toda instrução sem lowering aqui (o antigo
                        // "; inst pendente" que o Clang aceitava calado).
                        unreachable!("instrução sem emissão passou pelo verificador: {inst:?}");
                    }
                }
                // Exceções por tabelas: a chamada Dart com pouso é `invoke`
                // (a raiz do resultado, abaixo, já sai no caminho normal).
                if let Some(pouso) = tab.and_then(|t| t.invocacoes.get(vid)) {
                    self.tornar_invoke(inicio_da_instrucao, v, pouso.0);
                }
                // G1: a raiz logo depois da definição — nada aloca entre
                // o retorno da chamada e este `set_root` (G5).
                if let Some(&slot) = self.slots.get(vid) {
                    if matches!(inst, Instruction::Phi { .. }) {
                        raizes_de_phi.push((slot, v));
                    } else if !matches!(inst, Instruction::Alloca(_)) {
                        writeln!(self.out, "  store i64 %v{v}, ptr %gcs{slot}").unwrap();
                    }
                } else if self.enraizados.contains(vid) {
                    if matches!(inst, Instruction::Phi { .. }) {
                        raizes_de_phi.push((usize::MAX, v));
                    } else {
                        self.raiz_no_mapa(v);
                    }
                }
                // Raízes por mapas: o que estava vivo na entrada deste ponto
                // de coleta (os operandos inclusive) continua vivo através
                // dele — é o que o põe no mapa da chamada.
                if tem_gc && let Some(vivos) = self.raizes_da_funcao.vivos_em.get(vid).cloned() {
                    self.manter_vivos(&vivos);
                }
                let _ = ty;
            }
            for (slot, pv) in std::mem::take(&mut raizes_de_phi) {
                self.raiz_de_phi(slot, pv);
            }

            // O build de conferência: o ponto de coleta do fim do bloco.
            if self.conferencia.is_some() {
                let vivos = self.raizes_da_funcao.vivos_no_fim.get(&block.id).cloned().unwrap_or_default();
                self.gravar_conferencia(&vivos);
            }
            for (b, nome, de, v, para) in self.conv_phi.clone() {
                if b == block.id.0 {
                    let origem = format!("%v{}", v.0);
                    self.emitir_conversao_nomeada(&nome, de, &origem, para);
                }
            }
            // Raízes por mapas: o ponto de coleta do fim do bloco (as
            // conversões acima podem encaixotar; o `throw` é tratado no
            // terminador, depois da chamada dele).
            if tem_gc
                && !matches!(block.terminator, Terminator::Throw(_))
                && let Some(vivos) = self.raizes_da_funcao.vivos_no_fim.get(&block.id).cloned()
            {
                self.manter_vivos(&vivos);
            }

            if let Some(p) = func.depuracao.as_ref().and_then(|d| d.saidas.get(&block.id))
                && posicao_escrita != Some(*p)
            {
                writeln!(self.out, "{}{} {}", depuracao::MARCADOR, p.0, p.1).unwrap();
                posicao_escrita = Some(*p);
                embutida_escrita = None;
            }
            // Exceções por tabelas: o `Return` com a exceção pendente não
            // retorna — desenrola até o pouso de quem a trata (que restaura
            // o topo da pilha-sombra: o quadro de raízes não é fechado
            // aqui). Com a pendência incerta, confere antes.
            if matches!(block.terminator, Terminator::Return(_)) {
                match tab.and_then(|t| t.saidas.get(&block.id)) {
                    Some(SaidaPorExcecao::Lanca) => {
                        writeln!(self.out, "  call void @df.lancar()").unwrap();
                        writeln!(self.out, "  unreachable").unwrap();
                        continue;
                    }
                    Some(SaidaPorExcecao::Guarda) => {
                        let b = block.id.0;
                        writeln!(self.out, "  %xgp{b} = load i8, ptr %ctx, align 8").unwrap();
                        writeln!(self.out, "  %xgn{b} = icmp ne i8 %xgp{b}, 0").unwrap();
                        writeln!(self.out, "  %xge{b} = call i1 @llvm.expect.i1(i1 %xgn{b}, i1 false)").unwrap();
                        writeln!(self.out, "  br i1 %xge{b}, label %xg{b}.lanca, label %xg{b}.ret").unwrap();
                        writeln!(self.out, "xg{b}.lanca:").unwrap();
                        writeln!(self.out, "  call void @df.lancar()").unwrap();
                        writeln!(self.out, "  unreachable").unwrap();
                        writeln!(self.out, "xg{b}.ret:").unwrap();
                    }
                    None => {}
                }
            }
            if self.rastro.is_some() && matches!(block.terminator, Terminator::Return(_)) {
                writeln!(self.out, "  call void @dartforge_rastro_saida()").unwrap();
            }
            // G3: o frame de raízes fecha antes de TODO `ret`, inclusive o
            // das saídas por exceção (que retornam o valor padrão).
            if self.tem_frame && matches!(block.terminator, Terminator::Return(_)) {
                writeln!(self.out, "  %gcvolta{} = load ptr, ptr %gcq, align 8", block.id.0).unwrap();
                writeln!(self.out, "  store ptr %gcvolta{}, ptr %ctxtopo, align 8", block.id.0).unwrap();
            }
            match &block.terminator {
                Terminator::Return(Some(op)) => {
                    if func.return_ty == Type::Void {
                        writeln!(self.out, "  ret void").unwrap();
                    } else {
                        let sop = self.coagir(op, func.return_ty);
                        writeln!(self.out, "  ret {} {sop}", func.return_ty.llvm_ir()).unwrap();
                    }
                }
                Terminator::Return(None) => {
                    let zero = match func.return_ty {
                        Type::F64 => "0.0",
                        Type::I1 => "false",
                        _ => "0",
                    };
                    if func.return_ty == Type::Void {
                        writeln!(self.out, "  ret void").unwrap();
                    } else {
                        writeln!(self.out, "  ret {} {zero}", func.return_ty.llvm_ir()).unwrap();
                    }
                }
                Terminator::Branch(target) => {
                    writeln!(self.out, "  br label %b{}", target.0).unwrap();
                }
                // O desvio ao pouso é o do `invoke` deste bloco
                // (`otimizar/tabelas.rs`): aqui só segue a continuação.
                Terminator::CondBranch { then_block, else_block, .. } if tab.is_some_and(|t| t.pousos.contains(then_block)) => {
                    writeln!(self.out, "  br label %b{}", else_block.0).unwrap();
                }
                Terminator::CondBranch { cond, then_block, else_block } => {
                    let sc = self.coagir(cond, Type::I1);
                    writeln!(self.out, "  br i1 {sc}, label %b{}, label %b{}", then_block.0, else_block.0).unwrap();
                }
                Terminator::Switch { val, default, cases } => {
                    let sv = self.coagir(val, Type::I64);
                    write!(self.out, "  switch i64 {sv}, label %b{} [", default.0).unwrap();
                    for (c, b) in cases {
                        write!(self.out, " i64 {c}, label %b{}", b.0).unwrap();
                    }
                    writeln!(self.out, " ]").unwrap();
                }
                Terminator::Throw(op) => {
                    let tag = match self.tipo_de(op) {
                        Type::I64 => 1,
                        Type::I1 | Type::I8 => 2,
                        Type::F64 => 4,
                        _ => 3,
                    };
                    let sop = self.coagir(op, Type::I64);
                    writeln!(self.out, "  call void @dartforge_exception_throw(i64 {sop}, i8 {tag})").unwrap();
                    if tem_gc && let Some(vivos) = self.raizes_da_funcao.vivos_no_fim.get(&block.id).cloned() {
                        self.manter_vivos(&vivos);
                    }
                    if tab.is_some() {
                        writeln!(self.out, "  call void @df.lancar()").unwrap();
                    }
                    writeln!(self.out, "  unreachable").unwrap();
                }
                Terminator::Unreachable => {
                    writeln!(self.out, "  unreachable").unwrap();
                }
            }
        }

        writeln!(self.out, "}}\n").unwrap();
        // §13.14: os rótulos do rastro, antes do DWARF (que consome os
        // marcadores de posição).
        if (func.depuracao.is_some() || compartilhada)
            && let Some(mut r) = self.rastro_vm.take()
        {
            let texto = self.out.split_off(inicio_da_funcao);
            let rotulado = r.rotular(&texto, func, func.depuracao.as_deref(), self.depuracao.is_some(), compartilhada);
            self.out.push_str(&rotulado);
            self.rastro_vm = Some(r);
        }
        if let Some(d) = func.depuracao.as_deref()
            && let Some(mut dep) = self.depuracao.take()
        {
            let texto = self.out.split_off(inicio_da_funcao);
            let anotado = dep.anotar(&texto, func, d);
            self.out.push_str(&anotado);
            self.depuracao = Some(dep);
        }
    }

    /// O número de slots do quadro de raízes da função em emissão: os de
    /// sempre e, no build de conferência, os de conferência depois deles.
    fn tamanho_do_quadro(&self) -> usize {
        match self.conferencia {
            Some((primeiro, k)) => primeiro + k,
            None => self.slots.values().max().map_or(0, |m| m + 1),
        }
    }

    /// O campo `n` do quadro (`QuadroDeRaizes` do runtime): o número de
    /// slots e, nos bits 40 em diante, quantos dos últimos são de
    /// conferência.
    fn cabecalho_do_quadro(&self, n: usize) -> u64 {
        match self.conferencia {
            Some((_, k)) => n as u64 | ((k as u64) << 40),
            None => n as u64,
        }
    }

    /// O build de conferência: grava `vivos` nos slots de conferência e zera
    /// os que sobram.
    fn gravar_conferencia(&mut self, vivos: &[ValueId]) {
        let Some((primeiro, k)) = self.conferencia else { return };
        for i in 0..k {
            match vivos.get(i) {
                Some(x) => writeln!(self.out, "  store i64 %v{}, ptr %gcs{}", x.0, primeiro + i).unwrap(),
                None => writeln!(self.out, "  store i64 0, ptr %gcs{}", primeiro + i).unwrap(),
            }
        }
    }

    /// Raízes por mapas: `%raiz<v>`, o valor `Ref` `%v<v>` como ponteiro
    /// `addrspace(1)` — os mesmos bits, na forma que o
    /// `rewrite-statepoints-for-gc` reconhece como referência do coletor. O
    /// código continua usando o `i64` (o coletor não move objetos); o
    /// ponteiro só existe para o valor aparecer no mapa de pilha.
    fn raiz_no_mapa(&mut self, v: u32) {
        writeln!(self.out, "  %raiz{v} = inttoptr i64 %v{v} to ptr addrspace(1)").unwrap();
    }

    /// A raiz de um `phi`, emitida depois do último `phi` do bloco: o `store`
    /// no slot do quadro, ou (slot `usize::MAX`) o ponteiro do mapa.
    fn raiz_de_phi(&mut self, slot: usize, v: u32) {
        if slot == usize::MAX {
            self.raiz_no_mapa(v);
        } else {
            writeln!(self.out, "  store i64 %v{v}, ptr %gcs{slot}").unwrap();
        }
    }

    /// Raízes por mapas: mantém vivos, até aqui, os valores enraizados
    /// `vivos` — um uso que não gera código (`llvm.fake.use`) logo depois de
    /// um ponto de coleta. É ele que faz o valor estar vivo *através* da
    /// chamada e, portanto, no mapa dela; vale também para o argumento cujo
    /// último uso é a própria chamada (o runtime conta com quem chama para
    /// mantê-lo vivo enquanto ela roda).
    fn manter_vivos(&mut self, vivos: &[ValueId]) {
        if crate::alvo::sabotagem("sem_uso_ficticio") {
            return;
        }
        if !self.enraizados.is_empty() && crate::alvo::sabotagem("bruto_no_mapa") {
            self.out.push_str("  call void (...) @llvm.fake.use(ptr addrspace(1) %raizsab)\n");
        }
        for x in vivos {
            writeln!(self.out, "  call void (...) @llvm.fake.use(ptr addrspace(1) %raiz{})", x.0).unwrap();
        }
    }

    /// Exceções por tabelas: a chamada Dart que a instrução `v` acabou de
    /// emitir (a última linha desde `inicio`: a de `CallStatic`,
    /// `ChamadaTipada`, da closure ou do seletor) vira
    /// `invoke … to label %inv<v>.fim unwind label %b<pouso>`, e a emissão
    /// continua no rótulo do caminho normal — o bloco da HIR passa a
    /// terminar nele ([`LlvmEmitter::rotulos_de_saida`]). As chamadas ao
    /// runtime que a mesma instrução emite antes (a entrada da closure, a
    /// busca do seletor) não desenrolam e continuam `call`.
    fn tornar_invoke(&mut self, inicio: usize, v: u32, pouso: u32) {
        let texto = self.out.split_off(inicio);
        let corpo = texto.strip_suffix('\n').unwrap_or(&texto);
        let (antes, chamada) = match corpo.rfind('\n') {
            Some(i) => (&corpo[..=i], &corpo[i + 1..]),
            None => ("", corpo),
        };
        assert!(
            chamada.starts_with("  call ") || chamada.contains(" = call "),
            "bug do compilador: a instrução com pouso não termina numa chamada: {chamada}"
        );
        self.out.push_str(antes);
        self.out.push_str(&chamada.replacen("call ", "invoke ", 1));
        writeln!(self.out, " to label %inv{v}.fim unwind label %b{pouso}").unwrap();
        writeln!(self.out, "inv{v}.fim:").unwrap();
        self.rotulo_atual = format!("inv{v}.fim");
    }

    /// `%cf<v>`: o ponteiro da entrada uniforme da closure `c` — o código
    /// dela, ou `@df_clo_invalido` quando o valor não é closure (o runtime
    /// devolve 0 e deixa o `NoSuchMethodError` pendente).
    fn entrada_da_closure(&mut self, v: u32, c: &str) {
        writeln!(self.out, "  %cc{v} = call i64 @dartforge_closure_entry(i64 {c})").unwrap();
        writeln!(self.out, "  %cz{v} = icmp eq i64 %cc{v}, 0").unwrap();
        writeln!(
            self.out,
            "  %cq{v} = select i1 %cz{v}, i64 ptrtoint (ptr @df_clo_invalido to i64), i64 %cc{v}"
        )
        .unwrap();
        writeln!(self.out, "  %cf{v} = inttoptr i64 %cq{v} to ptr").unwrap();
    }

    /// O descritor de uma chamada pela convenção uniforme:
    /// [n_posicionais, n_nomeados, hash(nome)…].
    fn descritor(args: usize, nomes: &[String]) -> Vec<i64> {
        let mut d = vec![(args - nomes.len()) as i64, nomes.len() as i64];
        d.extend(nomes.iter().map(|n| crate::lower::closures::hash_nome(n)));
        d
    }

    fn registrar_vetor(&mut self, v: Vec<i64>) -> usize {
        if let Some(&k) = self.vetor_de.get(&v) {
            return k;
        }
        let k = self.vetores.len();
        self.vetor_de.insert(v.clone(), k);
        self.vetores.push(v);
        k
    }

    /// `@df_clo_invalido` (a entrada que a chamada usa quando o valor não é
    /// closure — o runtime já deixou o `NoSuchMethodError` pendente) e os
    /// vetores constantes.
    fn emit_closures(&mut self) {
        let modulo = self.module;
        for func in &modulo.functions {
            for block in &func.blocks {
                for (_, inst, _) in &block.instructions {
                    match inst {
                        Instruction::ConstArray(v) => {
                            self.registrar_vetor(v.clone());
                        }
                        Instruction::TabelaDeFuncoes(v) => {
                            if !self.tabela_de_funcoes_de.contains_key(v) {
                                self.tabela_de_funcoes_de.insert(v.clone(), self.tabelas_de_funcoes.len());
                                self.tabelas_de_funcoes.push(v.clone());
                            }
                            for s in v {
                                if !self.funcao_por_simbolo.contains_key(s.as_str()) {
                                    self.anotar_externo(s, Type::Ref, &[]);
                                }
                            }
                        }
                        Instruction::CallClosure { args, nomes, .. } | Instruction::CallSeletor { args, nomes, .. } => {
                            self.registrar_vetor(Self::descritor(args.len(), nomes));
                            self.nomes_de_argumento.extend(nomes.iter().cloned());
                        }
                        _ => {}
                    }
                }
            }
        }
        if self.module.modo_sdk {
            // O descritor sem argumentos do `dartforge_dispatch_toString`.
            self.registrar_vetor(vec![0, 0]);
        }
        self.out.push_str("; Closures: entrada inválida e vetores constantes\n");
        self.out.push_str("define internal i64 @df_clo_invalido(i64 %c, ptr %a, ptr %d) {\nb0:\n  ret i64 0\n}\n");
        for (k, v) in self.vetores.iter().enumerate() {
            let itens: Vec<String> = v.iter().map(|x| format!("i64 {x}")).collect();
            writeln!(
                self.out,
                "@df.arr.{k} = private unnamed_addr constant [{} x i64] [{}]",
                v.len(),
                itens.join(", ")
            )
            .unwrap();
        }
        for (k, v) in self.tabelas_de_funcoes.iter().enumerate() {
            let itens: Vec<String> = v.iter().map(|s| format!("ptr @{s}")).collect();
            writeln!(self.out, "@df.fns.{k} = private unnamed_addr constant [{} x ptr] [{}]", v.len(), itens.join(", ")).unwrap();
        }
        self.out.push('\n');
    }

    /// Converte os bits i64 lidos de uma célula/ambiente (%u) para a
    /// representação `ty` do resultado `%v<v>`.
    fn bits_para_repr(&mut self, v: u32, bits: &str, ty: Type) {
        match ty {
            Type::F64 => writeln!(self.out, "  %v{v} = bitcast i64 {bits} to double").unwrap(),
            Type::I1 => writeln!(self.out, "  %v{v} = icmp ne i64 {bits}, 0").unwrap(),
            Type::I8 => writeln!(self.out, "  %v{v} = trunc i64 {bits} to i8").unwrap(),
            _ => writeln!(self.out, "  %v{v} = add i64 {bits}, 0").unwrap(),
        }
    }

    /// Tag da ABI (bits, tag) de um valor pela representação (E1).
    fn tag_de(&self, op: &Operand) -> u8 {
        match self.tipo_de(op) {
            Type::I64 => 1,
            Type::I1 | Type::I8 => 2,
            Type::F64 => 4,
            _ => 3,
        }
    }

    /// Emite uma instrução das closures (P1); `false` se não é uma delas.
    fn emit_closure_inst(&mut self, v: u32, inst: &Instruction, ty: Type) -> bool {
        match inst {
            Instruction::AllocCell { value } => self.emitir_alloc_cell(v, value),
            Instruction::CellGet { cell } => self.emitir_cell_get(v, cell, ty),
            Instruction::CellSet { cell, value } => self.emitir_cell_set(cell, value),
            Instruction::EnvGet { env, index } => self.emitir_env_get(v, env, *index, ty),
            Instruction::JuntarTextos { partes } => self.emitir_juntar_textos(v, partes),
            Instruction::AllocEnv { values } => self.emitir_alloc_env(v, values),
            Instruction::AllocClosure { code_symbol, env } => self.emitir_alloc_closure(v, code_symbol, env),
            Instruction::AllocClosureTipada { code_symbol, env, tipado, abi, direto } => {
                self.emitir_alloc_closure_tipada(v, code_symbol, env, tipado, *abi, *direto);
            }
            Instruction::ChamadaTipada { alvo, args, ret } => {
                let a = self.coagir(alvo, Type::I64);
                let mut partes = Vec::with_capacity(args.len());
                for (x, t) in args {
                    let s = self.coagir(x, *t);
                    partes.push(format!("{} {s}", t.llvm_ir()));
                }
                writeln!(self.out, "  %ct{v} = inttoptr i64 {a} to ptr").unwrap();
                if *ret == Type::Void {
                    writeln!(self.out, "  call void %ct{v}({})", partes.join(", ")).unwrap();
                } else {
                    writeln!(self.out, "  %v{v} = call {} %ct{v}({})", ret.llvm_ir(), partes.join(", ")).unwrap();
                }
            }
            Instruction::TearOff { code_symbol } => self.emitir_tearoff(v, code_symbol),
            Instruction::CallClosure { closure, args, nomes, tupla_tipos, .. } => {
                let k = self.vetor_de[&Self::descritor(args.len(), nomes)];
                // O slot depois dos argumentos leva a tupla de tipos (a
                // mesma convenção da chamada por seletor).
                let n = args.len() + 1;
                for (i, a) in args.iter().enumerate() {
                    let s = self.coagir(a, Type::I64);
                    writeln!(self.out, "  %ca{v}_{i} = getelementptr [{n} x i64], ptr %cargs{v}, i64 0, i64 {i}").unwrap();
                    writeln!(self.out, "  store i64 {s}, ptr %ca{v}_{i}").unwrap();
                }
                let tupla = self.coagir(tupla_tipos, Type::I64);
                writeln!(self.out, "  %cat{v} = getelementptr [{n} x i64], ptr %cargs{v}, i64 0, i64 {}", args.len()).unwrap();
                writeln!(self.out, "  store i64 {tupla}, ptr %cat{v}").unwrap();
                let c = self.coagir(closure, Type::Ref);
                self.entrada_da_closure(v, &c);
                writeln!(self.out, "  %v{v} = call i64 %cf{v}(i64 {c}, ptr %cargs{v}, ptr @df.arr.{k})").unwrap();
            }
            Instruction::CallClosureRepasse { closure, args, desc } => {
                let c = self.coagir(closure, Type::Ref);
                let a = self.operand_str(args);
                let d = self.operand_str(desc);
                self.entrada_da_closure(v, &c);
                writeln!(self.out, "  %v{v} = call i64 %cf{v}(i64 {c}, ptr {a}, ptr {d})").unwrap();
            }
            Instruction::CallSeletor { seletor, recv, args, nomes, tupla_tipos } => {
                self.emitir_chamada_por_seletor(v, seletor, recv, args, nomes, tupla_tipos);
            }
            Instruction::CallSeletorRepasse { seletor, recv, args, desc } => {
                self.emitir_repasse_por_seletor(v, seletor, recv, args, desc);
            }
            Instruction::LoadIndexed { base, index } => {
                let b = self.operand_str(base);
                let i = self.coagir(index, Type::I64);
                writeln!(self.out, "  %li{v} = getelementptr i64, ptr {b}, i64 {i}").unwrap();
                writeln!(self.out, "  %v{v} = load i64, ptr %li{v}").unwrap();
            }
            Instruction::ConstArray(vals) => {
                let k = self.vetor_de[vals];
                writeln!(self.out, "  %v{v} = getelementptr i64, ptr @df.arr.{k}, i64 0").unwrap();
            }
            Instruction::TabelaDeFuncoes(fns) => {
                let k = self.tabela_de_funcoes_de[fns];
                writeln!(self.out, "  %v{v} = getelementptr ptr, ptr @df.fns.{k}, i64 0").unwrap();
            }
            _ => return false,
        }
        true
    }

    /// Todo `alloca` da função nasce no bloco de entrada — os vetores das
    /// closures (argumentos, pares do ambiente), os dos literais de lista,
    /// mapa e record, e os locais: um `alloca` fora da entrada reserva pilha
    /// nova a cada execução, e num laço a pilha cresce até estourar.
    fn emit_buffers_de_closure(&mut self, func: &Function) {
        for block in &func.blocks {
            for (vid, inst, _) in &block.instructions {
                match inst {
                    Instruction::Alloca(_) if self.allocas_no_quadro.contains(vid) => {}
                    Instruction::Alloca(ty) => {
                        writeln!(self.out, "  %v{} = alloca {}", vid.0, ty.llvm_ir()).unwrap();
                    }
                    Instruction::AllocList { elements } => self.buffer_de_lista(vid.0, elements),
                    Instruction::AllocRecord { elements } => self.buffer_de_record(vid.0, elements),
                    Instruction::CallClosure { args, .. } => {
                        writeln!(self.out, "  %cargs{} = alloca [{} x i64]", vid.0, args.len() + 1).unwrap();
                    }
                    Instruction::CallSeletor { args, .. } => {
                        writeln!(self.out, "  %sargs{} = alloca [{} x i64]", vid.0, args.len() + 1).unwrap();
                    }
                    Instruction::AllocEnv { values } => self.buffer_de_ambiente(vid.0, values),
                    Instruction::JuntarTextos { partes } => self.buffer_de_juntar_textos(vid.0, partes),
                    _ => {}
                }
            }
        }
    }

    /// Os globais do módulo (`@dfg_<id>` e a bandeira `$ok`) viram slots da
    /// área de globais do isolado: a VM guarda os estáticos na *field table*
    /// de cada isolado, e aqui cada isolado (uma thread) tem a sua área, que
    /// o runtime cria na primeira vez (`dartforge_area_de_globais`, zerada).
    /// Os caches dos seletores (`seletores.rs`) ganham slots depois destes.
    ///
    /// Numa geração de hot reload ([`LlvmEmitter::com_area_anterior`]), a
    /// área começa com o layout da geração viva inteiro — inclusive os slots
    /// que sumiram e os caches de seletor dela, que o código antigo ainda
    /// usa — e só acrescenta: um global que continua fica no mesmo índice.
    fn emit_globais(&mut self) {
        let anterior: std::collections::HashMap<i64, usize> = match &self.area_anterior {
            Some(a) => {
                self.hashes_de_slot = a.clone();
                a.iter().enumerate().filter(|(_, h)| **h != 0).map(|(i, h)| (*h, i)).collect()
            }
            None => std::collections::HashMap::new(),
        };
        self.indice_de_slot.clone_from(&anterior);
        for (_, _, simbolo) in &self.module.globais {
            for nome in [simbolo.clone(), format!("{simbolo}$ok")] {
                let h = hash_de_slot(&nome);
                let n = match anterior.get(&h) {
                    Some(&i) => i,
                    None => {
                        self.hashes_de_slot.push(h);
                        self.hashes_de_slot.len() - 1
                    }
                };
                self.slots_de_global.insert(nome, n);
            }
        }
    }

    /// O descritor da área de globais do módulo: `[chave, n, nome_0…]`, com
    /// a chave estável do módulo e o hash do nome de cada slot. Os slots de
    /// cache de seletor têm [`BIT_DE_CACHE`] no nome: não migram para uma
    /// área nova e a publicação de uma recarga os zera (guardam endereços de
    /// código, e as tabelas de métodos acabaram de mudar). Em produção,
    /// `[chave, -n]` ([`LlvmEmitter::com_area_enxuta`]).
    fn emitir_descritor_da_area(&mut self) {
        let chave = self.module.registro.clone().unwrap_or_else(|| "df.programa".to_string());
        if self.area_enxuta {
            // Produção (`com_area_enxuta`): `[chave, -n]`, sem os nomes.
            let n = self.hashes_de_slot.len() as i64;
            writeln!(self.out, "@df.area = private unnamed_addr constant [2 x i64] [i64 {}, i64 {}]", hash_de_slot(&chave), -n)
                .unwrap();
            return;
        }
        let mut valores = vec![hash_de_slot(&chave).to_string(), String::new()];
        for h in &self.hashes_de_slot {
            valores.push(h.to_string());
        }
        valores[1] = (valores.len() - 2).to_string();
        let itens: Vec<String> = valores.iter().map(|v| format!("i64 {v}")).collect();
        writeln!(self.out, "@df.area = private unnamed_addr constant [{} x i64] [{}]", itens.len(), itens.join(", ")).unwrap();
    }

    /// O slot do próximo cache de seletor da função em emissão: as duas
    /// palavras (id de classe, entrada) com o nome `(função, posição)`, no
    /// mesmo índice da geração viva quando ela já o tinha — o código de uma
    /// função que não mudou é o mesmo texto nas duas gerações (J04).
    pub(super) fn slot_de_cache(&mut self) -> usize {
        let nome = format!("{}#{}", self.funcao_atual, self.cache_na_funcao);
        self.cache_na_funcao += 1;
        let (h0, h1) = (hash_de_cache(&nome, 0), hash_de_cache(&nome, 1));
        if let Some(&i) = self.indice_de_slot.get(&h0)
            && self.hashes_de_slot.get(i + 1) == Some(&h1)
        {
            return i;
        }
        let i = self.hashes_de_slot.len();
        self.hashes_de_slot.push(h0);
        self.hashes_de_slot.push(h1);
        i
    }

    /// Os `alloca` `Ref` usados fora do `load`/`store` deles (o endereço
    /// passado a uma função local direta): ver `allocas_no_quadro`.
    fn allocas_ref_que_escapam(func: &Function) -> std::collections::HashSet<ValueId> {
        let mut refs = std::collections::HashSet::new();
        for b in &func.blocks {
            for (vid, inst, _) in &b.instructions {
                if matches!(inst, Instruction::Alloca(Type::Ref)) {
                    refs.insert(*vid);
                }
            }
        }
        let mut escapam = std::collections::HashSet::new();
        if refs.is_empty() {
            return escapam;
        }
        for b in &func.blocks {
            for (_, inst, _) in &b.instructions {
                let usos = match inst {
                    Instruction::Load { .. } => Vec::new(),
                    Instruction::Store { val: Operand::Val(v), .. } => vec![*v],
                    Instruction::Store { .. } => Vec::new(),
                    outro => crate::lower::async_sm::usos_de(outro),
                };
                escapam.extend(usos.into_iter().filter(|v| refs.contains(v)));
            }
            escapam.extend(crate::lower::async_sm::usos_do_terminador(&b.terminator).into_iter().filter(|v| refs.contains(v)));
        }
        escapam
    }

    /// A função usa a área de globais (global, bandeira ou cache de seletor)?
    fn usa_area(func: &Function) -> bool {
        func.blocks.iter().any(|b| {
            b.instructions.iter().any(|(_, i, _)| {
                matches!(
                    i,
                    Instruction::LoadGlobal { .. }
                        | Instruction::StoreGlobal { .. }
                        | Instruction::CallSeletor { .. }
                        | Instruction::CallSeletorRepasse { .. }
                        | Instruction::Const(Constant::String(_) | Constant::StringWtf8(_))
                ) || Self::rti_avaliar_em_cache(i)
            })
        })
    }

    /// O quadro de raízes no topo do bloco de entrada (ver o prólogo).
    fn quadro_na_entrada() -> bool {
        static DESLIGADO: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        !*DESLIGADO.get_or_init(|| std::env::var_os("DARTFORGE_SEM_QUADRO_NA_ENTRADA").is_some_and(|v| !v.is_empty() && v != "0"))
    }

    /// `dartforge_rti_avaliar` sem tupla e com `this`: vai pelo cache do
    /// ponto de uso na área ([`Self::emitir_rti_avaliar_em_cache`]).
    /// `DARTFORGE_SEM_CACHE_RTI=1` na compilação desliga (medida).
    fn rti_avaliar_em_cache(i: &Instruction) -> bool {
        static DESLIGADO: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        matches!(i, Instruction::CallRuntime { name, args, .. }
            if name == "dartforge_rti_avaliar"
                && args.len() == 4
                && matches!(args[3].0, Operand::Constant(Constant::Int(0)))
                && !matches!(args[1].0, Operand::Constant(_)))
            && !*DESLIGADO.get_or_init(|| std::env::var_os("DARTFORGE_SEM_CACHE_RTI").is_some_and(|v| !v.is_empty() && v != "0"))
    }

    /// `P<i>` avaliado com cache no ponto de uso (N17): duas palavras da
    /// área do isolado guardam a chave do tipo de `this` — o metadado do
    /// cabeçalho do objeto (`heap::Cabecalho`, deslocamento 12 do bloco, 10
    /// do handle), ou `-16 - classe` sem metadado, a mesma chave da memória
    /// do runtime (`chave_do_valor`) — e o resultado. Um objeto do espaço
    /// com a chave gravada responde com duas cargas; o resto (valores do
    /// runtime, a primeira vez, outra chave) chama
    /// `dartforge_rti_avaliar_cache`, que responde e grava. O modelo e a
    /// classe são constantes do ponto; a tupla é 0. A área é por isolado (os
    /// ids de tipo são do universo dele) e a recarga zera os caches.
    fn emitir_rti_avaliar_em_cache(&mut self, v: u32, args: &[(Operand, Type)]) {
        let modelo = self.coagir(&args[0].0, Type::I64);
        let this = self.coagir(&args[1].0, Type::I64);
        let classe = self.coagir(&args[2].0, Type::I64);
        let slot = self.slot_de_cache();
        let o = &mut self.out;
        writeln!(o, "  %ra{v}c = getelementptr i64, ptr %area, i64 {slot}").unwrap();
        writeln!(o, "  %ra{v}m = and i64 {this}, {MASCARA_DE_OBJETO}").unwrap();
        writeln!(o, "  %ra{v}o = icmp eq i64 %ra{v}m, 2").unwrap();
        writeln!(o, "  br i1 %ra{v}o, label %ra{v}.obj, label %ra{v}.lenta").unwrap();
        writeln!(o, "ra{v}.obj:").unwrap();
        writeln!(o, "  %ra{v}p = inttoptr i64 {this} to ptr").unwrap();
        writeln!(o, "  %ra{v}mp = getelementptr inbounds i8, ptr %ra{v}p, i64 10").unwrap();
        writeln!(o, "  %ra{v}md = load i32, ptr %ra{v}mp, align 4").unwrap();
        writeln!(o, "  %ra{v}cp = getelementptr inbounds i8, ptr %ra{v}p, i64 2").unwrap();
        writeln!(o, "  %ra{v}cl = load i32, ptr %ra{v}cp, align 4").unwrap();
        writeln!(o, "  %ra{v}mz = zext i32 %ra{v}md to i64").unwrap();
        writeln!(o, "  %ra{v}cs = sext i32 %ra{v}cl to i64").unwrap();
        writeln!(o, "  %ra{v}cn = sub i64 -16, %ra{v}cs").unwrap();
        writeln!(o, "  %ra{v}tm = icmp ne i32 %ra{v}md, 0").unwrap();
        writeln!(o, "  %ra{v}k = select i1 %ra{v}tm, i64 %ra{v}mz, i64 %ra{v}cn").unwrap();
        writeln!(o, "  %ra{v}k0 = load i64, ptr %ra{v}c, align 8").unwrap();
        writeln!(o, "  %ra{v}ok = icmp eq i64 %ra{v}k0, %ra{v}k").unwrap();
        writeln!(o, "  br i1 %ra{v}ok, label %ra{v}.acerto, label %ra{v}.lenta").unwrap();
        writeln!(o, "ra{v}.acerto:").unwrap();
        writeln!(o, "  %ra{v}rp = getelementptr inbounds i64, ptr %ra{v}c, i64 1").unwrap();
        writeln!(o, "  %ra{v}r1 = load i64, ptr %ra{v}rp, align 8").unwrap();
        writeln!(o, "  br label %ra{v}.fim").unwrap();
        writeln!(o, "ra{v}.lenta:").unwrap();
        writeln!(o, "  %ra{v}r2 = call i64 @dartforge_rti_avaliar_cache(ptr %ra{v}c, i64 {modelo}, i64 {this}, i64 {classe})").unwrap();
        writeln!(o, "  br label %ra{v}.fim").unwrap();
        writeln!(o, "ra{v}.fim:").unwrap();
        writeln!(o, "  %v{v} = phi i64 [ %ra{v}r1, %ra{v}.acerto ], [ %ra{v}r2, %ra{v}.lenta ]").unwrap();
        self.rotulo_atual = format!("ra{v}.fim");
    }

    /// O endereço do slot de um global, em `%ga<v>`.
    fn endereco_do_global(&mut self, v: u32, simbolo: &str) {
        let slot = *self
            .slots_de_global
            .get(simbolo)
            .unwrap_or_else(|| panic!("bug do compilador: global @{simbolo} sem slot na área"));
        writeln!(self.out, "  %ga{v} = getelementptr i64, ptr %area, i64 {slot}").unwrap();
    }

    fn emit_dispatch_functions(&mut self) {
        self.out.push_str("define i64 @dartforge_dispatch_toString(i64 %obj) {\n");
        self.out.push_str("b0:\n");
        self.out.push_str("  %is_null = icmp eq i64 %obj, 0\n");
        self.out.push_str("  br i1 %is_null, label %ret_null, label %check_obj\n");
        self.out.push_str("ret_null:\n");
        self.out.push_str("  %null_s = call i64 @dartforge_to_string_handle(i64 0)\n");
        self.out.push_str("  ret i64 %null_s\n");
        self.out.push_str("check_obj:\n");
        self.out.push_str("  %cls = call i64 @df.classe(i64 %obj)\n");

        let mut cases = Vec::new();
        for class in &self.module.classes {
            if let Some(sym) = &class.to_string_symbol {
                cases.push((class.id, sym.clone()));
            }
        }

        if cases.is_empty() {
            self.out.push_str("  br label %fallback\n");
        } else {
            write!(self.out, "  switch i64 %cls, label %fallback [").unwrap();
            for (cid, _) in &cases {
                write!(self.out, " i64 {cid}, label %case_{cid}").unwrap();
            }
            writeln!(self.out, " ]").unwrap();
            for (cid, sym) in &cases {
                writeln!(self.out, "case_{cid}:").unwrap();
                writeln!(self.out, "  %res_{cid} = call i64 @{sym}(i64 %obj)").unwrap();
                writeln!(self.out, "  ret i64 %res_{cid}").unwrap();
            }
        }
        self.out.push_str("fallback:\n");
        self.out.push_str("  %fb = call i64 @dartforge_to_string_handle(i64 %obj)\n");
        self.out.push_str("  ret i64 %fb\n");
        self.out.push_str("}\n\n");
    }

    fn emit_entry(&mut self) {
        if self.module.excecoes_por_tabelas {
            self.emitir_portas();
        }
        if self.module.modo_sdk {
            self.emitir_registro("df.registrar.programa");
            let ids: Vec<String> = self.module.cids_do_runtime.iter().map(|c| format!("i64 {c}")).collect();
            writeln!(
                self.out,
                "@df.cids = private unnamed_addr constant [{} x i64] [{}]",
                ids.len(),
                ids.join(", ")
            )
            .unwrap();
            if let Some(v) = &self.module.versao_do_sdk {
                writeln!(
                    self.out,
                    "@df.versao_do_sdk = private unnamed_addr constant [{} x i8] c\"{}\"",
                    v.len(),
                    seletores::bytes_llvm(v)
                )
                .unwrap();
            }
            // A preparação de um isolado (a principal e a de cada
            // `Isolate.spawn`, que o runtime chama na thread nova): os
            // registros das bibliotecas, a RTI e o embedder.
            writeln!(self.out, "define void @df.preparar_isolado() {{").unwrap();
            // A memória ARC, ligada antes de todo código Dart do isolado.
            if self.module.memoria_arc {
                writeln!(self.out, "  call void @dartforge_memoria_arc_v1()").unwrap();
            }
            writeln!(self.out, "  call void @dartforge_registrar_cids(ptr @df.cids, i64 {})", ids.len()).unwrap();
            if let Some(v) = &self.module.versao_do_sdk {
                writeln!(self.out, "  call void @dartforge_registrar_versao_do_sdk(ptr @df.versao_do_sdk, i64 {})", v.len()).unwrap();
            }
            // As tabelas das classes dos valores do runtime (que não passam
            // por `dartforge_object_new_t`).
            for c in self.module.cids_do_runtime.clone() {
                if let Some(f) = self.module.funcoes_de_tabela.get(&(c as u32)).cloned() {
                    self.anotar_externo(&f, Type::Ptr, &[]);
                    writeln!(self.out, "  call void @dartforge_registrar_tabela(i64 {c}, ptr @{f})").unwrap();
                }
            }
            for r in &self.module.registros_do_sdk {
                writeln!(self.out, "  call void @{r}()").unwrap();
                self.externos.insert(r.clone(), format!("declare void @{r}()"));
            }
            writeln!(self.out, "  call void @df.registrar.programa()").unwrap();
            // RTI e laço de eventos, como na entrada de sempre (abaixo).
            if let Some(iniciar) = &self.module.iniciar_rti {
                writeln!(self.out, "  {}", chamada_de_entrada(self.module.excecoes_por_tabelas, iniciar, &[])).unwrap();
            }
            // O que o embedder da VM prepara antes do `main` (o script de
            // `dart:io`, o `Uri.base`), já com as bibliotecas registradas.
            // `dart:ffi`: os tipos nativos e os trampolins das assinaturas
            // (`lower/ffi.rs`).
            for c in self.module.ffi_compostos.clone() {
                writeln!(
                    self.out,
                    "  call void @dartforge_ffi_registrar_composto(i64 {}, i64 {}, i64 {}, i64 {}, i64 {}, i64 {}, i64 {})",
                    c.rti, c.classe, c.campos, c.indice_base, c.indice_deslocamento, c.tamanho, c.alinhamento
                )
                .unwrap();
            }
            for (c, letra) in &self.module.ffi_tipos {
                writeln!(self.out, "  call void @dartforge_ffi_registrar_tipo(i64 {c}, i64 {})", u32::from(*letra)).unwrap();
            }
            for (i, (chave, simbolo)) in self.module.ffi_trampolins.iter().enumerate() {
                writeln!(
                    self.out,
                    "  call void @dartforge_ffi_registrar_trampolim(ptr @df.ffi.chave.{i}, i64 {}, ptr @\"{simbolo}\")",
                    chave.len()
                )
                .unwrap();
            }
            for (i, cb) in self.module.ffi_callbacks.iter().enumerate() {
                writeln!(
                    self.out,
                    "  call void @dartforge_ffi_registrar_callback(ptr @df.ffi.cbchave.{i}, i64 {}, ptr @df.ffi.cbentrada.{i})",
                    cb.chave.len()
                )
                .unwrap();
            }
            writeln!(self.out, "  call void @dartforge_preparar_embedder()").unwrap();
            writeln!(self.out, "  ret void\n}}\n").unwrap();
            for (i, (chave, _)) in self.module.ffi_trampolins.iter().enumerate() {
                writeln!(self.out, "@df.ffi.chave.{i} = private unnamed_addr constant [{} x i8] c\"{chave}\"", chave.len()).unwrap();
            }
            self.emit_callbacks_ffi();
            writeln!(self.out, "define void @dartforge_entry() {{").unwrap();
            // Exceções por tabelas: as portas pelas quais o runtime chama
            // código Dart, entregues antes de qualquer código Dart rodar.
            if self.module.excecoes_por_tabelas {
                writeln!(self.out, "  call void @dartforge_registrar_portas(ptr @df.portas, i64 {N_PORTAS})").unwrap();
            }
            let chamar = self.module.chamar_dart.as_ref().map_or("null".to_string(), |c| format!("@{c}"));
            writeln!(self.out, "  call void @dartforge_registrar_isolados(ptr @df.preparar_isolado, ptr {chamar})").unwrap();
            writeln!(self.out, "  call void @df.preparar_isolado()").unwrap();
            self.chamar_main();
            if let Some(chamar) = &self.module.chamar_dart {
                writeln!(self.out, "  call void @dartforge_laco_de_eventos(ptr @{chamar})").unwrap();
            }
            writeln!(self.out, "  ret void\n}}\n").unwrap();
            // O runtime e o SDK moram na DLL do SDK da fonte: o `main` do
            // executável é este, e entrega a entrada ao runtime.
            writeln!(
                self.out,
                "define i32 @main() {{\n  %r = call i32 @dartforge_iniciar(ptr @dartforge_entry, ptr @dartforge_dispatch_toString)\n  ret i32 %r\n}}\n"
            )
            .unwrap();
            return;
        }
        writeln!(self.out, "define void @dartforge_entry() {{").unwrap();
        // A memória ARC, ligada antes de todo código Dart.
        if self.module.memoria_arc {
            writeln!(self.out, "  call void @dartforge_memoria_arc_v1()").unwrap();
        }
        // Exceções por tabelas: as portas pelas quais o runtime chama
        // código Dart, entregues antes de qualquer código Dart rodar.
        if self.module.excecoes_por_tabelas {
            writeln!(self.out, "  call void @dartforge_registrar_portas(ptr @df.portas, i64 {N_PORTAS})").unwrap();
        }
        // Registra classes
        for class in &self.module.classes {
            let idx = self.string_const_index(class.name.as_bytes()).unwrap_or(0);
            let len = class.name.as_bytes().len();
            writeln!(
                self.out,
                "  call void @dartforge_register_class_name(i64 {}, ptr @.str.{}, i64 {})",
                class.id, idx, len
            ).unwrap();
        }

        if self.mapas && !self.mapas_no_jit {
            self.out.push_str(Self::chamada_de_registro_dos_mapas());
        }
        if let Some(c) = self.chamada_de_registro_do_rastro() {
            self.out.push_str(&c);
            let campos = self.chamadas_de_registro_dos_campos_do_rastro();
            self.out.push_str(&campos);
        }
        // Registra grafo de subtipagem
        for (sub, sup) in &self.module.subtyping_edges {
            writeln!(
                self.out,
                "  call void @dartforge_register_subclass(i64 {sub}, i64 {sup})"
            ).unwrap();
        }

        // RTI: o universo de tipos (classes citadas e regras de supertipo).
        if let Some(iniciar) = &self.module.iniciar_rti {
            writeln!(self.out, "  {}", chamada_de_entrada(self.module.excecoes_por_tabelas, iniciar, &[])).unwrap();
        }
        self.chamar_main();
        // P6: microtarefas e timers depois do `main` (runtime, `eventos.rs`).
        if let Some(chamar) = &self.module.chamar_dart {
            writeln!(self.out, "  call void @dartforge_laco_de_eventos(ptr @{chamar})").unwrap();
        }
        writeln!(self.out, "  ret void").unwrap();
        writeln!(self.out, "}}\n").unwrap();
    }

    /// A chamada do `main` na entrada: com parâmetros, o primeiro é a lista
    /// dos argumentos da linha de comando (`dartforge_argumentos_do_main`) e
    /// o segundo, `null`.
    /// Um argumento primitivo de chamada C: o operando convertido ao tipo C
    /// (estreitos com a extensão da ABI), como `tipo atributos valor`.
    fn argumento_c(&mut self, v: u32, i: usize, op: &Operand, tc: TipoC) -> String {
        let x = match tc {
            TipoC::F32 => {
                let d = self.coagir(op, Type::F64);
                writeln!(self.out, "  %na{v}_{i} = fptrunc double {d} to float").unwrap();
                format!("%na{v}_{i}")
            }
            TipoC::F64 => self.coagir(op, Type::F64),
            TipoC::Bool => self.coagir(op, Type::I1),
            TipoC::Ptr | TipoC::Handle => {
                let n = self.coagir(op, Type::I64);
                writeln!(self.out, "  %na{v}_{i} = inttoptr i64 {n} to ptr").unwrap();
                format!("%na{v}_{i}")
            }
            TipoC::I64 | TipoC::U64 => self.coagir(op, Type::I64),
            TipoC::Void => unreachable!("argumento nativo void"),
            estreito => {
                let n = self.coagir(op, Type::I64);
                writeln!(self.out, "  %na{v}_{i} = trunc i64 {n} to {}", estreito.llvm()).unwrap();
                format!("%na{v}_{i}")
            }
        };
        format!("{} {}{x}", tc.llvm(), tc.extensao())
    }

    /// O texto `call …` de uma chamada C por `%fn<v>`. Numa variádica feita por
    /// uma função `gc` (raízes por mapas), a chamada passa por um intermediário
    /// `@df.vararg.<k>` de assinatura fixa, `noinline` e fora da estratégia de
    /// coleta: o `rewrite-statepoints-for-gc` não embrulha chamadas variádicas
    /// que devolvem valor (`gc.statepoint doesn't support wrapping non-void
    /// vararg functions yet`), e a chamada ao intermediário vira um statepoint
    /// comum, com as raízes vivas no mapa dela. O intermediário repassa os
    /// argumentos com os mesmos tipos e atributos.
    fn texto_da_chamada_c(&mut self, v: u32, ret: &str, tipo: &str, partes: &[String], variadica: bool) -> String {
        let lista = partes.join(", ");
        if !variadica || self.enraizados.is_empty() {
            return format!("call {tipo} %fn{v}({lista})");
        }
        let nome = format!("df.vararg.{}", self.variadicas.len());
        let mut params = vec!["ptr %f".to_string()];
        let mut repasse = Vec::with_capacity(partes.len());
        for (j, p) in partes.iter().enumerate() {
            let cabeca = p.rsplit_once(' ').map_or(p.as_str(), |(c, _)| c);
            params.push(format!("{cabeca} %p{j}"));
            repasse.push(format!("{cabeca} %p{j}"));
        }
        let interna = format!("call {tipo} %f({})", repasse.join(", "));
        let corpo = if ret == "void" { format!("  {interna}\n  ret void\n") } else { format!("  %r = {interna}\n  ret {ret} %r\n") };
        self.variadicas.push(format!("define internal {ret} @{nome}({}) noinline {{\n{corpo}}}\n\n", params.join(", ")));
        let mut args = vec![format!("ptr %fn{v}")];
        args.extend(partes.iter().cloned());
        format!("call {ret} @{nome}({})", args.join(", "))
    }

    /// Uma chamada C com structs/unions por valor (`llvm/abi_c.rs`): cada
    /// composto é copiado dos bytes do operando para uma temporária alinhada
    /// da pilha (liberada por `stackrestore` depois da chamada), e passado
    /// em peças, `byval` ou por ponteiro; um retorno composto vai para
    /// `destino` (direto: pelas peças gravadas numa temporária; `sret`: o
    /// próprio destino).
    fn chamada_nativa_composta(
        &mut self,
        v: u32,
        alvo: &Operand,
        args: &[(Operand, TipoNativo)],
        ret: &TipoNativo,
        destino: Option<&Operand>,
        variadica: Option<usize>,
    ) {
        use abi_c::{PassagemArg, PassagemRet};
        let conv = abi_c::Convencao::do_alvo();
        let mut regs = abi_c::Registradores::novos();
        let a = self.coagir(alvo, Type::I64);
        writeln!(self.out, "  %fn{v} = inttoptr i64 {a} to ptr").unwrap();
        writeln!(self.out, "  %pilha{v} = call ptr @llvm.stacksave.p0()").unwrap();
        // Retorno primeiro: um `sret` ocupa o primeiro registrador inteiro.
        let passagem_ret = match ret {
            TipoNativo::Composto(l) => Some((abi_c::retorno(conv, l, &mut regs), l.clone())),
            TipoNativo::Prim(_) => None,
        };
        let mut partes = Vec::with_capacity(args.len() + 1);
        let destino_ptr = destino.map(|d| {
            let d = self.coagir(d, Type::I64);
            writeln!(self.out, "  %dst{v} = inttoptr i64 {d} to ptr").unwrap();
            format!("%dst{v}")
        });
        if let (Some((PassagemRet::Sret { alinhamento }, l)), Some(d)) = (&passagem_ret, &destino_ptr) {
            partes.push(format!("ptr sret([{} x i8]) align {alinhamento} {d}", l.tamanho));
        }
        let mut tipos_fixos: Vec<String> = Vec::new();
        if matches!(&passagem_ret, Some((PassagemRet::Sret { .. }, _))) {
            tipos_fixos.push("ptr".to_string());
        }
        for (i, (op, t)) in args.iter().enumerate() {
            let variadico = variadica.is_some_and(|n| i >= n);
            match t {
                // Promoções de argumento padrão do C num argumento variádico:
                // `float` vai como `double`; inteiros menores que `int`,
                // como `int` (com o sinal do tipo).
                TipoNativo::Prim(tc) if variadico => {
                    regs.consumir_primitivo(*tc);
                    let p = match tc {
                        TipoC::F32 => self.argumento_c(v, i, op, TipoC::F64),
                        TipoC::I8 | TipoC::I16 => self.argumento_c(v, i, op, TipoC::I32),
                        TipoC::U8 | TipoC::U16 | TipoC::Bool => {
                            let n = self.coagir(op, if *tc == TipoC::Bool { Type::I1 } else { Type::I64 });
                            let ext = if *tc == TipoC::Bool { format!("zext i1 {n} to i32") } else { format!("trunc i64 {n} to i32") };
                            writeln!(self.out, "  %na{v}_{i} = {ext}").unwrap();
                            format!("i32 %na{v}_{i}")
                        }
                        _ => self.argumento_c(v, i, op, *tc),
                    };
                    partes.push(p);
                }
                TipoNativo::Prim(tc) => {
                    regs.consumir_primitivo(*tc);
                    let p = self.argumento_c(v, i, op, *tc);
                    tipos_fixos.push(p.split(' ').next().unwrap_or("i64").to_string());
                    partes.push(p);
                }
                TipoNativo::Composto(l) => {
                    // A cópia: alinhada, zerada até a palavra, com os bytes.
                    let n = self.coagir(op, Type::I64);
                    let tam = l.tamanho.div_ceil(16) * 16;
                    let al = l.alinhamento.max(16);
                    writeln!(self.out, "  %src{v}_{i} = inttoptr i64 {n} to ptr").unwrap();
                    writeln!(self.out, "  %tmp{v}_{i} = alloca [{tam} x i8], align {al}").unwrap();
                    writeln!(self.out, "  call void @llvm.memset.p0.i64(ptr %tmp{v}_{i}, i8 0, i64 {tam}, i1 false)").unwrap();
                    writeln!(self.out, "  call void @llvm.memcpy.p0.p0.i64(ptr %tmp{v}_{i}, ptr %src{v}_{i}, i64 {}, i1 false)", l.tamanho).unwrap();
                    // Na parte variádica as peças não entram no tipo da
                    // chamada (`ret (fixos, ...)`).
                    let passagem = if variadico {
                        abi_c::argumento_variadico(conv, l, &mut regs)
                    } else {
                        abi_c::argumento(conv, l, &mut regs)
                    };
                    let mut fixo = |t: String| {
                        if !variadico {
                            tipos_fixos.push(t);
                        }
                    };
                    match passagem {
                        PassagemArg::Direta(pecas) => {
                            for (k, p) in pecas.iter().enumerate() {
                                writeln!(self.out, "  %pp{v}_{i}_{k} = getelementptr i8, ptr %tmp{v}_{i}, i64 {}", p.deslocamento).unwrap();
                                writeln!(self.out, "  %pc{v}_{i}_{k} = load {}, ptr %pp{v}_{i}_{k}, align 1", p.tipo).unwrap();
                                partes.push(format!("{} {}%pc{v}_{i}_{k}", p.tipo, p.atributos));
                                fixo(p.tipo.clone());
                            }
                        }
                        PassagemArg::Byval { alinhamento } => {
                            partes.push(format!("ptr byval([{} x i8]) align {alinhamento} %tmp{v}_{i}", l.tamanho));
                            fixo("ptr".to_string());
                        }
                        PassagemArg::Indireta => {
                            partes.push(format!("ptr %tmp{v}_{i}"));
                            fixo("ptr".to_string());
                        }
                    }
                }
            }
        }
        // O tipo da chamada: numa variádica, `ret (fixos, ...)`.
        let tipo_da_chamada = |ret: &str| match variadica {
            Some(_) if tipos_fixos.is_empty() => format!("{ret} (...)"),
            Some(_) => format!("{ret} ({}, ...)", tipos_fixos.join(", ")),
            None => ret.to_string(),
        };
        let tipo_de_retorno = match (ret, &passagem_ret) {
            (TipoNativo::Prim(TipoC::Void), _) | (TipoNativo::Composto(_), Some((PassagemRet::Sret { .. }, _))) => "void".to_string(),
            (TipoNativo::Prim(tc), _) => tc.llvm().to_string(),
            (TipoNativo::Composto(_), Some((PassagemRet::Direta(pecas), _))) => abi_c::tipo_do_retorno(pecas),
            (TipoNativo::Composto(_), None) => unreachable!("retorno composto sem passagem"),
        };
        let chamada = self.texto_da_chamada_c(v, &tipo_de_retorno, &tipo_da_chamada(&tipo_de_retorno), &partes, variadica.is_some());
        match (ret, &passagem_ret) {
            (TipoNativo::Prim(TipoC::Void), _) => writeln!(self.out, "  {chamada}").unwrap(),
            (TipoNativo::Prim(tc), _) => {
                let conv_ret = match tc {
                    TipoC::I8 | TipoC::I16 | TipoC::I32 => Some(format!("sext {} %nr{v} to i64", tc.llvm())),
                    TipoC::U8 | TipoC::U16 | TipoC::U32 => Some(format!("zext {} %nr{v} to i64", tc.llvm())),
                    TipoC::F32 => Some(format!("fpext float %nr{v} to double")),
                    TipoC::Ptr | TipoC::Handle => Some(format!("ptrtoint ptr %nr{v} to i64")),
                    _ => None,
                };
                match conv_ret {
                    Some(c) => {
                        writeln!(self.out, "  %nr{v} = {chamada}").unwrap();
                        writeln!(self.out, "  %v{v} = {c}").unwrap();
                    }
                    None => writeln!(self.out, "  %v{v} = {chamada}").unwrap(),
                }
            }
            (TipoNativo::Composto(_), Some((PassagemRet::Sret { .. }, _))) => {
                writeln!(self.out, "  {chamada}").unwrap();
            }
            (TipoNativo::Composto(_), Some((PassagemRet::Direta(pecas), l))) => {
                let tipo = abi_c::tipo_do_retorno(pecas);
                let tam = l.tamanho.div_ceil(16) * 16;
                writeln!(self.out, "  %rr{v} = {chamada}").unwrap();
                writeln!(self.out, "  %rt{v} = alloca [{tam} x i8], align 16").unwrap();
                for (k, p) in pecas.iter().enumerate() {
                    let val = if pecas.len() == 1 {
                        format!("%rr{v}")
                    } else {
                        writeln!(self.out, "  %re{v}_{k} = extractvalue {tipo} %rr{v}, {k}").unwrap();
                        format!("%re{v}_{k}")
                    };
                    writeln!(self.out, "  %rp{v}_{k} = getelementptr i8, ptr %rt{v}, i64 {}", p.deslocamento).unwrap();
                    writeln!(self.out, "  store {} {val}, ptr %rp{v}_{k}, align 1", p.tipo).unwrap();
                }
                if let Some(d) = &destino_ptr {
                    writeln!(self.out, "  call void @llvm.memcpy.p0.p0.i64(ptr {d}, ptr %rt{v}, i64 {}, i1 false)", l.tamanho).unwrap();
                }
            }
            (TipoNativo::Composto(_), None) => unreachable!("retorno composto sem passagem"),
        }
        writeln!(self.out, "  call void @llvm.stackrestore.p0(ptr %pilha{v})").unwrap();
    }

    /// As entradas C dos callbacks do `dart:ffi` (`ffi_callbacks.rs` do
    /// runtime). Cada entrada tem a ABI C da assinatura e o contexto do
    /// callback no parâmetro `nest` (r10 no x86-64, x15 no AArch64), que o
    /// trampolim escrito pelo runtime carrega: no modo ouvinte copia
    /// os argumentos para a mensagem; senão converte-os para a
    /// representação Dart, chama o corpo HIR e converte o retorno — ou
    /// devolve o retorno excepcional, se a closure lançou.
    fn emit_callbacks_ffi(&mut self) {
        use abi_c::{PassagemArg, PassagemRet};
        if self.module.ffi_callbacks.is_empty() {
            return;
        }
        let conv = abi_c::Convencao::do_alvo();
        for (i, cb) in self.module.ffi_callbacks.clone().iter().enumerate() {
            writeln!(self.out, "@df.ffi.cbchave.{i} = private unnamed_addr constant [{} x i8] c\"{}\"", cb.chave.len(), cb.chave).unwrap();
            let mut regs = abi_c::Registradores::novos();
            // O retorno primeiro: um `sret` ocupa o primeiro registrador.
            let ret = match &cb.ret {
                TipoNativo::Composto(l) => Some((abi_c::retorno(conv, l, &mut regs), l.clone())),
                TipoNativo::Prim(_) => None,
            };
            let mut decls = vec!["ptr nest %ctx".to_string()];
            if let Some((PassagemRet::Sret { alinhamento }, l)) = &ret {
                decls.push(format!("ptr sret([{} x i8]) align {alinhamento} %sret", l.tamanho));
            }
            // Como cada composto chega: em peças (remontadas numa
            // temporária) ou já em memória (`byval`/ponteiro).
            let mut pecas_de: Vec<Option<(Vec<abi_c::Peca>, usize)>> = Vec::with_capacity(cb.params.len());
            for (j, t) in cb.params.iter().enumerate() {
                match t {
                    TipoNativo::Prim(tc) => {
                        regs.consumir_primitivo(*tc);
                        decls.push(format!("{} {}%a{j}", tc.llvm(), tc.extensao()));
                        pecas_de.push(None);
                    }
                    TipoNativo::Composto(l) => match abi_c::argumento(conv, l, &mut regs) {
                        PassagemArg::Direta(pecas) => {
                            for (k, p) in pecas.iter().enumerate() {
                                decls.push(format!("{} {}%a{j}_{k}", p.tipo, p.atributos));
                            }
                            pecas_de.push(Some((pecas, l.tamanho)));
                        }
                        PassagemArg::Byval { alinhamento } => {
                            decls.push(format!("ptr byval([{} x i8]) align {alinhamento} %a{j}", l.tamanho));
                            pecas_de.push(None);
                        }
                        PassagemArg::Indireta => {
                            decls.push(format!("ptr %a{j}"));
                            pecas_de.push(None);
                        }
                    },
                }
            }
            let (tipo_ret, ret_ext) = match (&cb.ret, &ret) {
                (TipoNativo::Prim(tc), _) => (
                    tc.llvm().to_string(),
                    match tc {
                        TipoC::I8 | TipoC::I16 => "signext ",
                        TipoC::U8 | TipoC::U16 | TipoC::Bool => "zeroext ",
                        _ => "",
                    },
                ),
                (_, Some((PassagemRet::Direta(pecas), _))) => (abi_c::tipo_do_retorno(pecas), ""),
                _ => ("void".to_string(), ""),
            };
            let r = tipo_ret.as_str();
            // Exceções por tabelas: a entrada pousa o desenrolamento do corpo
            // (abaixo) — código C não é atravessado. A sabotagem
            // `callback_sem_pouso` (o D9 do §7.3) tira o pouso.
            let por_tabelas = self.module.excecoes_por_tabelas && !crate::alvo::sabotagem("callback_sem_pouso");
            let personalidade = if por_tabelas { " personality ptr @dartforge_personalidade" } else { "" };
            writeln!(self.out, "define internal {ret_ext}{r} @df.ffi.cbentrada.{i}({}){personalidade} {{", decls.join(", "))
                .unwrap();
            let n = cb.params.len().max(1);
            writeln!(self.out, "entrada:\n  %saida = alloca i64\n  %buf = alloca [{n} x i64]").unwrap();
            if let Some((_, l)) = &ret {
                writeln!(self.out, "  %rt = alloca [{} x i8], align 16", l.tamanho.div_ceil(16) * 16).unwrap();
            }
            // O endereço de cada composto (`%end{j}`, i64).
            for (j, t) in cb.params.iter().enumerate() {
                let TipoNativo::Composto(_) = t else { continue };
                match &pecas_de[j] {
                    Some((pecas, tamanho)) => {
                        writeln!(self.out, "  %m{j} = alloca [{} x i8], align 16", tamanho.div_ceil(16) * 16).unwrap();
                        for (k, p) in pecas.iter().enumerate() {
                            writeln!(self.out, "  %mp{j}_{k} = getelementptr i8, ptr %m{j}, i64 {}", p.deslocamento).unwrap();
                            writeln!(self.out, "  store {} %a{j}_{k}, ptr %mp{j}_{k}, align 1", p.tipo).unwrap();
                        }
                        writeln!(self.out, "  %end{j} = ptrtoint ptr %m{j} to i64").unwrap();
                    }
                    None => writeln!(self.out, "  %end{j} = ptrtoint ptr %a{j} to i64").unwrap(),
                }
            }
            writeln!(self.out, "  %modo = call i64 @dartforge_ffi_callback_entrar(ptr %ctx)").unwrap();
            writeln!(self.out, "  %e_ouvinte = icmp ne i64 %modo, 0\n  br i1 %e_ouvinte, label %ouvinte, label %local").unwrap();
            // Ouvinte: os bits de cada argumento na mensagem (de um
            // composto, o endereço: o runtime copia os bytes).
            writeln!(self.out, "ouvinte:").unwrap();
            for (j, t) in cb.params.iter().enumerate() {
                let bits = match t {
                    TipoNativo::Composto(_) => format!("add i64 %end{j}, 0"),
                    TipoNativo::Prim(tc) => match tc {
                        TipoC::I8 | TipoC::I16 | TipoC::I32 => format!("sext {} %a{j} to i64", tc.llvm()),
                        TipoC::U8 | TipoC::U16 | TipoC::U32 | TipoC::Bool => format!("zext {} %a{j} to i64", tc.llvm()),
                        TipoC::I64 | TipoC::U64 => format!("add i64 %a{j}, 0"),
                        TipoC::F32 => {
                            writeln!(self.out, "  %of{j} = fpext float %a{j} to double").unwrap();
                            format!("bitcast double %of{j} to i64")
                        }
                        TipoC::F64 => format!("bitcast double %a{j} to i64"),
                        TipoC::Ptr | TipoC::Handle => format!("ptrtoint ptr %a{j} to i64"),
                        TipoC::Void => unreachable!("parâmetro nativo void"),
                    },
                };
                writeln!(self.out, "  %o{j} = {bits}\n  %g{j} = getelementptr [{n} x i64], ptr %buf, i64 0, i64 {j}\n  store i64 %o{j}, ptr %g{j}").unwrap();
            }
            writeln!(self.out, "  call void @dartforge_ffi_callback_postar(ptr %ctx, ptr %buf, i64 {})", cb.params.len()).unwrap();
            match &cb.ret {
                TipoNativo::Prim(TipoC::Void) => writeln!(self.out, "  ret void").unwrap(),
                TipoNativo::Prim(TipoC::Ptr | TipoC::Handle) => writeln!(self.out, "  ret ptr null").unwrap(),
                TipoNativo::Prim(TipoC::F32 | TipoC::F64) => writeln!(self.out, "  ret {r} 0.0").unwrap(),
                TipoNativo::Prim(_) => writeln!(self.out, "  ret {r} 0").unwrap(),
                TipoNativo::Composto(_) if r == "void" => writeln!(self.out, "  ret void").unwrap(),
                TipoNativo::Composto(_) => writeln!(self.out, "  ret {r} zeroinitializer").unwrap(),
            }
            // Local: a chamada ao corpo HIR na representação Dart.
            writeln!(self.out, "local:\n  %c = ptrtoint ptr %ctx to i64").unwrap();
            let mut args = vec!["i64 %c".to_string()];
            for (j, t) in cb.params.iter().enumerate() {
                let tc = match t {
                    TipoNativo::Composto(_) => {
                        args.push(format!("i64 %end{j}"));
                        continue;
                    }
                    TipoNativo::Prim(tc) => *tc,
                };
                let (conv_arg, ty) = match tc {
                    TipoC::I8 | TipoC::I16 | TipoC::I32 => (Some(format!("sext {} %a{j} to i64", tc.llvm())), "i64"),
                    TipoC::U8 | TipoC::U16 | TipoC::U32 => (Some(format!("zext {} %a{j} to i64", tc.llvm())), "i64"),
                    TipoC::F32 => (Some(format!("fpext float %a{j} to double")), "double"),
                    TipoC::Ptr | TipoC::Handle => (Some(format!("ptrtoint ptr %a{j} to i64")), "i64"),
                    TipoC::I64 | TipoC::U64 => (None, "i64"),
                    TipoC::F64 => (None, "double"),
                    TipoC::Bool => (None, "i1"),
                    TipoC::Void => unreachable!("parâmetro nativo void"),
                };
                match conv_arg {
                    Some(c) => {
                        writeln!(self.out, "  %d{j} = {c}").unwrap();
                        args.push(format!("{ty} %d{j}"));
                    }
                    None => args.push(format!("{ty} %a{j}")),
                }
            }
            let hr = cb.ret.tipo_hir().llvm_ir();
            let vazio = matches!(cb.ret, TipoNativo::Prim(TipoC::Void));
            if por_tabelas {
                // O corpo que lança desenrola: o pouso restaura o topo da
                // pilha-sombra e segue com o valor padrão e a exceção
                // pendente, como o corpo devolvia no modelo de conferência.
                self.out.push_str(
                    "  %tctx = call ptr @dartforge_contexto()\n  %ttp = getelementptr inbounds i8, ptr %tctx, i64 8\n  %ttopo = load ptr, ptr %ttp, align 8\n",
                );
                let pouso = "tlp:\n  %tx = landingpad { ptr, i32 } catch ptr null\n  store ptr %ttopo, ptr %ttp, align 8\n  br label %tcont\n";
                if vazio {
                    writeln!(self.out, "  invoke void @{}({}) to label %tcont unwind label %tlp", cb.corpo, args.join(", ")).unwrap();
                    self.out.push_str(pouso);
                    self.out.push_str("tcont:\n");
                } else {
                    let zero = match hr {
                        "double" => "0.0",
                        "i1" => "false",
                        _ => "0",
                    };
                    writeln!(self.out, "  %r.ok = invoke {hr} @{}({}) to label %tok unwind label %tlp", cb.corpo, args.join(", ")).unwrap();
                    self.out.push_str(pouso);
                    self.out.push_str("tok:\n  br label %tcont\ntcont:\n");
                    writeln!(self.out, "  %r = phi {hr} [ %r.ok, %tok ], [ {zero}, %tlp ]").unwrap();
                }
            } else if vazio {
                writeln!(self.out, "  call void @{}({})", cb.corpo, args.join(", ")).unwrap();
            } else {
                writeln!(self.out, "  %r = call {hr} @{}({})", cb.corpo, args.join(", ")).unwrap();
            }
            // Uma struct devolvida: os bytes copiados já (antes de qualquer
            // alocação); com exceção o corpo devolve 0 e o retorno é zerado.
            if let Some((_, l)) = &ret {
                let tam = l.tamanho.div_ceil(16) * 16;
                writeln!(self.out, "  call void @llvm.memset.p0.i64(ptr %rt, i8 0, i64 {tam}, i1 false)").unwrap();
                writeln!(self.out, "  call void @dartforge_ffi_copiar_composto(ptr %rt, i64 %r, i64 {})", l.tamanho).unwrap();
            }
            writeln!(self.out, "  %x = call i8 @dartforge_ffi_callback_sair(ptr %ctx, ptr %saida)").unwrap();
            match (&cb.ret, &ret) {
                (TipoNativo::Prim(TipoC::Void), _) => {
                    writeln!(self.out, "  ret void\n}}\n").unwrap();
                    continue;
                }
                (_, Some((PassagemRet::Sret { .. }, l))) => {
                    writeln!(self.out, "  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %rt, i64 {}, i1 false)", l.tamanho).unwrap();
                    writeln!(self.out, "  ret void\n}}\n").unwrap();
                    continue;
                }
                (_, Some((PassagemRet::Direta(pecas), _))) => {
                    let mut atual = "undef".to_string();
                    for (k, p) in pecas.iter().enumerate() {
                        writeln!(self.out, "  %rp{k} = getelementptr i8, ptr %rt, i64 {}", p.deslocamento).unwrap();
                        writeln!(self.out, "  %rv{k} = load {}, ptr %rp{k}, align 1", p.tipo).unwrap();
                        if pecas.len() == 1 {
                            atual = format!("%rv{k}");
                        } else {
                            writeln!(self.out, "  %ra{k} = insertvalue {r} {atual}, {} %rv{k}, {k}", p.tipo).unwrap();
                            atual = format!("%ra{k}");
                        }
                    }
                    writeln!(self.out, "  ret {r} {atual}\n}}\n").unwrap();
                    continue;
                }
                _ => {}
            }
            let TipoNativo::Prim(tr) = cb.ret else { unreachable!("retorno composto sem passagem") };
            writeln!(self.out, "  %e_excecao = icmp ne i8 %x, 0\n  br i1 %e_excecao, label %excecao, label %normal").unwrap();
            // O retorno excepcional (bits do tipo C) e o normal (Dart → C).
            writeln!(self.out, "excecao:\n  %e = load i64, ptr %saida").unwrap();
            let exc = match tr {
                TipoC::I64 | TipoC::U64 => "add i64 %e, 0".to_string(),
                TipoC::F64 => "bitcast i64 %e to double".to_string(),
                TipoC::F32 => {
                    writeln!(self.out, "  %ed = bitcast i64 %e to double").unwrap();
                    "fptrunc double %ed to float".to_string()
                }
                TipoC::Ptr | TipoC::Handle => "inttoptr i64 %e to ptr".to_string(),
                estreito => format!("trunc i64 %e to {}", estreito.llvm()),
            };
            writeln!(self.out, "  %ev = {exc}\n  ret {r} %ev").unwrap();
            let normal = match tr {
                TipoC::I64 | TipoC::U64 | TipoC::F64 | TipoC::Bool => None,
                TipoC::F32 => Some("fptrunc double %r to float".to_string()),
                TipoC::Ptr | TipoC::Handle => Some("inttoptr i64 %r to ptr".to_string()),
                estreito => Some(format!("trunc i64 %r to {}", estreito.llvm())),
            };
            match normal {
                Some(c) => writeln!(self.out, "normal:\n  %nv = {c}\n  ret {r} %nv\n}}\n").unwrap(),
                None => writeln!(self.out, "normal:\n  ret {r} %r\n}}\n").unwrap(),
            }
        }
    }

    fn chamar_main(&mut self) {
        let Some(entry) = self.module.entry_symbol.clone() else { return };
        let por_tabelas = self.module.excecoes_por_tabelas;
        let n = self.module.entry_params.min(2);
        if n == 0 {
            writeln!(self.out, "  {}", chamada_de_entrada(por_tabelas, &entry, &[])).unwrap();
            return;
        }
        writeln!(self.out, "  %df.args = call i64 @dartforge_argumentos_do_main()").unwrap();
        let args: &[&str] = if n == 1 { &["i64 %df.args"] } else { &["i64 %df.args", "i64 0"] };
        writeln!(self.out, "  {}", chamada_de_entrada(por_tabelas, &entry, args)).unwrap();
    }

    /// Exceções por tabelas: as portas pelas quais o runtime chama código
    /// Dart (`runtime/src/excecoes_tabelas.rs`), uma por assinatura —
    /// `@df.porta.<v|r><n>(função, n palavras)`, `v` sem valor e `r`
    /// devolvendo uma palavra — e a tabela `@df.portas`, no índice
    /// `2n + (1 se devolve)`, que a entrada do programa registra. A porta
    /// chama a função com `invoke`; um desenrolamento pousa nela, que
    /// restaura o topo da pilha-sombra e volta com a exceção pendente — o
    /// que o runtime espera de uma função Dart que lançou. Assim nenhum
    /// quadro do runtime fica no caminho de um desenrolamento.
    fn emitir_portas(&mut self) {
        let mut tabela = Vec::with_capacity(N_PORTAS);
        for n in 0..N_PORTAS / 2 {
            for retorna in [false, true] {
                let nome = format!("df.porta.{}{n}", if retorna { 'r' } else { 'v' });
                let params: String = (0..n).map(|i| format!(", i64 %a{i}")).collect();
                let args: Vec<String> = (0..n).map(|i| format!("i64 %a{i}")).collect();
                let (tipo, resultado, volta, padrao) =
                    if retorna { ("i64", "%r = ", "ret i64 %r", "ret i64 0") } else { ("void", "", "ret void", "ret void") };
                writeln!(self.out, "define internal {tipo} @{nome}(ptr %f{params}) personality ptr @dartforge_personalidade {{")
                    .unwrap();
                self.out.push_str(
                    "entrada:\n  %ctx = call ptr @dartforge_contexto()\n  %tp = getelementptr inbounds i8, ptr %ctx, i64 8\n  %topo = load ptr, ptr %tp, align 8\n",
                );
                writeln!(self.out, "  {resultado}invoke {tipo} %f({}) to label %volta unwind label %pouso", args.join(", ")).unwrap();
                writeln!(
                    self.out,
                    "volta:\n  {volta}\npouso:\n  %lp = landingpad {{ ptr, i32 }} catch ptr null\n  store ptr %topo, ptr %tp, align 8\n  {padrao}\n}}"
                )
                .unwrap();
                tabela.push(format!("ptr @{nome}"));
            }
        }
        writeln!(self.out, "@df.portas = private unnamed_addr constant [{N_PORTAS} x ptr] [{}]\n", tabela.join(", ")).unwrap();
    }

    /// Tipo do valor que o emissor de fato imprime para uma instrucao.
    ///
    /// Nao basta acreditar no tipo registrado na HIR: varios arms imprimem um
    /// tipo fixo (todo Add sai como i64, todo ICmp como i1) e um registro
    /// divergente faria a coercao trabalhar com a informacao errada. O tipo
    /// registrado so vale onde o emissor o usa (Load, Phi, Alloca e as
    /// instrucoes ainda nao expandidas).
    pub(crate) fn tipo_do_resultado(inst: &Instruction, registrado: Type) -> Type {
        match inst {
            Instruction::Const(Constant::Bool(_)) => Type::I1,
            Instruction::Const(Constant::Double(_)) => Type::F64,
            Instruction::Const(Constant::Int(_)) => Type::I64,
            Instruction::Const(_) => Type::Ref,
            Instruction::Add(..)
            | Instruction::Sub(..)
            | Instruction::Mul(..)
            | Instruction::SDiv(..)
            | Instruction::SRem(..)
            | Instruction::Shl(..)
            | Instruction::AShr(..)
            | Instruction::And(..)
            | Instruction::Or(..)
            | Instruction::Xor(..)
            | Instruction::Neg(..)
            | Instruction::Not(..)
            | Instruction::LShr(..)
            | Instruction::DoubleToInt(..) => Type::I64,
            Instruction::AllocObject { .. }
            | Instruction::AllocList { .. }
            | Instruction::AllocRecord { .. }
            | Instruction::Box { .. }
            | Instruction::AllocCell { .. }
            | Instruction::AllocEnv { .. }
            | Instruction::JuntarTextos { .. }
            | Instruction::AllocClosure { .. }
            | Instruction::AllocClosureTipada { .. }
            | Instruction::TearOff { .. }
            | Instruction::CallClosure { .. }
            | Instruction::CallSeletor { .. }
            | Instruction::CallSeletorRepasse { .. }
            | Instruction::CallClosureRepasse { .. } => Type::Ref,
            Instruction::ChamadaNativa { ret, .. } => ret.tipo_hir(),
            Instruction::ChamadaTipada { ret, .. } => *ret,
            Instruction::CargaNativa { tipo, .. } => tipo.tipo_hir(),
            Instruction::GravacaoNativa { .. } => Type::Void,
            Instruction::ChamadaNativaComposta { ret, destino, .. } => {
                if destino.is_some() { Type::Void } else { ret.tipo_hir() }
            }
            Instruction::CellSet { .. } => Type::Void,
            Instruction::ConstArray(_) | Instruction::TabelaDeFuncoes(_) => Type::Ptr,
            Instruction::Unbox { to, .. } => *to,
            Instruction::Alloca(_) => Type::Ptr,
            Instruction::GetField { .. } => Type::I64,
            Instruction::FAdd(..)
            | Instruction::FSub(..)
            | Instruction::FMul(..)
            | Instruction::FDiv(..)
            | Instruction::FNeg(..)
            | Instruction::IntToDouble(..) => Type::F64,
            Instruction::ICmp(..) | Instruction::FCmp(..) | Instruction::LNot(..) => Type::I1,
            Instruction::ZExt { to, .. } | Instruction::Trunc { to, .. } | Instruction::Bitcast { to, .. } => *to,
            Instruction::LoadGlobal { ty, .. } => *ty,
            Instruction::CallStatic { ret_ty, .. } | Instruction::CallRuntime { ret_ty, .. } => *ret_ty,
            Instruction::Load { ty, .. } => *ty,
            Instruction::Phi { ty, .. } => *ty,
            _ => registrado,
        }
    }


    /// A conferência da pilha no prólogo (o `stack_overflow_check` da VM,
    /// `runtime/vm/compiler/backend/il.h:9693`, `CheckStackOverflowInstr`): o
    /// endereço de um `alloca` do quadro abaixo de
    /// `Contexto::limite_da_pilha` lança `StackOverflowError`
    /// (`dartforge_estouro_de_pilha`) e a função volta com a exceção
    /// pendente, antes de encadear o quadro de raízes. Só nas funções que
    /// chamam (as que têm `%ctx`): uma folha não aprofunda a pilha.
    fn emitir_conferencia_da_pilha(&mut self, func: &Function) {
        let por_tabelas = self.tab.is_some();
        let o = &mut self.out;
        writeln!(o, "  %pilhaq = alloca i8, align 1").unwrap();
        writeln!(o, "  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 {}", layout::contexto::LIMITE_DA_PILHA).unwrap();
        writeln!(o, "  %pilhal = load ptr, ptr %pilhalp, align 8").unwrap();
        writeln!(o, "  %pilhab = icmp ult ptr %pilhaq, %pilhal").unwrap();
        writeln!(o, "  %pilhax = call i1 @llvm.expect.i1(i1 %pilhab, i1 false)").unwrap();
        writeln!(o, "  br i1 %pilhax, label %pilha.estouro, label %pilha.ok").unwrap();
        writeln!(o, "pilha.estouro:").unwrap();
        writeln!(o, "  call void @dartforge_estouro_de_pilha()").unwrap();
        if self.rastro.is_some() {
            writeln!(o, "  call void @dartforge_rastro_saida()").unwrap();
        }
        // Exceções por tabelas: o `StackOverflowError` desenrola daqui.
        if por_tabelas {
            writeln!(o, "  call void @df.lancar()").unwrap();
            writeln!(o, "  unreachable").unwrap();
            writeln!(o, "pilha.ok:").unwrap();
            self.rotulo_atual = "pilha.ok".to_string();
            self.rotulos_de_saida.entry(0).or_insert_with(|| "pilha.ok".to_string());
            return;
        }
        match func.return_ty {
            Type::Void => writeln!(o, "  ret void").unwrap(),
            Type::F64 => writeln!(o, "  ret double 0.0").unwrap(),
            Type::I1 => writeln!(o, "  ret i1 false").unwrap(),
            Type::Ptr => writeln!(o, "  ret ptr null").unwrap(),
            Type::I64 | Type::Ref => writeln!(o, "  ret i64 0").unwrap(),
            Type::I8 => writeln!(o, "  ret i8 0").unwrap(),
            t => writeln!(o, "  ret {} zeroinitializer", t.llvm_ir()).unwrap(),
        }
        writeln!(o, "pilha.ok:").unwrap();
        self.rotulo_atual = "pilha.ok".to_string();
        self.rotulos_de_saida.entry(0).or_insert_with(|| "pilha.ok".to_string());
    }

    /// O corpo do objeto `so` (`%fcb{v}`, o cabeçalho) e os campos dele
    /// (`%fp{v}`), em linha: um objeto do espaço de objetos (`h & 3 == 2` e
    /// `h > 0`; `crates/runtime/src/heap.rs`, `Cabecalho`) tem o cabeçalho em
    /// `h - 2`; qualquer outra coisa — null, `Smi`, valor do runtime — usa o
    /// `OBJETO_VAZIO` (o ponteiro em `Contexto::vazios`, deslocamento 40 do
    /// contexto), campos zerados, como `dartforge_object_campos` dava. Com o
    /// bit `FORA` nas `flags`, o corpo é o endereço guardado no primeiro
    /// campo (o objeto que mudou de número de campos): uma seleção, sem
    /// desvio. Sem chamada.
    fn emitir_endereco_dos_campos(&mut self, v: u32, so: &str) {
        let o = &mut self.out;
        if self.campos_por_chamada {
            writeln!(o, "  %fcb{v} = call ptr @df.corpo(i64 {so}, ptr %ctx)").unwrap();
            writeln!(o, "  %fp{v} = getelementptr inbounds i8, ptr %fcb{v}, i64 16").unwrap();
            return;
        }
        // O bit de sinal entra na máscara: negativo nunca é objeto.
        writeln!(o, "  %fxk{v} = and i64 {so}, {MASCARA_DE_OBJETO}").unwrap();
        writeln!(o, "  %fxo{v} = icmp eq i64 %fxk{v}, 2").unwrap();
        writeln!(o, "  %fxa{v} = add i64 {so}, -2").unwrap();
        writeln!(o, "  %fxq{v} = inttoptr i64 %fxa{v} to ptr").unwrap();
        writeln!(o, "  %fxvp{v} = getelementptr inbounds i8, ptr %ctx, i64 40").unwrap();
        writeln!(o, "  %fxv{v} = load ptr, ptr %fxvp{v}, align 8").unwrap();
        writeln!(o, "  %fxh{v} = select i1 %fxo{v}, ptr %fxq{v}, ptr %fxv{v}").unwrap();
        writeln!(o, "  %fxfp{v} = getelementptr inbounds i8, ptr %fxh{v}, i64 1").unwrap();
        writeln!(o, "  %fxf{v} = load i8, ptr %fxfp{v}, align 1").unwrap();
        writeln!(o, "  %fxfb{v} = and i8 %fxf{v}, 1").unwrap();
        writeln!(o, "  %fxfo{v} = icmp ne i8 %fxfb{v}, 0").unwrap();
        writeln!(o, "  %fxcp{v} = getelementptr inbounds i8, ptr %fxh{v}, i64 16").unwrap();
        writeln!(o, "  %fxc{v} = load ptr, ptr %fxcp{v}, align 8").unwrap();
        writeln!(o, "  %fcb{v} = select i1 %fxfo{v}, ptr %fxc{v}, ptr %fxh{v}").unwrap();
        writeln!(o, "  %fp{v} = getelementptr inbounds i8, ptr %fcb{v}, i64 16").unwrap();
    }

    /// Grava `sv` no campo `idx` do corpo `%fcb{v}` e acende ou apaga o bit
    /// dele no mapa de referências (`is_ref`: `0`, `1` ou um `i8` só
    /// conhecido em execução) — os 32 primeiros no cabeçalho, os demais nas
    /// palavras depois dos campos (achadas pelo `n` do corpo).
    fn emitir_gravacao_de_campo(&mut self, v: u32, sufixo: &str, idx: usize, sv: &str, is_ref: &str) {
        let o = &mut self.out;
        let t = format!("{v}{sufixo}");
        writeln!(o, "  %fg{t} = getelementptr inbounds i64, ptr %fp{v}, i64 {idx}").unwrap();
        writeln!(o, "  store i64 {sv}, ptr %fg{t}, align 8").unwrap();
        let (palavra, largura, bit) = if idx < 32 {
            writeln!(o, "  %fmp{t} = getelementptr inbounds i8, ptr %fcb{v}, i64 8").unwrap();
            (format!("%fmp{t}"), "i32", idx)
        } else {
            writeln!(o, "  %fnp{t} = getelementptr inbounds i8, ptr %fcb{v}, i64 2").unwrap();
            writeln!(o, "  %fn{t} = load i16, ptr %fnp{t}, align 2").unwrap();
            writeln!(o, "  %fnz{t} = zext i16 %fn{t} to i64").unwrap();
            writeln!(o, "  %fnw{t} = add i64 %fnz{t}, {}", (idx - 32) / 64).unwrap();
            writeln!(o, "  %fmp{t} = getelementptr inbounds i64, ptr %fp{v}, i64 %fnw{t}").unwrap();
            (format!("%fmp{t}"), "i64", (idx - 32) % 64)
        };
        // As constantes na largura da palavra, com sinal (o texto do IR).
        let (limpa, acende) = if largura == "i32" {
            (i64::from(!(1u32 << bit) as i32), i64::from((1u32 << bit) as i32))
        } else {
            (!(1u64 << bit) as i64, (1u64 << bit) as i64)
        };
        writeln!(o, "  %fm{t} = load {largura}, ptr {palavra}, align 4").unwrap();
        writeln!(o, "  %fmc{t} = and {largura} %fm{t}, {limpa}").unwrap();
        match is_ref {
            "0" => writeln!(o, "  store {largura} %fmc{t}, ptr {palavra}, align 4").unwrap(),
            "1" => {
                writeln!(o, "  %fms{t} = or {largura} %fm{t}, {acende}").unwrap();
                writeln!(o, "  store {largura} %fms{t}, ptr {palavra}, align 4").unwrap();
            }
            r => {
                writeln!(o, "  %fmz{t} = zext i8 {r} to {largura}").unwrap();
                writeln!(o, "  %fmb{t} = shl {largura} %fmz{t}, {bit}").unwrap();
                writeln!(o, "  %fms{t} = or {largura} %fmc{t}, %fmb{t}").unwrap();
                writeln!(o, "  store {largura} %fms{t}, ptr {palavra}, align 4").unwrap();
            }
        }
    }

    /// `dartforge_object_new(c, n)` com classe e número de campos
    /// constantes e `n ≤ TLAB_N`: a alocação sai em linha
    /// ([`LlvmEmitter::emitir_alocacao_em_linha`]). Devolve `(c, n)`.
    fn alocacao_em_linha(inst: &Instruction) -> Option<(i64, i64)> {
        let Instruction::CallRuntime { name, args, .. } = inst else { return None };
        if name != "dartforge_object_new" || args.len() != 2 {
            return None;
        }
        match (&args[0].0, &args[1].0) {
            (Operand::Constant(Constant::Int(c)), Operand::Constant(Constant::Int(n))) if (0..=TLAB_N).contains(n) && (0..=i64::from(i32::MAX)).contains(c) => {
                Some((*c, *n))
            }
            _ => None,
        }
    }

    /// A alocação em linha de um objeto de classe `c` e `n` campos, como o
    /// `TryAllocateObject` da VM (`stub_code_compiler.cc`): avança o cursor
    /// da faixa da TLAB de `n` campos do isolado (`Contexto::tlab`: cursor
    /// em `64 + 16n`, fim em `72 + 16n`; os blocos já foram contados como
    /// alocados e têm cabeçalho e campos zerados, menos o encadeamento no
    /// primeiro campo), grava o cabeçalho (jovem, `n`, a classe) e devolve o
    /// handle (`bloco + 2`). Faixa esgotada — ou,
    /// para a classe que registra a tabela de métodos na primeira alocação
    /// (`dartforge_object_new_t`), classe ainda não registrada — vai ao
    /// runtime, que coleta se preciso e reabastece. O bloco da HIR termina
    /// no rótulo `ao{v}.fim` ([`LlvmEmitter::rotulos_de_saida`]).
    fn emitir_alocacao_em_linha(&mut self, v: u32, c: i64, n: i64) {
        let tabela = self.module.funcoes_de_tabela.get(&(c as u32)).cloned();
        if let Some(f) = &tabela {
            self.anotar_externo(f, Type::Ptr, &[]);
        }
        // A TLAB é por palavras do corpo: `n ≤ TLAB_N` campos ocupam
        // `max(n, 1)` palavras (sem extensão do mapa).
        let w = layout::palavras_de_instancia(n as usize);
        let tamanho = layout::bytes_do_bloco(w);
        if self.alocacao_fora_de_linha {
            // Produção do programa grande: a mesma conta, numa chamada ao
            // `@df.nova_instancia` (docs/NATIVO-PRODUCAO-GRANDE.md §3).
            let cabecalho = layout::palavra_do_cabecalho(layout::estado::JOVEM, layout::flags::INSTANCIA, n as usize, c as i32);
            let t = tabela.as_ref().map_or("null".to_string(), |f| format!("@{f}"));
            writeln!(
                self.out,
                "  %v{v} = call i64 @df.nova_instancia(ptr %ctx, i64 {}, i64 {tamanho}, i64 {}, i64 {c}, i64 {n}, ptr {t})",
                layout::contexto::tlab_cursor(w),
                cabecalho as i64
            )
            .unwrap();
            return;
        }
        let o = &mut self.out;
        writeln!(o, "  %ta{v} = getelementptr inbounds i8, ptr %ctx, i64 {}", layout::contexto::tlab_cursor(w)).unwrap();
        writeln!(o, "  %tb{v} = load ptr, ptr %ta{v}, align 8").unwrap();
        writeln!(o, "  %tfa{v} = getelementptr inbounds i8, ptr %ctx, i64 {}", layout::contexto::tlab_fim(w)).unwrap();
        writeln!(o, "  %tf{v} = load ptr, ptr %tfa{v}, align 8").unwrap();
        writeln!(o, "  %tnx{v} = getelementptr i8, ptr %tb{v}, i64 {tamanho}").unwrap();
        writeln!(o, "  %tz{v} = icmp ugt ptr %tnx{v}, %tf{v}").unwrap();
        if tabela.is_some() {
            writeln!(o, "  %tnp{v} = getelementptr inbounds i8, ptr %ctx, i64 56").unwrap();
            writeln!(o, "  %tn{v} = load i64, ptr %tnp{v}, align 8").unwrap();
            writeln!(o, "  %tk{v} = icmp ule i64 %tn{v}, {c}").unwrap();
            writeln!(o, "  %tq{v} = or i1 %tz{v}, %tk{v}").unwrap();
            writeln!(o, "  %tx{v} = call i1 @llvm.expect.i1(i1 %tq{v}, i1 false)").unwrap();
            writeln!(o, "  br i1 %tx{v}, label %ao{v}.lento, label %ao{v}.reg").unwrap();
            writeln!(o, "ao{v}.reg:").unwrap();
            writeln!(o, "  %trp{v} = getelementptr inbounds i8, ptr %ctx, i64 48").unwrap();
            writeln!(o, "  %tr{v} = load ptr, ptr %trp{v}, align 8").unwrap();
            writeln!(o, "  %trb{v} = getelementptr inbounds i8, ptr %tr{v}, i64 {c}").unwrap();
            writeln!(o, "  %trv{v} = load i8, ptr %trb{v}, align 1").unwrap();
            writeln!(o, "  %trz{v} = icmp eq i8 %trv{v}, 0").unwrap();
            writeln!(o, "  %try{v} = call i1 @llvm.expect.i1(i1 %trz{v}, i1 false)").unwrap();
            writeln!(o, "  br i1 %try{v}, label %ao{v}.lento, label %ao{v}.rapido").unwrap();
        } else {
            writeln!(o, "  %tx{v} = call i1 @llvm.expect.i1(i1 %tz{v}, i1 false)").unwrap();
            writeln!(o, "  br i1 %tx{v}, label %ao{v}.lento, label %ao{v}.rapido").unwrap();
        }
        // O bloco livre tem o cabeçalho zerado (menos o número de campos),
        // os campos zerados e o próximo da lista no primeiro campo: avança-se
        // o cursor, zera-se o primeiro campo e grava-se a palavra do
        // cabeçalho (estado jovem, sem flags, `n`, a classe).
        let cabecalho = layout::palavra_do_cabecalho(layout::estado::JOVEM, layout::flags::INSTANCIA, n as usize, c as i32);
        writeln!(o, "ao{v}.rapido:").unwrap();
        writeln!(o, "  store ptr %tnx{v}, ptr %ta{v}, align 8").unwrap();
        writeln!(o, "  %tpp{v} = getelementptr inbounds i8, ptr %tb{v}, i64 16").unwrap();
        writeln!(o, "  store i64 0, ptr %tpp{v}, align 8").unwrap();
        writeln!(o, "  store i64 {}, ptr %tb{v}, align 8", cabecalho as i64).unwrap();
        writeln!(o, "  %thb{v} = ptrtoint ptr %tb{v} to i64").unwrap();
        writeln!(o, "  %th{v} = add i64 %thb{v}, 2").unwrap();
        writeln!(o, "  br label %ao{v}.fim").unwrap();
        writeln!(o, "ao{v}.lento:").unwrap();
        match &tabela {
            Some(f) => writeln!(o, "  %tl{v} = call i64 @dartforge_object_new_t(i64 {c}, i64 {n}, ptr @{f})").unwrap(),
            None => writeln!(o, "  %tl{v} = call i64 @dartforge_object_new(i64 {c}, i64 {n})").unwrap(),
        }
        writeln!(o, "  br label %ao{v}.fim").unwrap();
        writeln!(o, "ao{v}.fim:").unwrap();
        writeln!(o, "  %v{v} = phi i64 [ %th{v}, %ao{v}.rapido ], [ %tl{v}, %ao{v}.lento ]").unwrap();
        self.rotulo_atual = format!("ao{v}.fim");
    }

    /// A gravação em linha de campo que pode pôr um `Ref` num objeto: leva a
    /// barreira de escrita ([`LlvmEmitter::emitir_barreira`]). Uma constante
    /// escalar ou null não precisa.
    fn barreira_em_linha(&self, inst: &Instruction) -> bool {
        // No ARC a gravação vai ao runtime, com a barreira dentro.
        if self.module.memoria_arc {
            return false;
        }
        let pode_ser_ref = |op: &Operand| {
            !matches!(op, Operand::Constant(Constant::Null | Constant::Int(_) | Constant::Bool(_) | Constant::Double(_)))
        };
        match inst {
            Instruction::SetField { index, value, .. } => {
                *index < CAMPOS_EM_LINHA && self.tipo_de(value) == Type::Ref && pode_ser_ref(value)
            }
            Instruction::CallRuntime { name, args, .. }
                if name == "dartforge_object_set"
                    && args.len() == 4
                    && matches!(args.get(1), Some((Operand::Constant(Constant::Int(i)), _)) if (0..CAMPOS_EM_LINHA as i64).contains(i)) =>
            {
                let escalar = matches!(&args[3].0, Operand::Constant(Constant::Int(0)) | Operand::Constant(Constant::Bool(false)));
                !escalar && pode_ser_ref(&args[2].0)
            }
            _ => false,
        }
    }

    /// A barreira de escrita depois de gravar o `Ref` `sv` no objeto `so`
    /// (`@df.barreira`, docs/NATIVO-ESPACO-UNIFICADO.md §2.7): o objeto velho
    /// que recebe um filho jovem vai para os lembrados da próxima coleta menor —
    /// o *store buffer* da barreira da VM. `dinamico`: o `is_ref` só conhecido em
    /// execução (0/1, `i8`). O bloco da HIR passa a terminar em `wb{v}.fim`.
    fn emitir_barreira(&mut self, v: u32, so: &str, sv: &str, dinamico: Option<&str>) {
        let o = &mut self.out;
        match dinamico {
            Some(r) => {
                writeln!(o, "  %wbr{v} = icmp ne i8 {r}, 0").unwrap();
                writeln!(o, "  br i1 %wbr{v}, label %wb{v}.ref, label %wb{v}.fim").unwrap();
                writeln!(o, "wb{v}.ref:").unwrap();
                writeln!(o, "  call void @df.barreira(i64 {so}, i64 {sv})").unwrap();
                writeln!(o, "  br label %wb{v}.fim").unwrap();
            }
            None => {
                writeln!(o, "  call void @df.barreira(i64 {so}, i64 {sv})").unwrap();
                writeln!(o, "  br label %wb{v}.fim").unwrap();
            }
        }
        writeln!(o, "wb{v}.fim:").unwrap();
        self.rotulo_atual = format!("wb{v}.fim");
    }

    /// A instrução lê o contexto da thread (`%ctx`): campos em linha ou
    /// alocação em linha.
    fn usa_contexto(inst: &Instruction) -> bool {
        match inst {
            Instruction::GetField { index, .. } | Instruction::SetField { index, .. } => *index < CAMPOS_EM_LINHA,
            Instruction::AllocObject { fields, .. } => !fields.is_empty(),
            Instruction::CallRuntime { name, args, .. } => {
                Self::alocacao_em_linha(inst).is_some()
                    || name == "dartforge_exception_clear"
                    || ((name == "dartforge_object_get" || (name == "dartforge_object_set" && args.len() == 4))
                        && matches!(args.get(1), Some((Operand::Constant(Constant::Int(i)), _)) if (0..CAMPOS_EM_LINHA as i64).contains(i)))
            }
            _ => false,
        }
    }
    /// Tipo estatico de um operando dentro da funcao corrente.
    /// A instrução pode alocar (e, portanto, coletar)? Conservador: só não
    /// coleta o que comprovadamente emite só aritmética, memória local ou
    /// extern que não aloca — e sem conversão que encaixote (um operando
    /// `Ref` onde se espera escalar pode lançar `TypeError`, que aloca; um
    /// escalar onde se espera `Ref`, ou uma constante de texto, aloca).
    fn pode_coletar(&self, inst: &Instruction) -> bool {
        let escalar = |op: &Operand| {
            self.tipo_de(op) != Type::Ref && !matches!(op, Operand::Constant(Constant::String(_) | Constant::StringWtf8(_)))
        };
        let exato = |op: &Operand, t: Type| match op {
            Operand::Constant(Constant::String(_) | Constant::StringWtf8(_)) => false,
            Operand::Constant(Constant::Null) => t == Type::Ref,
            _ => {
                let a = self.tipo_de(op);
                a == t || (a != Type::Ref && t != Type::Ref)
            }
        };
        match inst {
            Instruction::Const(Constant::Int(_) | Constant::Double(_) | Constant::Bool(_) | Constant::Null) => false,
            Instruction::Add(a, b)
            | Instruction::Sub(a, b)
            | Instruction::Mul(a, b)
            | Instruction::SDiv(a, b)
            | Instruction::SRem(a, b)
            | Instruction::Shl(a, b)
            | Instruction::AShr(a, b)
            | Instruction::LShr(a, b)
            | Instruction::And(a, b)
            | Instruction::Or(a, b)
            | Instruction::Xor(a, b)
            | Instruction::FAdd(a, b)
            | Instruction::FSub(a, b)
            | Instruction::FMul(a, b)
            | Instruction::FDiv(a, b)
            | Instruction::ICmp(_, a, b)
            | Instruction::FCmp(_, a, b) => !(escalar(a) && escalar(b)),
            Instruction::Neg(a)
            | Instruction::Not(a)
            | Instruction::FNeg(a)
            | Instruction::LNot(a)
            | Instruction::IntToDouble(a)
            | Instruction::DoubleToInt(a) => !escalar(a),
            Instruction::Alloca(_) | Instruction::Load { .. } | Instruction::Phi { .. } => false,
            // Aritmética de vetores e o desencaixe (carga direta): não alocam.
            Instruction::Simd { .. } => false,
            Instruction::Unbox { to, .. } if to.e_vetor() => false,
            // Em linha (load/store no vetor de campos), sem conversão que
            // aloque: um `Ref` já é `i64`, um `double` é `bitcast`.
            Instruction::GetField { index, .. } => (*index as usize) >= CAMPOS_EM_LINHA,
            Instruction::SetField { index, value, .. } => {
                (*index as usize) >= CAMPOS_EM_LINHA
                    || matches!(value, Operand::Constant(Constant::String(_) | Constant::StringWtf8(_)))
            }
            Instruction::Store { ptr, val } => {
                let t = match ptr {
                    Operand::Val(p) => self.apontado.get(p).copied().unwrap_or(Type::I64),
                    _ => return true,
                };
                !exato(val, t)
            }
            Instruction::CargaNativa { endereco, indice, .. } => !(escalar(endereco) && escalar(indice)),
            Instruction::GravacaoNativa { endereco, indice, valor, .. } => {
                !(escalar(endereco) && escalar(indice) && escalar(valor))
            }
            Instruction::CallRuntime { name, args, .. } => {
                let e = externs::efeitos_de(name);
                let nao_aloca = !e.aloca && !e.chama_dart;
                !(nao_aloca && args.iter().all(|(a, t)| exato(a, *t)))
            }
            _ => true,
        }
    }

    fn tipo_de(&self, op: &Operand) -> Type {
        match op {
            Operand::Val(v) => self.tipos.get(v).copied().unwrap_or(Type::I64),
            Operand::Constant(Constant::Int(_)) => Type::I64,
            Operand::Constant(Constant::Double(_)) => Type::F64,
            Operand::Constant(Constant::Bool(_)) => Type::I1,
            Operand::Constant(Constant::Null) => Type::Ref,
            Operand::Constant(Constant::String(_) | Constant::StringWtf8(_)) => Type::Ref,
            Operand::Constant(Constant::Funcao(_)) => Type::I64,
        }
    }

    fn largura(t: Type) -> u32 {
        match t {
            Type::I1 => 1,
            Type::I8 => 8,
            _ => 64,
        }
    }

    /// Literal double na forma hexadecimal do LLVM.
    ///
    /// format!("{d}") imprime 1 para 1.0 e o LLVM recusa "double 1"; a forma
    /// 0x com os 16 digitos do padrao IEEE 754 sempre vale.
    fn double_literal(d: f64) -> String {
        format!("0x{:016X}", d.to_bits())
    }

    /// Reescreve uma constante diretamente no tipo pedido, sem instrucao.
    /// Devolve None quando o operando nao e constante.
    fn constante_no_tipo(&self, op: &Operand, alvo: Type) -> Option<String> {
        let Operand::Constant(c) = op else { return None };
        let s = match (c, alvo) {
            (Constant::String(_) | Constant::StringWtf8(_), _) => panic!("string deve ser carregada via Instruction::Const"),
            (Constant::Funcao(f), Type::I64) => format!("ptrtoint (ptr @{f} to i64)"),
            (Constant::Funcao(_), _) => panic!("endereço de função só como i64"),
            (Constant::Int(n), Type::I1) => if *n != 0 { "true".to_string() } else { "false".to_string() },
            (Constant::Bool(b), Type::I1) => if *b { "true".to_string() } else { "false".to_string() },
            (Constant::Null, Type::I1) => "false".to_string(),
            (Constant::Double(d), Type::I1) => if d.to_bits() != 0 { "true".to_string() } else { "false".to_string() },
            (Constant::Double(d), Type::F64) => Self::double_literal(*d),
            (Constant::Int(n), Type::F64) => Self::double_literal(f64::from_bits(*n as u64)),
            (Constant::Bool(b), Type::F64) => Self::double_literal(f64::from_bits(u64::from(*b))),
            (Constant::Null, Type::F64) => Self::double_literal(0.0),
            (Constant::Int(n), _) => n.to_string(),
            (Constant::Bool(b), _) => if *b { "1".to_string() } else { "0".to_string() },
            (Constant::Null, _) => "0".to_string(),
            (Constant::Double(d), _) => (d.to_bits() as i64).to_string(),
        };
        Some(s)
    }

    /// Converte um operando para o tipo que a posicao exige.
    ///
    /// O emissor imprime o tipo LLVM em cada posicao ("ret i64", "add i64",
    /// "br i1", o tipo declarado de cada argumento do runtime), mas a HIR
    /// carrega bool ora como i1 (resultado de icmp), ora como i8 (a fronteira
    /// com o Rust), e int/handle como i64. Sem esta conversao o Clang recusa o
    /// modulo inteiro — era a causa de 172 das 214 falhas do corpus nativo.
    ///
    /// Entre i1/i8/i64 a conversao e zext/trunc. Entre double e i64 e bitcast,
    /// nao sitofp/fptosi: a HIR tem nos proprios (IntToDouble/DoubleToInt) para
    /// a conversao numerica, entao um double ocupando um slot i64 so pode ser o
    /// padrao de bits — e assim que Const(Double) e emitido e como os doubles
    /// atravessam o runtime.
    fn coagir(&mut self, op: &Operand, alvo: Type) -> String {
        if alvo == Type::Void {
            return self.operand_str(op);
        }
        if let Some(s) = self.constante_no_tipo(op, alvo) {
            return s;
        }
        let mut atual = self.tipo_de(op);
        let mut texto = self.operand_str(op);
        if atual.llvm_ir() == alvo.llvm_ir() && (atual == Type::F64) == (alvo == Type::F64) {
            return texto;
        }
        // double vira i64 (e vice-versa) sempre pelos bits; larguras menores
        // passam antes por i64 porque bitcast exige tamanho igual.
        if atual == Type::F64 && alvo != Type::F64 {
            texto = self.emitir_conversao("bitcast", Type::F64, &texto, Type::I64);
            atual = Type::I64;
        }
        if alvo == Type::F64 {
            if atual != Type::I64 && atual != Type::Ref {
                texto = self.emitir_conversao("zext", atual, &texto, Type::I64);
                atual = Type::I64;
            }
            if atual != Type::F64 {
                texto = self.emitir_conversao("bitcast", Type::I64, &texto, Type::F64);
            }
            return texto;
        }
        if Self::largura(atual) < Self::largura(alvo) {
            self.emitir_conversao("zext", atual, &texto, alvo)
        } else if Self::largura(atual) > Self::largura(alvo) {
            self.emitir_conversao("trunc", atual, &texto, alvo)
        } else {
            texto
        }
    }

    /// Mesma conversao, mas gravando num nome escolhido por quem chama
    /// (usado pelas entradas de phi, que precisam de um nome estavel).
    fn emitir_conversao_nomeada(&mut self, nome: &str, de: Type, origem: &str, para: Type) {
        let mut atual = de;
        let mut texto = origem.to_string();
        if atual == Type::F64 && para != Type::F64 && Self::largura(para) != 64 {
            texto = self.emitir_conversao("bitcast", Type::F64, &texto, Type::I64);
            atual = Type::I64;
        }
        if para == Type::F64 && atual != Type::F64 && Self::largura(atual) != 64 {
            texto = self.emitir_conversao("zext", atual, &texto, Type::I64);
            atual = Type::I64;
        }
        let op = if atual == Type::F64 || para == Type::F64 {
            "bitcast"
        } else if Self::largura(atual) < Self::largura(para) {
            "zext"
        } else {
            "trunc"
        };
        writeln!(self.out, "  {nome} = {op} {} {texto} to {}", atual.llvm_ir(), para.llvm_ir()).unwrap();
    }

    fn emitir_conversao(&mut self, op: &str, de: Type, texto: &str, para: Type) -> String {
        let c = self.prox_coercao;
        self.prox_coercao += 1;
        writeln!(self.out, "  %c{c} = {op} {} {texto} to {}", de.llvm_ir(), para.llvm_ir()).unwrap();
        format!("%c{c}")
    }

    fn operand_str(&self, op: &Operand) -> String {
        match op {
            Operand::Val(v) => format!("%v{}", v.0),
            Operand::Constant(Constant::Int(n)) => n.to_string(),
            Operand::Constant(Constant::Double(d)) => {
                let _bits = d.to_bits();
                // Em instrução com double imediato, LLVM aceita ponto ou notação científica
                format!("{d}")
            }
            Operand::Constant(Constant::Bool(b)) => if *b { "true" } else { "false" }.to_string(),
            Operand::Constant(Constant::Null) => "0".to_string(),
            Operand::Constant(Constant::String(_) | Constant::StringWtf8(_)) => {
                panic!("string deve ser carregada via Instruction::Const");
            }
            Operand::Constant(Constant::Funcao(f)) => format!("ptrtoint (ptr @{f} to i64)"),
        }
    }
}

/// Raízes por mapas: os ajudantes `@df.*` que comprovadamente não coletam
/// ganham `"gc-leaf-function"` — fora de linha (o programa grande), a
/// chamada a eles não vira ponto de coleta. Em linha o atributo não muda
/// nada. Só os que não chamam o runtime, ou chamam uma extern que não aloca
/// no heap do coletor (`dartforge_lembrar`).
fn ajudantes_folha(texto: &str) -> String {
    const FOLHAS: &[&str] = &["@df.corpo(", "@df.barreira(", "@df.barreira_elemento(", "@df.e_objeto(", "@df.filho_jovem("];
    let mut saida = String::with_capacity(texto.len() + 256);
    for linha in texto.split_inclusive('\n') {
        let alvo = linha.starts_with("define internal ") && FOLHAS.iter().any(|f| linha.contains(f));
        match linha.rfind(" {") {
            Some(i) if alvo => {
                saida.push_str(&linha[..i]);
                saida.push_str(" \"gc-leaf-function\"");
                saida.push_str(&linha[i..]);
            }
            _ => saida.push_str(linha),
        }
    }
    saida
}

/// Quantas portas Rust → Dart o módulo do programa define
/// ([`LlvmEmitter::emitir_portas`]): aridades 0 a 7, sem valor e com valor. O
/// runtime tem o mesmo número (`PORTAS_DART`, `excecoes_tabelas.rs`).
const N_PORTAS: usize = 16;

/// O que todo módulo declara nas exceções por tabelas
/// (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13):
///
/// * a personalidade das funções com pouso (`dartforge_personalidade`, do
///   runtime) e o registro das portas;
/// * `@df.lancar`, o desenrolamento: uma exceção estruturada do sistema com
///   o código próprio `0xE0444652` (o inteiro abaixo), não continuável, sem
///   parâmetros — a exceção Dart em si é a pendência do runtime. `internal`:
///   cada módulo (e cada parte de um módulo dividido) tem a sua cópia.
const EXCECOES_POR_TABELAS: &str = "; Exceções por tabelas (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md)\n\
declare i32 @dartforge_personalidade(...)\n\
declare void @dartforge_registrar_portas(ptr, i64)\n\
declare dllimport void @RaiseException(i32, i32, i32, ptr)\n\
define internal void @df.lancar() noreturn noinline cold \"gc-leaf-function\" {\n\
  call void @RaiseException(i32 -532396462, i32 1, i32 0, ptr null)\n\
  unreachable\n\
}\n";

/// O mesmo nos alvos Itanium (Linux, macOS): `@df.lancar` pede ao runtime
/// o objeto de exceção da thread (`excecoes_tabelas.rs`) e o entrega ele
/// mesmo ao desenrolador do sistema (`_Unwind_RaiseException`), como o
/// `RaiseException` do Windows: nenhum quadro Rust fica entre o lançamento
/// e o pouso. Na variante do runtime com `panic=unwind` (a DLL do SDK da
/// fonte), uma função Rust `extern "C"` no caminho tem a guarda de abortar,
/// e a personalidade do Rust devolvia `_URC_FATAL_PHASE1_ERROR` (3) à
/// exceção estrangeira. Se o desenrolador voltar, o código vai ao runtime,
/// que encerra.
const EXCECOES_POR_TABELAS_ITANIUM: &str = "; Exceções por tabelas (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md)
declare i32 @dartforge_personalidade(...)
declare void @dartforge_registrar_portas(ptr, i64)
declare ptr @dartforge_objeto_de_desenrolamento()
declare i32 @_Unwind_RaiseException(ptr)
declare void @dartforge_desenrolamento_falhou(i32) noreturn
define internal void @df.lancar() noreturn noinline cold \"gc-leaf-function\" {
  %o = call ptr @dartforge_objeto_de_desenrolamento()
  %r = call i32 @_Unwind_RaiseException(ptr %o)
  call void @dartforge_desenrolamento_falhou(i32 %r)
  unreachable
}
";

/// A chamada, na entrada do programa, de uma função Dart sem valor: direta
/// ou, nas exceções por tabelas, pela porta da aridade
/// ([`LlvmEmitter::emitir_portas`]) — a entrada é chamada pelo runtime, e um
/// desenrolamento não pode chegar a ele.
fn chamada_de_entrada(por_tabelas: bool, simbolo: &str, args: &[&str]) -> String {
    if por_tabelas {
        let mut lista = format!("ptr @{simbolo}");
        for a in args {
            lista.push_str(", ");
            lista.push_str(a);
        }
        format!("call void @df.porta.v{}({lista})", args.len())
    } else {
        format!("call void @{simbolo}({})", args.join(", "))
    }
}

/// O hash (FNV-1a de 64 bits) do nome de um slot ou da chave do módulo na
/// área de globais; nunca 0, que marca o slot que não migra.
fn hash_de_slot(nome: &str) -> i64 {
    let h = fnv(nome.bytes()) & !(BIT_DE_CACHE as u64);
    (if h == 0 { 1 } else { h }) as i64
}

/// O bit que marca, no descritor da área, o nome de um slot de cache de
/// seletor (o runtime tem a mesma constante, `gc_raizes.rs`).
pub(crate) const BIT_DE_CACHE: i64 = 1 << 62;

/// O nome da palavra `palavra` (0 ou 1) do cache `nome`, com [`BIT_DE_CACHE`].
fn hash_de_cache(nome: &str, palavra: u8) -> i64 {
    (fnv(nome.bytes().chain([0, palavra])) | BIT_DE_CACHE as u64) as i64
}

/// FNV-1a de 64 bits.
fn fnv(bytes: impl Iterator<Item = u8>) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// A caixa e o desencaixe de `int` em linha (R10, `runtime/src/heap.rs`,
/// `smi`): um `int` em `[-2^62, 2^62)` numa posição `Ref` é o `Smi`
/// `(v << 1) | 1`, sem alocação; fora da faixa, e para desencaixar uma
/// referência que não é `Smi` (o `_Mint`, ou o `TypeError`), o runtime.
/// `internal` em cada módulo e `alwaysinline`: o caminho comum vira três
/// instruções no lugar da chamada.
const OBTER_AREA: &str = "@df.area_id = internal global i64 0, align 8\n\
define internal ptr @df.obter_area() alwaysinline {\n\
  %ctx = call ptr @dartforge_contexto()\n\
  %id = load atomic i64, ptr @df.area_id monotonic, align 8\n\
  %tp = getelementptr inbounds i8, ptr %ctx, i64 16\n\
  %t = load ptr, ptr %tp, align 8\n\
  %np = getelementptr inbounds i8, ptr %ctx, i64 24\n\
  %n = load i64, ptr %np, align 8\n\
  %dentro = icmp ult i64 %id, %n\n\
  br i1 %dentro, label %ler, label %lenta\n\
ler:\n\
  %e = getelementptr ptr, ptr %t, i64 %id\n\
  %p = load ptr, ptr %e, align 8\n\
  %tem = icmp ne ptr %p, null\n\
  br i1 %tem, label %pronto, label %lenta\n\
pronto:\n\
  ret ptr %p\n\
lenta:\n\
  %q = call ptr @dartforge_area_de_globais_id(ptr @df.area, ptr @df.area_id)\n\
  ret ptr %q\n\
}\n";

/// A classe de um valor e o cache do ponto de chamada por seletor, em
/// linha. Um objeto do espaço (`h & 3 == 2`, `runtime/src/heap.rs`,
/// `Cabecalho`) tem o `class_id` no cabeçalho (`bloco + 4`, e o handle é
/// `bloco + 2`): uma carga, como o `LoadClassId` da VM. Um valor do runtime
/// num slot (`h > 0`, `h & 3 == 0`, índice `h / 4 - 1`) tem a classe no
/// vetor denso do heap (`Heap::classes`, 8 bytes por slot, o `cid` nos 4
/// primeiros), cujo endereço e comprimento o contexto da thread publica
/// (deslocamentos 336 e 344, `runtime/src/heap.rs`, `Contexto`): relidos a
/// cada uso, porque o vetor cresce (muda de endereço) quando o heap ganha
/// slots. Índice fora da faixa ou classe desconhecida (`i32::MIN`), e o
/// resto (`null` e `Smi` sem o SDK da fonte), perguntam ao runtime
/// (`dartforge_value_class`).
///
/// `df.subclasse` é o `cid <: C` pelo mapa de bits do alvo que o runtime
/// monta na primeira consulta (`runtime/src/nucleo.rs`, `MapasDeSubtipo`;
/// deslocamentos 352–368 do contexto), o *type testing stub* da VM: sem
/// chamada quando o mapa existe e o `cid` está na largura dele.
/// O cache (`cache[0]` = classe + 1, `cache[1]` = entrada, na área do
/// isolado) é conferido aqui; só a falha chama `dartforge_seletor`, que
/// busca na tabela da classe e regrava o cache — o *inline cache*
/// monomórfico da VM (`ICData`), sem a chamada ao runtime no acerto.
///
/// Com o SDK da fonte, `null` e o `Smi` também saem sem o runtime: as
/// classes deles (`Null`, `_Smi`) são as da tabela `cids` do módulo
/// (`sdk_modulo::cids_do_runtime`, a que o runtime recebe na partida).
fn classe_do_valor(cids: &[i64]) -> String {
    let texto = CLASSE_DO_VALOR
        .replace("@@MASCARA@@", &MASCARA_DE_OBJETO.to_string())
        .replace("@@CLASSE@@", &(layout::desl::CLASSE as i64 - layout::DESLOCAMENTO_DO_HANDLE).to_string())
        .replace("@@SUBTIPOS@@", &layout::contexto::SUBTIPOS.to_string())
        .replace("@@N_SUBTIPOS@@", &layout::contexto::N_SUBTIPOS.to_string())
        .replace("@@LARGURA@@", &layout::contexto::LARGURA_SUBTIPOS.to_string());
    // Sem o SDK da fonte (o caminho legado), `null` e o `Smi` vão ao runtime.
    if !matches!(cids, [n, s, ..] if *n >= 0 && *s >= 0) {
        return texto.replace("@@RAPIDOS@@", "");
    }
    let rapidos = format!(
        "  %z = icmp eq i64 %h, 0\n\
  br i1 %z, label %nulo, label %s0\n\
nulo:\n\
  ret i64 {nulo}\n\
s0:\n\
  %b = and i64 %h, 1\n\
  %i = icmp ne i64 %b, 0\n\
  br i1 %i, label %smi, label %s1\n\
smi:\n\
  ret i64 {smi}\n\
s1:\n",
        nulo = layout::cid::NULL,
        smi = layout::cid::SMI
    );
    texto.replace("@@RAPIDOS@@", &rapidos)
}

const CLASSE_DO_VALOR: &str = "define internal i64 @df.classe(i64 %h) alwaysinline {\n\
@@RAPIDOS@@  %m = and i64 %h, @@MASCARA@@\n\
  %o = icmp eq i64 %m, 2\n\
  br i1 %o, label %obj, label %rt\n\
obj:\n\
  %p = inttoptr i64 %h to ptr\n\
  %cp = getelementptr inbounds i8, ptr %p, i64 @@CLASSE@@\n\
  %c = load i32, ptr %cp, align 4, !invariant.load !{}\n\
  %r = sext i32 %c to i64\n\
  ret i64 %r\n\
rt:\n\
  %x = call i64 @dartforge_value_class(i64 %h)\n\
  ret i64 %x\n\
}\n\
define internal ptr @df.seletor(ptr %c, i64 %r, i64 %h, ptr %n, i64 %l) alwaysinline {\n\
  %cid = call i64 @df.classe(i64 %r)\n\
  %k = add i64 %cid, 1\n\
  %c0 = load i64, ptr %c, align 8\n\
  %sim = icmp eq i64 %c0, %k\n\
  br i1 %sim, label %acerto, label %falha\n\
acerto:\n\
  %ep = getelementptr inbounds i64, ptr %c, i64 1\n\
  %e = load ptr, ptr %ep, align 8\n\
  ret ptr %e\n\
falha:\n\
  %f = call ptr @dartforge_seletor(ptr %c, i64 %r, i64 %h, ptr %n, i64 %l)\n\
  ret ptr %f\n\
}\n\
define internal i8 @df.subclasse(i64 %cid, i64 %alvo) alwaysinline {\n\
  %ctx = call ptr @dartforge_contexto()\n\
  %lp = getelementptr inbounds i8, ptr %ctx, i64 @@LARGURA@@\n\
  %l = load i64, ptr %lp, align 8\n\
  %d1 = icmp ult i64 %cid, %l\n\
  br i1 %d1, label %t1, label %lento\n\
t1:\n\
  %np = getelementptr inbounds i8, ptr %ctx, i64 @@N_SUBTIPOS@@\n\
  %n = load i64, ptr %np, align 8\n\
  %d2 = icmp ult i64 %alvo, %n\n\
  br i1 %d2, label %t2, label %lento\n\
t2:\n\
  %tp = getelementptr inbounds i8, ptr %ctx, i64 @@SUBTIPOS@@\n\
  %t = load ptr, ptr %tp, align 8\n\
  %mp = getelementptr inbounds ptr, ptr %t, i64 %alvo\n\
  %mapa = load ptr, ptr %mp, align 8\n\
  %tem = icmp ne ptr %mapa, null\n\
  br i1 %tem, label %t3, label %lento\n\
t3:\n\
  %w = lshr i64 %cid, 6\n\
  %wp = getelementptr inbounds i64, ptr %mapa, i64 %w\n\
  %pal = load i64, ptr %wp, align 8\n\
  %s = and i64 %cid, 63\n\
  %x = lshr i64 %pal, %s\n\
  %x8 = trunc i64 %x to i8\n\
  %b = and i8 %x8, 1\n\
  ret i8 %b\n\
lento:\n\
  %y = call i8 @dartforge_is_subclass(i64 %cid, i64 %alvo)\n\
  ret i8 %y\n\
}\n";

/// Ajudantes `@df.*` que vão fora de linha (com o `seletor` e a `classe`, o
/// despacho dinâmico, em [`ajudantes_fora_de_linha`]): cada um é uma
/// sequência que se repete em centenas de milhares de lugares do programa
/// grande (docs/NATIVO-PRODUCAO-GRANDE.md §1.4).
pub const AJUDANTES_FORA_DO_PROGRAMA_GRANDE: &[&str] =
    &["subclasse", "alocar", "barreira", "barreira_elemento", "obter_area", "nova_instancia"];

/// A lista de ajudantes fora de linha: `DARTFORGE_AJUDANTES_FORA` (lista
/// separada por vírgula; `1`, todos; vazio, nenhum); senão todos no
/// programa grande, no desenvolvimento (`-O0`, onde o `alwaysinline` copia
/// o ajudante em cada uso e a geração de código paga por cada cópia:
/// `desenvolvimento_grande`) e na produção (`programa_grande`), e nenhum
/// nos demais — o programa pequeno roda o dobro do tempo no `-O0` sem os
/// ajudantes em linha (o corpus/nativo: 53 → 116 s).
pub fn ajudantes_fora_de_linha(desenvolvimento_grande: bool, programa_grande: bool) -> Vec<String> {
    let todos = || ["seletor", "classe"].iter().chain(AJUDANTES_FORA_DO_PROGRAMA_GRANDE).map(|s| (*s).to_string()).collect();
    match std::env::var("DARTFORGE_AJUDANTES_FORA") {
        Ok(v) if v == "1" => todos(),
        Ok(v) => v.split(',').map(str::trim).filter(|s| !s.is_empty()).map(str::to_string).collect(),
        Err(_) if desenvolvimento_grande || programa_grande => todos(),
        Err(_) => Vec::new(),
    }
}

/// O despacho por seletor com o descritor estático (`@df.seld.<k>`), fora
/// de linha ([`LlvmEmitter::seletor_compacto`]).
const SELETOR_POR_DESCRITOR: &str = "define internal ptr @df.seletor_d(ptr %c, i64 %r, ptr %d) noinline {
  %h = load i64, ptr %d, align 8
  %np = getelementptr inbounds i8, ptr %d, i64 8
  %n = load ptr, ptr %np, align 8
  %lp = getelementptr inbounds i8, ptr %d, i64 16
  %l = load i64, ptr %lp, align 8
  %f = call ptr @df.seletor(ptr %c, i64 %r, i64 %h, ptr %n, i64 %l)
  ret ptr %f
}
";

/// A alocação de instância de `emitir_alocacao_em_linha` fora de linha:
/// `%cur` é o deslocamento do cursor da TLAB das palavras do objeto no
/// contexto (o fim vem 8 bytes depois), `%tam` os bytes do bloco, `%cab` a
/// palavra do cabeçalho, `%tab` a função da tabela de métodos (null quando a
/// classe não registra a tabela na primeira alocação).
fn nova_instancia() -> String {
    format!(
        "define internal i64 @df.nova_instancia(ptr %ctx, i64 %cur, i64 %tam, i64 %cab, i64 %c, i64 %n, ptr %tab) noinline {{
  %ta = getelementptr inbounds i8, ptr %ctx, i64 %cur
  %tb = load ptr, ptr %ta, align 8
  %tfa = getelementptr inbounds i8, ptr %ta, i64 8
  %tf = load ptr, ptr %tfa, align 8
  %tnx = getelementptr i8, ptr %tb, i64 %tam
  %tz = icmp ugt ptr %tnx, %tf
  %semtab = icmp eq ptr %tab, null
  br i1 %tz, label %lento, label %reg
reg:
  br i1 %semtab, label %rapido, label %reg2
reg2:
  %tnp = getelementptr inbounds i8, ptr %ctx, i64 56
  %tn = load i64, ptr %tnp, align 8
  %tk = icmp ule i64 %tn, %c
  br i1 %tk, label %lento, label %reg3
reg3:
  %trp = getelementptr inbounds i8, ptr %ctx, i64 48
  %tr = load ptr, ptr %trp, align 8
  %trb = getelementptr inbounds i8, ptr %tr, i64 %c
  %trv = load i8, ptr %trb, align 1
  %trz = icmp eq i8 %trv, 0
  br i1 %trz, label %lento, label %rapido
rapido:
  store ptr %tnx, ptr %ta, align 8
  %tpp = getelementptr inbounds i8, ptr %tb, i64 16
  store i64 0, ptr %tpp, align 8
  store i64 %cab, ptr %tb, align 8
  %thb = ptrtoint ptr %tb to i64
  %th = add i64 %thb, 2
  ret i64 %th
lento:
  br i1 %semtab, label %semt, label %comt
comt:
  %a = call i64 @dartforge_object_new_t(i64 %c, i64 %n, ptr %tab)
  ret i64 %a
semt:
  %b = call i64 @dartforge_object_new(i64 %c, i64 %n)
  ret i64 %b
}}
"
    )
}

/// `texto` com os `define internal … @df.<nome>(…) alwaysinline` de `nomes`
/// trocados para `noinline`.
fn fora_de_linha(texto: &str, nomes: &[String]) -> String {
    if nomes.is_empty() {
        return texto.to_string();
    }
    let mut saida = String::with_capacity(texto.len());
    for linha in texto.split_inclusive('\n') {
        let alvo = linha.starts_with("define internal ")
            && nomes.iter().any(|n| linha.contains(&format!("@df.{n}(")));
        if alvo {
            saida.push_str(&linha.replacen(" alwaysinline", " noinline", 1));
        } else {
            saida.push_str(linha);
        }
    }
    saida
}

/// Os ajudantes da fundação do espaço unificado (docs/NATIVO-ESPACO-UNIFICADO.md
/// §3.5), com os deslocamentos do contrato de layout:
///
/// * `@df.e_objeto(h)`: `h & (7 | i64::MIN) == 2`;
/// * `@df.filho_jovem(v)`: o filho gravado pede a barreira (objeto jovem);
/// * `@df.barreira(o, v)`: `o` velho recebendo filho jovem → `dartforge_lembrar`
///   (peso 1:9 no ramo lento, como a barreira da Julia);
/// * `@df.barreira_elemento(o, i, v)`: a mesma com cartões (`o` velho ou
///   lembrado com `CARTOES`: suja o cartão do elemento `i`);
/// * `@df.alocar(cabecalho, w)`: a TLAB de `w ≤ TLAB_N` palavras (o cursor
///   avança `16 + 8w` e grava a palavra 0); esgotada ou `w` maior,
///   `dartforge_alocar(cid, w, cabecalho >> 8)`.
/// `@df.corpo(h, ctx)`: o começo do corpo de um objeto (o cabeçalho; os
/// campos 16 bytes depois), a mesma conta de `emitir_endereco_dos_campos` em
/// linha — um valor que não é objeto lê o objeto de reserva do contexto
/// (`ctx + 40`), e um corpo fora do bloco (anexo, `flags & 1`) é seguido pelo
/// ponteiro em `+16`. `alwaysinline` quando o emissor está em linha (para o
/// otimizador ver igual); sem o atributo no desenvolvimento, uma chamada.
fn ajudante_do_corpo(por_chamada: bool, producao: bool) -> String {
    // Na produção por chamada: fora de linha de verdade, e sem efeito nem
    // dependência de memória (`memory(none)`): o corpo de um objeto não
    // muda durante a vida dele — o coletor não move, e o corpo de fora só
    // nasce na migração da recarga do JIT (que nunca é produção) —, então o
    // otimizador junta todas as chamadas sobre o mesmo objeto, mesmo com
    // gravações e chamadas no meio (o quadro do `$async`, o `this`).
    // Antes, `memory(read)`: duas chamadas só se juntavam sem escrita entre
    // elas. docs/NATIVO-PRODUCAO-GRANDE.md §6.3.
    let atributo = match (por_chamada, producao) {
        (false, _) => " alwaysinline",
        (true, false) => "",
        (true, true) => " noinline nounwind willreturn memory(none)",
    };
    format!(
        "define internal ptr @df.corpo(i64 %o, ptr %ctx){atributo} {{
  %k = and i64 %o, {MASCARA_DE_OBJETO}
  %e = icmp eq i64 %k, 2
  %a = add i64 %o, -2
  %q = inttoptr i64 %a to ptr
  %vp = getelementptr inbounds i8, ptr %ctx, i64 40
  %v = load ptr, ptr %vp, align 8
  %h = select i1 %e, ptr %q, ptr %v
  %fp = getelementptr inbounds i8, ptr %h, i64 1
  %f = load i8, ptr %fp, align 1
  %b = and i8 %f, 1
  %x = icmp ne i8 %b, 0
  %cp = getelementptr inbounds i8, ptr %h, i64 16
  %c = load ptr, ptr %cp, align 8
  %r = select i1 %x, ptr %c, ptr %h
  ret ptr %r
}}
"
    )
}

fn ajudantes_do_espaco() -> String {
    use layout::{contexto, desl, estado, flags};
    let d = |x: usize| x as i64 - layout::DESLOCAMENTO_DO_HANDLE;
    format!(
        "define internal i1 @df.e_objeto(i64 %h) alwaysinline {{\n\
  %m = and i64 %h, {mascara}\n\
  %r = icmp eq i64 %m, 2\n\
  ret i1 %r\n\
}}\n\
define internal i1 @df.filho_jovem(i64 %v) alwaysinline {{\n\
  %b = and i64 %v, 1\n\
  %imediato = icmp ne i64 %b, 0\n\
  %nulo = icmp eq i64 %v, 0\n\
  %nada = or i1 %imediato, %nulo\n\
  br i1 %nada, label %nao, label %s1\n\
s1:\n\
  %m = and i64 %v, {mascara}\n\
  %o = icmp eq i64 %m, 2\n\
  br i1 %o, label %obj, label %nao\n\
obj:\n\
  %p = inttoptr i64 %v to ptr\n\
  %ep = getelementptr inbounds i8, ptr %p, i64 {estado_h}\n\
  %e = load i8, ptr %ep, align 8\n\
  %j = icmp eq i8 %e, {jovem}\n\
  ret i1 %j\n\
nao:\n\
  ret i1 false\n\
}}\n\
define internal void @df.barreira(i64 %o, i64 %v) alwaysinline {{\n\
  %p = inttoptr i64 %o to ptr\n\
  %ep = getelementptr inbounds i8, ptr %p, i64 {estado_h}\n\
  %e = load i8, ptr %ep, align 8\n\
  %velho = icmp eq i8 %e, {velho}\n\
  %x = call i1 @llvm.expect.i1(i1 %velho, i1 false)\n\
  br i1 %x, label %filho, label %fim\n\
filho:\n\
  %j = call i1 @df.filho_jovem(i64 %v)\n\
  br i1 %j, label %lembrar, label %fim\n\
lembrar:\n\
  call void @dartforge_lembrar(i64 %o)\n\
  br label %fim\n\
fim:\n\
  ret void\n\
}}\n\
define internal void @df.barreira_elemento(i64 %o, i64 %i, i64 %v) alwaysinline {{\n\
  %p = inttoptr i64 %o to ptr\n\
  %ep = getelementptr inbounds i8, ptr %p, i64 {estado_h}\n\
  %e = load i8, ptr %ep, align 8\n\
  %e3 = sub i8 %e, {velho}\n\
  %velho = icmp ult i8 %e3, 2\n\
  %x = call i1 @llvm.expect.i1(i1 %velho, i1 false)\n\
  br i1 %x, label %filho, label %fim\n\
filho:\n\
  %j = call i1 @df.filho_jovem(i64 %v)\n\
  br i1 %j, label %lento, label %fim\n\
lento:\n\
  %fp = getelementptr inbounds i8, ptr %p, i64 {flags_h}\n\
  %f = load i8, ptr %fp, align 1\n\
  %fc = and i8 %f, {cartoes}\n\
  %temc = icmp ne i8 %fc, 0\n\
  br i1 %temc, label %cartao, label %lembrar\n\
cartao:\n\
  %lp = getelementptr inbounds i8, ptr %p, i64 {comprimento_h}\n\
  %len = load i64, ptr %lp, align 8\n\
  %w = lshr i64 %i, {log_palavra}\n\
  %k = add i64 %len, %w\n\
  %kb = shl i64 %k, 3\n\
  %kd = add i64 %kb, {elementos_h}\n\
  %cp = getelementptr inbounds i8, ptr %p, i64 %kd\n\
  %cv = load i64, ptr %cp, align 8\n\
  %s = lshr i64 %i, {log_cartao}\n\
  %s63 = and i64 %s, 63\n\
  %bit = shl i64 1, %s63\n\
  %nv = or i64 %cv, %bit\n\
  store i64 %nv, ptr %cp, align 8\n\
  br label %lembrar\n\
lembrar:\n\
  %ev = icmp eq i8 %e, {velho}\n\
  br i1 %ev, label %chamar, label %fim\n\
chamar:\n\
  call void @dartforge_lembrar(i64 %o)\n\
  br label %fim\n\
fim:\n\
  ret void\n\
}}\n\
define internal i64 @df.alocar(i64 %cab, i64 %w) alwaysinline {{\n\
  %pequeno = icmp ule i64 %w, {tlab_n}\n\
  br i1 %pequeno, label %tlab, label %lento\n\
tlab:\n\
  %ctx = call ptr @dartforge_contexto()\n\
  %o16 = shl i64 %w, 4\n\
  %oc = add i64 %o16, {tlab}\n\
  %ta = getelementptr inbounds i8, ptr %ctx, i64 %oc\n\
  %tb = load ptr, ptr %ta, align 8\n\
  %of = add i64 %oc, 8\n\
  %tfa = getelementptr inbounds i8, ptr %ctx, i64 %of\n\
  %tf = load ptr, ptr %tfa, align 8\n\
  %w8 = shl i64 %w, 3\n\
  %tam = add i64 %w8, {cabecalho}\n\
  %tnx = getelementptr i8, ptr %tb, i64 %tam\n\
  %esgotada = icmp ugt ptr %tnx, %tf\n\
  %x = call i1 @llvm.expect.i1(i1 %esgotada, i1 false)\n\
  br i1 %x, label %lento, label %rapido\n\
rapido:\n\
  store ptr %tnx, ptr %ta, align 8\n\
  store i64 %cab, ptr %tb, align 8\n\
  %hb = ptrtoint ptr %tb to i64\n\
  %h = add i64 %hb, {deslocamento}\n\
  ret i64 %h\n\
lento:\n\
  %cid = lshr i64 %cab, 32\n\
  %fl = lshr i64 %cab, 8\n\
  %fn = and i64 %fl, 16777215\n\
  %r = call i64 @dartforge_alocar(i64 %cid, i64 %w, i64 %fn)\n\
  ret i64 %r\n\
}}\n",
        mascara = MASCARA_DE_OBJETO,
        estado_h = d(desl::ESTADO),
        flags_h = d(desl::FLAGS),
        comprimento_h = d(desl::COMPRIMENTO),
        elementos_h = d(desl::ELEMENTOS),
        jovem = estado::JOVEM,
        velho = estado::VELHO,
        cartoes = flags::CARTOES,
        log_palavra = layout::ELEMENTOS_POR_PALAVRA_DE_CARTAO.trailing_zeros(),
        log_cartao = layout::ELEMENTOS_POR_CARTAO.trailing_zeros(),
        tlab_n = layout::TLAB_N,
        tlab = contexto::TLAB,
        cabecalho = layout::TAMANHO_DO_CABECALHO,
        deslocamento = layout::DESLOCAMENTO_DO_HANDLE,
    )
}

// `@df.caixa_int`, `@df.desencaixa_int` e `@df.env_ref` são da P2
// (`caixas_ir::AJUDANTES`, docs/NATIVO-ESPACO-UNIFICADO.md §3.5).

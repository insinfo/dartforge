//! Gerador de LLVM IR a partir da HIR nativa.

pub mod c_abi;
pub mod externs;
mod debug_info;
mod roots;
mod arc_context;
mod arc_ports;
mod stack_trace;
pub mod verify_maps;
pub mod verify_shadow;
mod simd;
// As emissões que dependem da representação dos valores do runtime, uma por
// pacote do espaço unificado (docs/NATIVO-ESPACO-UNIFICADO.md §4.2, passo 4).
mod ir_boxes;
mod ir_lists;
mod ir_strings;
mod ir_typed;

use dartforge_runtime::layout;
/// Maior índice de campo lido em linha (o contrato de layout).
const INLINE_FIELDS: usize = layout::CAMPOS_EM_LINHA;
/// O maior corpo, em palavras, com alocação em linha (a TLAB de `w` palavras,
/// `layout::context::tlab_cursor(w)`): um objeto de até `TLAB_N` campos.
const TLAB_N: i64 = layout::TLAB_N as i64;
/// A máscara de `e_objeto` (`h & (7 | i64::MIN) == 2`).
const OBJECT_MASK: i64 = 7 | i64::MIN;
mod selectors;
#[cfg(test)]
mod tests;

use crate::hir::*;
use std::fmt::Write;

pub struct LlvmEmitter<'a> {
    module: &'a Module,
    /// As funções do módulo pelo símbolo (a primeira com cada nome, como o
    /// `find` linear que esta tabela substituiu: ele era feito a cada
    /// `CallStatic` e tornava a emissão quadrática — dezenas de minutos no
    /// new_sali/backend, docs/NATIVO-PROJETOS-REAIS.md).
    function_by_symbol: std::collections::HashMap<&'a str, &'a Function>,
    out: String,
    /// As constantes de string, na ordem de emissão (determinística); o
    /// índice de cada uma é a posição.
    string_constants: Vec<Vec<u8>>,
    /// O índice de cada constante pelo conteúdo: coletar e consultar sem
    /// percorrer a tabela (antes, quadrático no número de literais).
    string_index: std::collections::HashMap<Vec<u8>, usize>,
    /// Tipo de cada valor da funcao sendo emitida, para coercao de operandos.
    types: std::collections::HashMap<ValueId, Type>,
    /// Contador dos temporarios de coercao (%c0, %c1, ...), por funcao.
    next_coercion: u32,
    /// Conversoes que uma entrada de phi exige, atribuidas ao bloco de ORIGEM:
    /// (bloco, nome do temporario, tipo de origem, valor, tipo do phi).
    conv_phi: Vec<(u32, String, Type, ValueId, Type)>,
    /// Tipo guardado por cada `alloca` da função (para o `store`).
    pointee: std::collections::HashMap<ValueId, Type>,
    /// Os `alloca` `Ref` cujo endereço sai da função (a captura por endereço
    /// de uma função local direta, `lower/direct_functions.rs`): moram no
    /// próprio slot do quadro de raízes, e quem recebe o endereço grava
    /// onde o coletor enxerga.
    frame_allocas: std::collections::HashSet<ValueId>,
    /// G: slot de raiz de cada valor `Ref` da função (SSA ou `alloca`).
    slots: std::collections::HashMap<ValueId, usize>,
    /// A função abriu um quadro de raízes (`%gcq`).
    has_frame: bool,
    /// A função leu `%ctx` (`dartforge_contexto`, o contexto da thread do
    /// runtime) na entrada: exceção pendente e pilha-sombra sem chamada.
    has_ctx: bool,
    /// O rótulo LLVM em que termina cada bloco da HIR que se divide em
    /// vários (a alocação em linha, [`LlvmEmitter::inline_allocation`]): é
    /// o predecessor que os `phi` dos sucessores nomeiam.
    exit_labels: std::collections::HashMap<u32, String>,
    /// O rótulo LLVM do trecho em emissão (o do bloco, ou o último que uma
    /// instrução dividida abriu): o predecessor de um `phi` interno.
    current_label: String,
    // --- P1 (closures, α) ---
    /// Vetores constantes de `i64` (`@df.arr.<k>`): assinaturas e descritores.
    vectors: Vec<Vec<i64>>,
    vector_of: std::collections::HashMap<Vec<i64>, usize>,
    /// Vetores constantes de endereços de função (`@df.fns.<k>`).
    function_tables: Vec<Vec<String>>,
    function_table_of: std::collections::HashMap<Vec<String>, usize>,
    /// Os nomes dos argumentos nomeados dos descritores do módulo: o
    /// descritor só guarda o hash, e o `Invocation` de um `noSuchMethod`
    /// precisa do nome (`dartforge_registrar_nome_de_argumento`).
    argument_names: std::collections::BTreeSet<String>,
    // --- P5c (SDK da fonte, δ; `selectors.rs`) ---
    /// Pontos de chamada por seletor já emitidos (um cache cada).
    selector_caches: usize,
    /// O símbolo da função em emissão e quantos caches de seletor ela já
    /// abriu: o nome do slot de um cache é `(função, posição)`, estável
    /// entre gerações de uma recarga (J04).
    current_function: String,
    cache_in_function: usize,
    /// O índice de cada nome de slot do layout da geração viva (recarga).
    slot_index: std::collections::HashMap<i64, usize>,
    /// Textos dos seletores, na ordem do primeiro uso.
    selector_names: Vec<String>,
    /// Assinatura de cada símbolo chamado, para declarar o que o módulo não
    /// define.
    externals: std::collections::BTreeMap<String, String>,
    /// Os thunks Owned genéricos chamados (`df.arc.owned.<base>`): a base e a
    /// assinatura (`selectors::emit_owned_thunks`).
    owned_thunks: std::collections::BTreeMap<String, (String, Vec<Type>)>,
    /// Os thunks de escopo nativo (`df.arc.scope.<base>`): base, argumentos
    /// e retorno.
    scope_thunks: std::collections::BTreeMap<String, (String, Vec<Type>, Type)>,
    /// Símbolos definidos em `comdat` (`selectors::linkage_of`).
    comdats: Vec<String>,
    /// `DARTFORGE_RASTRO=1` na compilação: cada função conta a entrada e a
    /// saída ao runtime, que mostra a pilha de funções Dart numa exceção
    /// (`DARTFORGE_DEPURAR=1`). Só para depurar o SDK da fonte.
    stack_trace: Option<usize>,
    stack_trace_names: Vec<String>,
    // --- Área de globais por isolado ---
    /// O slot (`i64`) de cada global do módulo na área (`@dfg_…` e a
    /// bandeira `$ok`): os estáticos do Dart são por isolado, e cada
    /// isolado é uma thread com a sua área (`dartforge_area_de_globais`).
    global_slots: std::collections::HashMap<String, usize>,
    /// O hash do nome de cada slot, na ordem (o descritor da área): 0 nos
    /// caches de seletor.
    slot_hashes: Vec<i64>,
    /// O layout da área da geração em execução (hot reload): os slots que
    /// continuam mantêm o índice, os novos vêm depois (`emit_globals`).
    previous_area: Option<Vec<i64>>,
    /// O descritor da área sem os nomes dos slots (`[key, -n]`): o
    /// executável de produção, que nunca recarrega (`with_lean_area`).
    lean_area: bool,
    /// J05: os metadados de depuração, quando alguma função tem posições.
    debug_info: Option<debug_info::DebugInfo>,
    // --- Espaço unificado (docs/NATIVO-ESPACO-UNIFICADO.md) ---
    /// Os literais de string são objetos estáticos do módulo (§2.11):
    /// verdadeiro no AOT e nos módulos do SDK; falso nos módulos do programa no
    /// JIT, cuja memória é liberada (J02).
    static_objects: bool,
    /// O endereço dos campos de um objeto por chamada a `@df.body` em vez
    /// das 15 instruções em linha (o perfil de desenvolvimento do programa,
    /// `-O0`): o IR do new_sali/backend tinha 465 mil cópias da sequência,
    /// 21% das linhas (docs/NATIVO-PROJETOS-REAIS.md, C9). A produção e os
    /// módulos do SDK continuam em linha.
    fields_per_call: bool,
    /// A alocação de instância por chamada ao `@df.new_instance` (a
    /// produção do programa com `new_instance` em
    /// [`LlvmEmitter::outlined_helpers`]).
    out_of_line_allocation: bool,
    /// Os ajudantes `@df.*` que vão `noinline` no módulo do programa de
    /// produção ([`out_of_line_helpers`]).
    outlined_helpers: Vec<String>,
    /// O despacho por seletor fora de linha (`selector` em
    /// [`LlvmEmitter::outlined_helpers`]): o ponto de chamada passa o
    /// descritor estático do seletor (`@df.seld.<k>`: hash, nome,
    /// comprimento) ao `@df.seletor_d`, que tem o `@df.selector` em linha —
    /// três argumentos em vez de cinco em cada um dos ~170 mil pontos do
    /// programa grande.
    compact_selector: bool,
    /// `optsize` em cada função do módulo (o programa grande na produção;
    /// o SDK de produção já o tem, `sdk_modulo::with_optsize`).
    optimize_size: bool,
    /// Os objetos recém-alocados neste bloco da HIR, sem ponto de coleta
    /// desde a alocação (`emit_field_addresses`): o corpo está no
    /// bloco (nunca de fora, nunca null nem `Smi`) e o objeto é jovem (a
    /// barreira não faz nada).
    fresh_set: std::collections::HashSet<u32>,
    /// Nenhum objeto tem corpo de fora (`with_body_without_outside`).
    body_without_outside: bool,
    /// O `this` da função em emissão, quando o corpo dele é direto
    /// ([`Self::direct_body`]).
    this_object: Option<u32>,
    /// Cada quadro de raízes desta emissão fica num quadro nativo próprio
    /// (o programa sem otimização: `-O0` no AOT, `CodeGenLevelNone` no JIT,
    /// sem inlining): o runtime pode conferir a ordem dos endereços na
    /// pilha-sombra (o quadro morto, `RT/heap.rs` `visitar_quadros`). Com
    /// inlining, dois quadros viram `alloca` da mesma função, em ordem
    /// qualquer, e a conferência daria falso positivo.
    ordered_frames: bool,
    /// O módulo é do perfil de produção (`--optimize`): com
    /// [`Self::fields_per_call`], o `@df.body` vai `noinline` (o
    /// otimizador o poria de volta em linha).
    production: bool,
    /// As tabelas de métodos são montadas na ligação
    /// (docs/NATIVO-PODA-DE-TABELAS.md §3.2): o módulo só declara a função
    /// de cada tabela, e o conteúdo vai para o resumo (`pruning.rs`). É o SDK
    /// de produção.
    tables_at_link: bool,
    /// O estado de cada pacote da emissão (§4.2, passo 4); os pacotes o usam
    /// conforme migram.
    #[allow(dead_code)]
    strings: ir_strings::StringState,
    #[allow(dead_code)]
    boxes: ir_boxes::BoxState,
    #[allow(dead_code)]
    lists: ir_lists::ListState,
    #[allow(dead_code)]
    typed_items: ir_typed::TypedState,
    /// Exceções por tabelas (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13):
    /// a decisão do passe `optimize::tables` para a função em emissão —
    /// quem é `invoke`, os pousos e como cada `Return` sai. `None` no modelo
    /// de sempre (a pendência conferida depois de cada chamada), e o IR é
    /// então o de sempre, byte a byte.
    tab: Option<&'a FunctionTables>,
    /// `DARTFORGE_EFEITOS=check` ([`externs::check_effects`]): os nomes
    /// das externs conferidas neste módulo, na ordem do primeiro uso (o
    /// índice é o do global `@df.efn.<k>` com o texto).
    checked_externs: Vec<String>,
    /// Raízes por mapas de pilha (`--roots=maps`,
    /// docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §14.12): os valores SSA `Ref`
    /// vivos através de um ponto de coleta não vão para o quadro da
    /// pilha-sombra; cada chamada que pode coletar leva, no operando
    /// `"deopt"`, os vivos na entrada dela, e o `rewrite-statepoints-for-gc`
    /// do LLVM a transforma em statepoint, com o lugar de cada um no mapa de
    /// pilha da chamada — o registrador preservado em que ele já está, ou o
    /// slot. O coletor não move objetos: nada é relocado, e o código segue
    /// com o próprio valor. Só os `alloca` `Ref` continuam no quadro.
    maps: bool,
    /// Com [`Self::maps`], a análise de raízes da função em emissão.
    function_roots: roots::Roots,
    /// Com [`Self::maps`], os valores enraizados da função em emissão.
    rooted_set: std::collections::HashSet<ValueId>,
    /// Raízes por mapas: as definições de `@df.vararg.<k>`, os intermediários
    /// das chamadas C variádicas feitas por uma função `gc`
    /// ([`Self::c_call_text`]); vão ao módulo depois das funções.
    variadic_set: Vec<String>,
    /// O build de conferência do percurso (`DARTFORGE_RAIZES_CONFERIR=1`
    /// com `--roots=maps`, docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §15.2,
    /// E2.5): numa função `gc`, o primeiro dos slots de conferência no fim
    /// do quadro de raízes e quantos são. Antes de cada ponto de coleta eles
    /// recebem exatamente os valores vivos nele (zero nos demais), e o
    /// runtime confere que o percurso por mapas visitou todos.
    check: Option<(usize, usize)>,
    /// Raízes por mapas num módulo do JIT (§3.8): o gerenciador de memória do
    /// JIT acha o mapa de cada objeto e o registra; o módulo não chama o
    /// registro nem leva o `-ni:1` (o `LLJIT` recusa um módulo com a camada
    /// de dados diferente da dele, e o JIT não roda otimização de IR).
    maps_in_jit: bool,
    /// Quantas funções `gc` o módulo emitiu (no Mach-O, o `.no_dead_strip`
    /// do mapa só vai a um módulo que tem mapa).
    gc_functions: usize,
    /// O rastro no formato da VM (§13.14, `llvm/stack_trace.rs`): os rótulos das
    /// chamadas e a tabela da seção `dfpcl`. `None`: sem tabela.
    vm_stack_trace: Option<stack_trace::StackTrace>,
    /// O módulo vai para o JIT: a seção do rastro de cada objeto é
    /// registrada pelo gerenciador de memória da sessão (`crates/jit`), e o
    /// módulo não tem os símbolos do ligador nem a chamada de registro.
    stack_trace_in_jit: bool,
}

impl<'a> LlvmEmitter<'a> {
    pub fn new(module: &'a Module) -> Self {
        Self {
            module,
            out: String::new(),
            string_constants: Vec::new(),
            string_index: std::collections::HashMap::new(),
            types: std::collections::HashMap::new(),
            next_coercion: 0,
            conv_phi: Vec::new(),
            pointee: std::collections::HashMap::new(),
            frame_allocas: std::collections::HashSet::new(),
            slots: std::collections::HashMap::new(),
            has_frame: false,
            has_ctx: false,
            exit_labels: std::collections::HashMap::new(),
            current_label: String::new(),

            vectors: Vec::new(),
            vector_of: std::collections::HashMap::new(),
            function_tables: Vec::new(),
            function_table_of: std::collections::HashMap::new(),
            argument_names: std::collections::BTreeSet::new(),
            selector_caches: 0,
            current_function: String::new(),
            cache_in_function: 0,
            slot_index: std::collections::HashMap::new(),
            selector_names: Vec::new(),
            externals: std::collections::BTreeMap::new(),
            owned_thunks: std::collections::BTreeMap::new(),
            scope_thunks: std::collections::BTreeMap::new(),
            comdats: Vec::new(),
            stack_trace: std::env::var("DARTFORGE_RASTRO").is_ok_and(|v| v == "1").then_some(0),
            stack_trace_names: Vec::new(),
            global_slots: std::collections::HashMap::new(),
            slot_hashes: Vec::new(),
            previous_area: None,
            lean_area: false,
            // As posições servem às tabelas de linha só com `--debug_info`; sem
            // ela, são só do rastro simbólico (`vm_stack_trace`).
            debug_info: (module.dwarf && module.functions.iter().any(|f| f.debug_info.is_some())).then(debug_info::DebugInfo::new_one),
            function_by_symbol: {
                let mut m = std::collections::HashMap::with_capacity(module.functions.len());
                for f in &module.functions {
                    m.entry(f.symbol.as_str()).or_insert(f);
                }
                m
            },
            static_objects: false,
            fields_per_call: false,
            production: false,
            ordered_frames: false,
            out_of_line_allocation: false,
            outlined_helpers: Vec::new(),
            optimize_size: false,
            fresh_set: std::collections::HashSet::new(),
            body_without_outside: false,
            this_object: None,
            compact_selector: false,
            tables_at_link: false,
            strings: Default::default(),
            boxes: Default::default(),
            lists: Default::default(),
            typed_items: Default::default(),
            tab: None,
            checked_externs: Vec::new(),
            maps: false,
            function_roots: roots::Roots::default(),
            rooted_set: std::collections::HashSet::new(),
            variadic_set: Vec::new(),
            check: None,
            gc_functions: 0,
            maps_in_jit: false,
            vm_stack_trace: None,
            stack_trace_in_jit: false,
        }
    }

    /// Delega o registro das seções do rastro ao gerenciador de memória do JIT.
    /// O módulo deixa de emitir os símbolos do ligador e a chamada de registro.
    pub fn with_stack_trace_in_jit(mut self, yes: bool) -> Self {
        self.stack_trace_in_jit = yes;
        self
    }

    /// O rastro simbólico (§13.14): os rótulos antes das chamadas e a
    /// tabela da imagem (`alvo::symbolic_stack_trace`; no JIT, com
    /// [`LlvmEmitter::with_stack_trace_in_jit`]).
    pub fn with_stack_trace(mut self, yes: bool) -> Self {
        self.vm_stack_trace = yes.then(stack_trace::StackTrace::new);
        self
    }

    /// A chamada de registro dos mapas de pilha da imagem, por formato: a
    /// base da imagem PE (o runtime acha `.dfgcm` e `.llvm_st` pelos
    /// cabeçalhos); os limites da seção `dfgcm` que o `ld.lld` define; no
    /// Mach-O, os do `__llvm_stackmaps` do LLVM.
    fn maps_registration_call() -> &'static str {
        match crate::alvo::sistema() {
            crate::alvo::Sistema::Windows => "  call void @dartforge_registrar_mapa(ptr @__ImageBase)\n",
            crate::alvo::Sistema::Linux => "  call void @dartforge_registrar_mapa_secao(ptr @__start_dfgcm, ptr @__stop_dfgcm)\n",
            crate::alvo::Sistema::MacOs => {
                "  call void @dartforge_registrar_mapa_llvm(ptr @\"\\01section$start$__LLVM_STACKMAPS$__llvm_stackmaps\", ptr @\"\\01section$end$__LLVM_STACKMAPS$__llvm_stackmaps\")\n"
            }
        }
    }

    /// O registro dos campos do `dart:async` que o rastro percorre (§13.14).
    fn trace_field_registration_calls(&self) -> String {
        let mut s = String::new();
        for (cid, name_str, position) in &self.module.trace_fields {
            let idx = self.string_const_index(name_str.as_bytes()).unwrap_or(0);
            writeln!(s, "  call void @dartforge_registrar_campo_do_rastro(i64 {cid}, ptr @.str.{idx}, i64 {}, i64 {position})", name_str.len()).unwrap();
        }
        // As entradas de tear-off dos ramos de stream.
        // Só as que sobraram da poda (a entrada some com quem a criava).
        for (sym_name, kind) in &self.module.stack_trace_tearoffs {
            if !self.module.functions.iter().any(|f| f.symbol == *sym_name) {
                continue;
            }
            writeln!(s, "  call void @dartforge_registrar_tearoff_do_rastro(ptr @{sym_name}, i64 {kind})").unwrap();
        }
        s
    }

    /// A chamada de registro da tabela do rastro da imagem (§13.14).
    fn trace_registration_call(&self) -> Option<String> {
        self.vm_stack_trace.as_ref()?;
        if self.stack_trace_in_jit {
            return None;
        }
        let (_, start, end) = stack_trace::markers();
        Some(format!("  call void @dartforge_registrar_rastro(ptr {start}, ptr {end})\n"))
    }

    /// Obtém o endereço dos campos por chamada a `@df.body`, fora de linha.
    /// Usado no perfil de desenvolvimento para reduzir o tamanho do IR.
    pub fn with_fields_per_call(mut self, yes: bool) -> Self {
        self.fields_per_call = yes;
        self
    }

    /// Seleciona o perfil de produção; com campos por chamada, mantém
    /// `@df.body` fora de linha por meio de `noinline`.
    pub fn with_production(mut self, yes: bool) -> Self {
        self.production = yes;
        self
    }

    /// Marca quadros de raízes como ordenados para a conferência do runtime.
    /// Requer emissão sem inlining, com um quadro nativo por função.
    pub fn with_ordered_frames(mut self, yes: bool) -> Self {
        self.ordered_frames = yes;
        self
    }

    /// Nenhum objeto terá corpo de fora (o bit `FORA`, que só a migração da
    /// recarga do JIT cria — `runtime/src/heap.rs`, `trocar_campos`): o AOT
    /// e o SDK de produção. Com isto, o campo do `this` de um método é lido
    /// direto do bloco ([`Self::direct_body`]).
    pub fn with_body_without_outside(mut self, yes: bool) -> Self {
        self.body_without_outside = yes;
        self
    }

    /// Aplica `optsize` a cada função emitida no módulo.
    pub fn with_optimize_size(mut self, yes: bool) -> Self {
        self.optimize_size = yes;
        self
    }

    /// Os ajudantes fora de linha do módulo do programa (veja
    /// [`out_of_line_helpers`]); um módulo do SDK os ignora.
    pub fn with_outlined_helpers(mut self, names: Vec<String>) -> Self {
        if self.module.sdk_library {
            return self;
        }
        self.out_of_line_allocation = names.iter().any(|n| n == "nova_instancia");
        self.compact_selector = names.iter().any(|n| n == "seletor");
        // O `@df.selector` continua `alwaysinline`: só o `@df.seletor_d`
        // (fora de linha) o chama.
        self.outlined_helpers = names.into_iter().filter(|n| n != "seletor").collect();
        self
    }

    pub fn with_static_objects(mut self, yes: bool) -> Self {
        self.static_objects = yes;
        self
    }

    /// Delega o registro dos mapas de pilha ao gerenciador de memória do JIT.
    /// O módulo deixa de emitir o registro e a opção de layout `-ni:1`.
    pub fn with_maps_in_jit(mut self, yes: bool) -> Self {
        self.maps_in_jit = yes;
        self
    }

    /// Emite raízes SSA pelos mapas de pilha dos statepoints do LLVM.
    /// Os `alloca` de referências continuam no quadro da pilha-sombra.
    pub fn with_roots_by_maps(mut self, yes: bool) -> Self {
        self.maps = yes;
        self
    }

    pub fn with_tables_at_link(mut self, yes: bool) -> Self {
        self.tables_at_link = yes;
        self
    }

    /// O emissor de uma geração nova de um programa em execução (hot reload
    /// do JIT): `previous` são os nomes dos slots da área da geração viva
    /// ([`crate::ir_area`]). Os globais que continuam mantêm o índice e
    /// os novos vêm depois, então o código das duas gerações lê e grava a
    /// mesma área — os quadros `async` suspensos e as closures que ainda
    /// executam código antigo veem os mesmos estáticos que o código novo.
    pub fn with_previous_area(mut self, previous: Option<Vec<i64>>) -> Self {
        self.previous_area = previous;
        self
    }

    /// O descritor da área só com a chave e o número de slots, `[key, -n]`
    /// (o sinal marca a forma enxuta para `dartforge_area_de_globais`): os
    /// hashes dos nomes só servem à recarga (a migração da área e a
    /// extensão do layout), e o executável de produção nunca recarrega. O
    /// JIT e a DLL de desenvolvimento ficam com o descritor completo.
    pub fn with_lean_area(mut self, lean: bool) -> Self {
        self.lean_area = lean;
        self
    }

    /// O módulo inteiro. Um módulo com diagnósticos não chega aqui:
    /// `emitir_ir` devolve o erro antes (N1).
    ///
    /// ```
    /// use dartforge_emit_native::{hir::Module, llvm::LlvmEmitter};
    /// let modulo = Module::new();
    /// let ir = LlvmEmitter::new(&modulo).emit_all();
    /// assert!(ir.contains("declare"));
    /// ```
    ///
    /// # Panics
    /// Módulo com diagnósticos ou metadados excepcionais incoerentes.
    /// Retoma ainda exige Itanium sem statepoints; Windows precisa de funclets.
    pub fn emit_all(mut self) -> String {
        assert!(self.module.errors.is_empty(), "emit_all com diagnósticos: {:?}", self.module.errors);
        assert!(!cfg!(windows) || !self.module.tables.iter().any(|t|
            t.exits.values().any(|s| *s == ExceptionExit::Resume)),
            "Retoma exige Itanium; Windows precisa de funclets SEH");
        // Coleta literais de strings do módulo para declaração como constantes globais
        self.collect_string_constants();

        // 1. Cabeçalho de target
        self.emit_header();

        // 2. Declarações do runtime
        self.emit_runtime_decls();
        self.emit_arc_aot_ports();

        // 3. Constantes de strings globais
        self.emit_string_constants();

        // 4. Classes e vtables
        self.emit_vtables();

        // 4b. Globais do usuário (N6)
        self.emit_globals();

        // 4c. Tabela de código das closures e vetores constantes (P1)
        self.emit_closures();

        // 5. Funções compiladas
        let module = self.module;
        if module.excecoes_por_tabelas {
            assert!(
                module.tables.len() == module.functions.len(),
                "bug do compilador: as funções do módulo mudaram depois do passe das exceções por tabelas"
            );
        }
        for (k, func) in module.functions.iter().enumerate() {
            self.tab = if module.excecoes_por_tabelas { module.tables.get(k) } else { None };
            self.emit_function(func);
        }
        self.tab = None;
        for d in std::mem::take(&mut self.variadic_set) {
            self.out.push_str(&d);
        }

        if self.module.sdk_library {
            // Uma biblioteca do SDK da fonte (P5c): sem entrada nem despacho
            // de `toString`, que são do programa; a função de registro das
            // classes dela, que a entrada do programa chama.
            let reg = self.module.record.clone().unwrap_or_else(|| "df.registrar".to_string());
            self.emit_registration(&reg);
            self.emit_selector_globals();
            self.emit_area_descriptor();
            self.out.push_str(GET_AREA);
            self.emit_string_globals();
            self.emit_external_declarations();
            self.close_maps();
            if let Some(r) = self.vm_stack_trace.take() {
                r.finish(&mut self.out);
            }
            if let Some(d) = self.debug_info.take() {
                d.finish(&mut self.out);
            }
            return self.out;
        }

        // 6. Funções de despacho polimórfico
        if self.module.sdk_mode {
            self.emit_to_string_by_selector();
        } else {
            self.emit_dispatch_functions();
        }

        // 7. Entrada global @dartforge_entry
        self.emit_entry();

        self.emit_selector_globals();
        self.emit_area_descriptor();
        // A área deste isolado com o layout desta geração, criada (ou
        // estendida) agora: a publicação de uma recarga do JIT a chama no
        // ponto seguro, antes de o código novo executar.
        self.out.push_str(
            "define void @df.preparar_area() {\n  %a = call ptr @dartforge_area_de_globais(ptr @df.area)\n  ret void\n}\n",
        );
        // A área no prólogo de cada função: o id do módulo indexa a tabela
        // desta thread no `Context` (deslocamentos 16 e 24); uma entrada
        // ausente vai ao runtime, que a preenche (e dá o id na primeira vez).
        // O índice da área do programa tem nome próprio: o particionamento
        // (`partition.rs`) o promove a externo, e a LTO completa dos módulos
        // do SDK, ao dividir o módulo juntado em partições, promove o
        // `@df.area_id` interno de um deles com o mesmo nome — "duplicate
        // symbol: df.area_id" (docs/NATIVO-PRODUCAO-GRANDE.md §5).
        let get_area = GET_AREA.replace("@df.area_id", "@df.area_id.programa");
        self.out.push_str(&out_of_line(&get_area, &self.outlined_helpers));
        self.emit_string_globals();
        self.emit_external_declarations();
        self.close_maps();
        if let Some(r) = self.vm_stack_trace.take() {
            r.finish(&mut self.out);
        }
        if let Some(d) = self.debug_info.take() {
            d.finish(&mut self.out);
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
    ///
    /// Antes, o `"deopt"()` vazio em toda chamada que pode coletar e ainda
    /// não tem o dela ([`empty_deopt`]).
    fn close_maps(&mut self) {
        if self.maps {
            self.out = empty_deopt(&self.out);
        }
        if self.maps && !self.maps_in_jit && self.gc_functions > 0 && crate::alvo::sistema() == crate::alvo::Sistema::MacOs {
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
                        Self::register_string(&mut self.string_constants, &mut self.string_index, bytes);
                    }
                }
            }
        }
        for class in &self.module.classes {
            Self::register_string(&mut self.string_constants, &mut self.string_index, class.name.as_bytes());
        }
        for (_, name_str, _) in &self.module.trace_fields {
            Self::register_string(&mut self.string_constants, &mut self.string_index, name_str.as_bytes());
        }
    }

    /// Acrescenta `bytes` à tabela de constantes, se ainda não estiver.
    fn register_string(table: &mut Vec<Vec<u8>>, idx_value: &mut std::collections::HashMap<Vec<u8>, usize>, bytes: &[u8]) {
        if !idx_value.contains_key(bytes) {
            idx_value.insert(bytes.to_vec(), table.len());
            table.push(bytes.to_vec());
        }
    }

    fn emit_header(&mut self) {
        if self.maps && !self.maps_in_jit {
            // `-ni:1`: o `addrspace(1)` é de ponteiros não integrais — o
            // otimizador não fabrica `ptrtoint`/`inttoptr` com as raízes.
            self.out.push_str(&crate::alvo::cabecalho_ir().replacen("-S128\"", "-S128-ni:1\"", 1));
        } else {
            self.out.push_str(crate::alvo::cabecalho_ir());
        }
        // Os ids das classes do programa, para a geração seguinte de uma
        // recarga do JIT (J03, `context::ir_ids`).
        for (id, lib, class) in &self.module.program_ids {
            writeln!(self.out, "; df.classe {id} {} {}", crate::context::escape_str(lib), crate::context::escape_str(class)).unwrap();
        }
        // O layout dos objetos, para migrar as instâncias vivas (J03):
        // `name:flags:ty`, com `a` anulável e `l` late.
        for (id, base, fields) in &self.module.program_fields {
            let list: Vec<String> = fields
                .iter()
                .map(|c| {
                    let flags = format!("{}{}", if c.nullable { "a" } else { "" }, if c.late { "l" } else { "" });
                    format!("{}:{flags}:{}", crate::context::escape_str(&c.name), crate::context::escape_str(&c.ty))
                })
                .collect();
            writeln!(self.out, "; df.campos {id} {base} {}", list.join(",")).unwrap();
        }
    }

    fn emit_runtime_decls(&mut self) {
        self.out.push_str("; Declarações do runtime nativo Rust (tabela em llvm/externs.rs)\n");
        for e in externs::EXTERNS {
            self.out.push_str(e.decl);
            // Raízes por mapas: a extern que não coleta é folha — a chamada
            // a ela não vira ponto de coleta (nem registro no mapa).
            if self.maps {
                let ef = externs::effects_of(e.nome());
                if (!ef.allocates && !ef.calls_dart) || crate::alvo::sabotaged_leaf(e.nome()) {
                    self.out.push_str(" \"gc-leaf-function\"");
                }
            }
            self.out.push('\n');
        }
        if self.vm_stack_trace.is_some() && !self.stack_trace_in_jit {
            // O começo e o fim da seção do rastro da imagem (§13.14).
            self.out.push_str(stack_trace::markers().0);
        }
        if self.maps && !self.maps_in_jit {
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
        // temporária da pilha (`llvm/c_abi.rs`).
        let composites = self.module.functions.iter().any(|f| {
            f.blocks.iter().any(|b| b.instructions.iter().any(|(_, i, _)| matches!(i, Instruction::CompositeNativeCall { .. })))
        });
        self.out.push_str(simd::DECLARATIONS);
        self.out.push_str("declare i8 @llvm.expect.i8(i8, i8)\n");
        self.out.push_str("declare i1 @llvm.expect.i1(i1, i1)\n");
        for (_, decl) in externs::GLOBALS {
            self.out.push_str(decl);
            self.out.push('\n');
        }
        // Os ajudantes `@df.*` de cada pacote (§3.5).
        let helpers_start = self.out.len();
        self.out.push_str(ir_strings::HELPERS);
        self.out.push_str(ir_boxes::HELPERS);
        self.out.push_str(&ir_lists::helpers(self.module.arc_memory));
        self.out.push_str(ir_typed::HELPERS);
        let value_class = value_class(&self.module.runtime_cids);
        self.out.push_str(&value_class);
        self.out.push_str(&space_helpers());
        self.out.push_str(&body_helper(self.fields_per_call, self.production));
        if !self.outlined_helpers.is_empty() {
            let helpers = self.out.split_off(helpers_start);
            self.out.push_str(&out_of_line(&helpers, &self.outlined_helpers));
        }
        if self.maps {
            let helpers = self.out.split_off(helpers_start);
            self.out.push_str(&leaf_helpers(&helpers));
        }
        if self.out_of_line_allocation {
            self.out.push_str(&new_instance());
        }
        if self.compact_selector {
            self.out.push_str(SELECTOR_BY_DESCRIPTOR);
        }
        if composites || !self.module.ffi_callbacks.is_empty() {
            self.out.push_str(
                "declare void @llvm.memcpy.p0.p0.i64(ptr, ptr, i64, i1)\n\
                 declare void @llvm.memset.p0.i64(ptr, i8, i64, i1)\n\
                 declare ptr @llvm.stacksave.p0()\n\
                 declare void @llvm.stackrestore.p0(ptr)\n",
            );
        }
        if self.module.excecoes_por_tabelas {
            self.out.push_str(if cfg!(windows) { EXCEPTIONS_BY_TABLES } else { EXCEPTIONS_BY_TABLES_ITANIUM });
        }
        if externs::check_effects() {
            // A moldura da conferência não coleta: com as raízes por mapas, é
            // folha (sem statepoint em volta dela).
            if self.maps {
                self.out.push_str(
                    "declare void @dartforge_efeitos_antes(ptr, i64, i64) \"gc-leaf-function\"\ndeclare void @dartforge_efeitos_depois() \"gc-leaf-function\"\n",
                );
            } else {
                self.out.push_str("declare void @dartforge_efeitos_antes(ptr, i64, i64)\ndeclare void @dartforge_efeitos_depois()\n");
            }
            self.out.push_str("declare i64 @dartforge_efeitos_nivel() nounwind \"gc-leaf-function\"\ndeclare void @dartforge_efeitos_restaurar(i64) nounwind \"gc-leaf-function\"\n");
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
        self.string_index.get(s).copied()
    }

    fn emit_vtables(&mut self) {
        // vtables globais se houver classes
        for class in &self.module.classes {
            writeln!(self.out, "; VTable da classe {} (id {})", class.name, class.id).unwrap();
        }
    }

    fn emit_function(&mut self, func: &Function) {
        self.current_function.clone_from(&func.symbol);
        self.cache_in_function = 0;
        self.frame_allocas = Self::escaping_ref_allocas(func);
        self.prepare_types_and_phis(func);

        let ret_ty = func.return_ty.llvm_ir();
        let params: Vec<String> = func
            .params
            .iter()
            .map(|(vid, _, ty)| format!("{} %v{}", ty.llvm_ir(), vid.0))
            .collect();
        let params_str = params.join(", ");

        let (linkage, comdat) = self.linkage_of(&func.symbol);
        // Uma cópia por imagem (`linkonce_odr`): o rastro não a rotula.
        let shared = !linkage.is_empty();
        // §7.4, modo sombra: os valores `Ref` da função (os candidatos de
        // `roots::analyze`) para o conferidor de dominância.
        if !self.maps && verify_shadow::linked() {
            let mut refs: Vec<u32> = func.params.iter().filter(|(_, _, t)| *t == Type::Ref).map(|(v, _, _)| v.0).collect();
            for b in &func.blocks {
                for (v, inst, t) in &b.instructions {
                    if *t == Type::Ref && !matches!(inst, Instruction::Const(Constant::Null) | Instruction::Alloca(_)) {
                        refs.push(v.0);
                    }
                }
            }
            let list: Vec<String> = refs.iter().map(|v| format!("v{v}")).collect();
            writeln!(self.out, "{} {}", verify_shadow::REFS_MARK, list.join(" ")).unwrap();
        }
        let function_start = self.out.len();
        let mut written_position: Option<(u32, u32)> = None;
        // A posição de uma instrução copiada pelo inlining (só o rastro).
        let mut written_embedded: Option<(u32, u32, u32)> = None;
        let attributes = if self.optimize_size { " optsize" } else { "" };
        // O `this` de um método é sempre um objeto do espaço (nunca null nem
        // `Smi`: a classe tem o campo lido); sem corpo de fora, o endereço
        // dos campos dele é `this − 2 + 16`.
        self.this_object = func
            .params
            .first()
            .filter(|(_, name_str, t)| self.body_without_outside && !self.module.arc_memory && name_str == "this" && *t == Type::Ref)
            .map(|(vid, _, _)| vid.0);
        // Exceções por tabelas: a decisão do passe para esta função. Só a
        // função com pouso nomeia a personalidade (é ela que lê a tabela dos
        // pousos); as outras o desenrolamento atravessa.
        let tab = self.tab;
        let has_landing_pad = tab.is_some_and(|t| !t.landing_pads.is_empty());
        let has_resume = tab.is_some_and(|t| t.exits.values().any(|s| *s == ExceptionExit::Resume));
        if let Some(c) = tab.and_then(|t| t.foreign_cleanup.as_ref()) {
            c.check(func, tab.unwrap()).expect("cleanup estrangeiro inválido");
        }
        let has_cleanup = has_resume || (cfg!(unix) && tab.is_some_and(|t| t.foreign_cleanup.is_some()));
        assert!(!has_cleanup || !self.maps, "Retoma não admite landingpad token de statepoint");
        if has_resume {
            crate::optimize::arc::check_resumes(func, tab.unwrap()).expect("Retoma inválida");
        }
        let personality = if has_cleanup { " personality ptr @dartforge_personalidade_cleanup_itanium" }
            else if has_landing_pad { " personality ptr @dartforge_personalidade" } else { "" };

        // G1/G2 (docs/NATIVO-PLANO.md §6.5): um slot por `alloca` de tipo
        // `Ref`, e os valores SSA `Ref` vivos em algum ponto de coleta, com
        // slot compartilhado entre os que nunca estão vivos juntos
        // (`roots.rs`).
        let converting_blocks: std::collections::HashSet<u32> = self.conv_phi.iter().map(|(b, ..)| *b).collect();
        let mut analysis = roots::analyze_with(
            func,
            &self.types,
            &|inst| self.may_collect(inst),
            &|b| converting_blocks.contains(&b.0),
            &|inst| self.callee_roots_the_arguments(inst),
        );
        // O orçamento do `rewrite-statepoints-for-gc` (§3.4): o passe cresce
        // de forma superlinear com `live_set × points de collection`. Acima do teto
        // a função fica inteira na pilha-sombra, como quadro residual
        // (§3.7): sem `gc`, com os slots de sempre.
        let cost = analysis.rooted_set.len() as u64 * (analysis.live_at.len() + analysis.live_at_end.len()) as u64;
        let within_budget = cost <= crate::alvo::maps_budget();
        if self.maps && !within_budget && std::env::var_os("DARTFORGE_RELATORIO_MAPAS").is_some() {
            eprintln!("dartforge: {} fora do orçamento dos mapas ({cost}): pilha-sombra", func.symbol);
        }
        if self.maps && within_budget {
            // Raízes por mapas: no quadro só ficam os `alloca` `Ref` (o
            // mapa de pilha descreve valores SSA, não memória); os valores
            // SSA enraizados vão para o mapa.
            analysis.slots.retain(|v, _| self.pointee.contains_key(v));
            self.rooted_set = analysis.rooted_set.iter().copied().collect();
        } else {
            self.rooted_set.clear();
        }
        self.slots = std::mem::take(&mut analysis.slots);
        self.function_roots = analysis;
        // Raízes por mapas: toda função é `gc` — o
        // `rewrite-statepoints-for-gc` transforma as chamadas dela que podem
        // coletar em statepoints (`crates/llvm`, `gerar`), cada uma com o
        // operando `"deopt"` dos vivos nela (§14.12): vazio na função sem
        // raízes no mapa (as da fora do orçamento estão no quadro), mas
        // presente, para o inliner juntar a ele os vivos de quem a chama.
        let has_gc = self.maps;
        if !self.rooted_set.is_empty() {
            self.gc_functions += 1;
        }
        // O build de conferência: tantos slots a mais quantos o maior
        // conjunto de vivos num ponto de coleta.
        self.check = None;
        if !self.rooted_set.is_empty() && crate::alvo::check_roots() {
            let k = self
                .function_roots
                .live_at
                .values()
                .chain(self.function_roots.live_at_end.values())
                .map(Vec::len)
                .max()
                .unwrap_or(0);
            if k > 0 {
                let first_one = self.slots.values().max().map_or(0, |m| m + 1);
                self.check = Some((first_one, k));
            }
        }
        let gc = if has_gc { " gc \"statepoint-example\"" } else { "" };
        writeln!(self.out, "define {linkage}{ret_ty} @{}({}){attributes}{comdat}{gc}{personality} {{", func.symbol, params_str)
            .unwrap();
        self.has_frame = !self.slots.is_empty() || self.check.is_some();
        self.exit_labels = func
            .blocks
            .iter()
            .filter_map(|b| {
                b.instructions.iter().rev().find_map(|(vid, i, _)| {
                    let v = vid.0;
                    if tab.is_some_and(|t| t.invocations.contains_key(vid)) {
                        // O caminho normal de um `invoke` ([`Self::make_invoke`]).
                        Some((b.id.0, format!("inv{v}.fim")))
                    } else if Self::inline_allocation(i).is_some() && !self.out_of_line_allocation {
                        Some((b.id.0, format!("ao{v}.fim")))
                    } else if self.inline_barrier(i) {
                        Some((b.id.0, format!("wb{v}.fim")))
                    } else if !self.static_objects && matches!(i, Instruction::Const(Constant::String(_) | Constant::StringWtf8(_))) {
                        // Sem os objetos estáticos (o JIT), o literal vai ao
                        // cache do ponto de uso (`ir_strings`), que divide o
                        // bloco; com eles, é uma constante (§2.11).
                        Some((b.id.0, format!("ls{v}.fim")))
                    } else if matches!(i, Instruction::CallRuntime { name, .. } if name == "dartforge_exception_clear") {
                        Some((b.id.0, format!("xc{v}.fim")))
                    } else if Self::rti_evaluate_cached(i) {
                        Some((b.id.0, format!("ra{v}.fim")))
                    } else {
                        None
                    }
                })
            })
            .collect();
        // O pouso com o desvio Dart/estrangeira termina em `lpad{b}.dart`
        // (abaixo): um `phi` emitido antes dele já precisa do rótulo.
        if has_cleanup && let Some(t) = tab {
            for p in &t.landing_pads {
                if t.exits.get(p) != Some(&ExceptionExit::Resume) {
                    self.exit_labels.entry(p.0).or_insert_with(|| format!("lpad{}.dart", p.0));
                }
            }
        }
        self.has_ctx = self.has_frame || Self::requires_explicit_context(func, tab);

        for block in &func.blocks {
            writeln!(self.out, "b{}:", block.id.0).unwrap();
            let block_start = self.out.len();
            self.current_label = format!("b{}", block.id.0);
            // O pouso de um `invoke`: quem desenrolou até aqui deixou a
            // exceção pendente e os quadros de raízes dele (e dos quadros
            // atravessados) ainda encadeados — o topo da pilha-sombra volta
            // a ser o desta função, antes de qualquer outra instrução.
            if tab.is_some_and(|t| t.landing_pads.contains(&block.id)) {
                if tab.is_some_and(|t| t.exits.get(&block.id) == Some(&ExceptionExit::Resume)) {
                    writeln!(self.out, "  %lpad{} = landingpad {{ ptr, i32 }} cleanup", block.id.0).unwrap();
                } else if has_gc {
                    // Numa função `gc` o `invoke` vira statepoint, e o pouso
                    // dele é `token` (o que as relocações do caminho de
                    // exceção referenciam).
                    writeln!(self.out, "  %lpad{} = landingpad token cleanup", block.id.0).unwrap();
                } else {
                    let cleanup = if has_cleanup { " cleanup" } else { "" };
                    writeln!(self.out, "  %lpad{} = landingpad {{ ptr, i32 }}{cleanup} catch ptr null", block.id.0).unwrap();
                }
                let top = if self.has_frame { "%gcq" } else { "%topo0" };
                if !crate::alvo::sabotage("pouso_sem_topo") {
                    writeln!(self.out, "  store ptr {top}, ptr %ctxtopo, align 8").unwrap();
                }
                if externs::check_effects() {
                    writeln!(self.out, "  call void @dartforge_efeitos_restaurar(i64 %efnivel)").unwrap();
                }
                if has_cleanup && tab.is_some_and(|t| t.exits.get(&block.id) != Some(&ExceptionExit::Resume)) {
                    // O inliner pode combinar catch e cleanup. Seletor zero
                    // executa cleanup, mas não autoriza tratar estrangeira como Dart.
                    let b = block.id.0;
                    writeln!(self.out, "  %lpseletor{b} = extractvalue {{ ptr, i32 }} %lpad{b}, 1").unwrap();
                    writeln!(self.out, "  %lptipo{b} = call i32 @llvm.eh.typeid.for(ptr null)").unwrap();
                    writeln!(self.out, "  %lpdart{b} = icmp eq i32 %lpseletor{b}, %lptipo{b}").unwrap();
                    writeln!(self.out, "  br i1 %lpdart{b}, label %lpad{b}.dart, label %lpad{b}.estrangeira").unwrap();
                    writeln!(self.out, "lpad{b}.estrangeira:").unwrap();
                    if let Some(c) = tab.and_then(|t| t.foreign_cleanup.as_ref()) {
                        for v in c.owners(block.id) {
                            writeln!(self.out, "  call void @dartforge_arc_release(i64 %v{})", v.0).unwrap();
                        }
                        for q in c.frames(block.id).iter().rev() {
                            writeln!(self.out, "  call void @dartforge_arc_quadro_fechar_v1(i64 %v{})", q.0).unwrap();
                        }
                    }
                    if self.stack_trace.is_some() {
                        writeln!(self.out, "  call void @dartforge_rastro_saida()").unwrap();
                    }
                    if self.has_frame {
                        writeln!(self.out, "  store ptr %gcant, ptr %ctxtopo, align 8").unwrap();
                    }
                    writeln!(self.out, "  resume {{ ptr, i32 }} %lpad{b}").unwrap();
                    writeln!(self.out, "lpad{b}.dart:").unwrap();
                    self.current_label = format!("lpad{b}.dart");
                    self.exit_labels.entry(b).or_insert_with(|| format!("lpad{b}.dart"));
                }
                // Raízes por mapas: o que o tratador ainda lê estava vivo na
                // entrada do `invoke` e vai no `"deopt"` dele.
            }
            if block.id.0 == 0 {
                self.emit_closure_buffers(func);
                if has_landing_pad && externs::check_effects() {
                    writeln!(self.out, "  %efnivel = call i64 @dartforge_efeitos_nivel()").unwrap();
                }
                // O quadro de raízes também nasce antes de qualquer chamada
                // (N17): o `df.get_area` abaixo é `alwaysinline` e tem
                // desvios, e um `alloca` depois dele deixava de ser do bloco
                // de entrada — virava alocação dinâmica, com `__chkstk` e o
                // `stacksave` a cada chamada da função, e impedia o inliner
                // de levar o quadro para o bloco de entrada de quem chama.
                // `DARTFORGE_SEM_QUADRO_NA_ENTRADA=1` na compilação volta ao
                // lugar antigo (medida).
                if self.has_frame && Self::frame_at_entry() {
                    let n = self.frame_size();
                    writeln!(self.out, "  %gcq = alloca {{ ptr, i64, [{n} x i64] }}, align 8").unwrap();
                }
                // O marco da conferência da pilha
                // (`emit_stack_check`), pelo mesmo motivo: depois
                // do `df.get_area` (ou do contexto em linha,
                // `sdk_modulo::with_context_in_tls`) ele virava alocação
                // dinâmica, com o ponteiro de quadro e o ajuste do `sp` a cada
                // chamada.
                if self.has_ctx {
                    writeln!(self.out, "  %pilhaq = alloca i8, align 1").unwrap();
                }
                if Self::uses_area(func) {
                    writeln!(self.out, "  %area = call ptr @df.obter_area()").unwrap();
                }
            }
            if block.id.0 == 0
                && let Some(k) = self.stack_trace.as_mut()
            {
                let n = *k;
                *k += 1;
                self.stack_trace_names.push(func.symbol.clone());
                writeln!(self.out, "  call void @dartforge_rastro_entrada(ptr @df.rastro.{n}, i64 {})", func.symbol.len()).unwrap();
            }
            // O contexto da thread (`runtime/src/heap.rs`, `Context`): a
            // exceção pendente no deslocamento 0, o topo da pilha-sombra no 8.
            if block.id.0 == 0 && self.has_ctx {
                writeln!(self.out, "  %ctx = call ptr @dartforge_contexto()").unwrap();
                writeln!(self.out, "  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8").unwrap();
                // Sem quadro de raízes, o topo que o pouso restaura é o da
                // entrada da função.
                if has_landing_pad && !self.has_frame {
                    writeln!(self.out, "  %topo0 = load ptr, ptr %ctxtopo, align 8").unwrap();
                }
                self.emit_stack_check(func);
            }
            if block.id.0 == 0 && self.has_frame {
                // O quadro de raízes no stack da função (a pilha-sombra,
                // `QuadroDeRaizes` do runtime): anterior, número de slots e
                // os slots, zerados antes de o runtime encadeá-lo. Cada raiz
                // é um `store` no slot dela (slots compartilhados: o tamanho
                // é o do maior).
                let n = self.frame_size();
                let t = format!("{{ ptr, i64, [{n} x i64] }}");
                if !Self::frame_at_entry() {
                    writeln!(self.out, "  %gcq = alloca {t}, align 8").unwrap();
                }
                let header = self.frame_header(n);
                // A sabotagem `quadro_sujo` (D5, §7.3): os slots nascem com um
                // valor de handle que não aponta para bloco nenhum; um slot
                // lido antes de escrito cai na validação de handle.
                let slots = if n > 0 && crate::alvo::sabotage("quadro_sujo") {
                    format!("[{}]", vec!["i64 4098"; n].join(", "))
                } else {
                    "zeroinitializer".to_string()
                };
                writeln!(self.out, "  store {t} {{ ptr null, i64 {header}, [{n} x i64] {slots} }}, ptr %gcq").unwrap();
                for slot in 0..n {
                    writeln!(self.out, "  %gcs{slot} = getelementptr inbounds {t}, ptr %gcq, i64 0, i32 2, i64 {slot}").unwrap();
                }
                // Encadeia o quadro: `previous` = topo; topo = quadro.
                writeln!(self.out, "  %gcant = load ptr, ptr %ctxtopo, align 8").unwrap();
                writeln!(self.out, "  store ptr %gcant, ptr %gcq, align 8").unwrap();
                writeln!(self.out, "  store ptr %gcq, ptr %ctxtopo, align 8").unwrap();
                for (vid, _, _) in &func.params {
                    if let Some(&slot) = self.slots.get(vid) {
                        writeln!(self.out, "  store i64 %v{}, ptr %gcs{slot}", vid.0).unwrap();
                    }
                }
                let mut in_frame: Vec<ValueId> = self.frame_allocas.iter().copied().collect();
                in_frame.sort_by_key(|v| v.0);
                for vid in in_frame {
                    let slot = self.slots[&vid];
                    writeln!(self.out, "  %v{} = getelementptr inbounds i8, ptr %gcs{slot}, i64 0", vid.0).unwrap();
                }
            }
            if block.id.0 == 0 {
                // Raízes por mapas: as chamadas do prólogo (a área do módulo,
                // o estouro de pilha, as conversões dos parâmetros) podem
                // coletar, com os parâmetros enraizados vivos — todo
                // parâmetro enraizado está vivo na entrada.
                if !self.rooted_set.is_empty() {
                    let params: Vec<ValueId> = func.params.iter().map(|p| p.0).filter(|v| self.rooted_set.contains(v)).collect();
                    self.attach_deopt_with(block_start, &params, false);
                }
                // A sabotagem `bruto_no_mapa`: um valor que o otimizador não
                // dobra (a carga volátil) posto no mapa como raiz.
                if !self.rooted_set.is_empty() && crate::alvo::sabotage("bruto_no_mapa") {
                    self.out.push_str(
                        "  %dfsab = alloca i64, align 8\n  store volatile i64 4098, ptr %dfsab, align 8\n  %dfsabv = load volatile i64, ptr %dfsab, align 8\n",
                    );
                }
            }
            // `phi` tem de ser a primeira instrução do bloco: as raízes dos
            // `phi` saem todas depois do último deles.
            let mut phi_roots: Vec<(usize, u32)> = Vec::new();
            self.fresh_set.clear();

            for (vid, inst, ty) in &block.instructions {
                let v = vid.0;
                // J05: a posição do comando, quando muda (`llvm/debug_info.rs`);
                // a de uma instrução copiada pelo inlining da HIR, com o
                // contexto dela, vai num marcador só do rastro (§13.14).
                if let Some(e) = func.debug_info.as_ref().and_then(|d| d.embedded_positions.get(vid)) {
                    if written_embedded != Some(*e) {
                        writeln!(self.out, "{}{} {} {}", debug_info::EMBEDDED_MARKER, e.0, e.1, e.2).unwrap();
                        written_embedded = Some(*e);
                        written_position = None;
                    }
                } else if let Some(p) = func.debug_info.as_ref().and_then(|d| d.positions.get(vid))
                    && written_position != Some(*p)
                {
                    writeln!(self.out, "{}{} {}", debug_info::MARKER, p.0, p.1).unwrap();
                    written_position = Some(*p);
                    written_embedded = None;
                }
                if !matches!(inst, Instruction::Phi { .. }) && !phi_roots.is_empty() {
                    for (slot, pv) in std::mem::take(&mut phi_roots) {
                        self.phi_root(slot, pv);
                    }
                }
                // O build de conferência: os vivos deste ponto de coleta,
                // exatamente, nos slots de conferência.
                if self.check.is_some()
                    && let Some(live_set) = self.function_roots.live_at.get(vid).cloned()
                {
                    self.store_check(&live_set);
                }
                // Um ponto de coleta pode promover o objeto recém-alocado
                // (velho depois da coleta): deixa de ser fresco.
                if self.may_collect(inst) {
                    self.fresh_set.clear();
                }
                let instruction_start = self.out.len();
                match inst {
                    Instruction::ArcCopy { value } | Instruction::ArcMove { value } => {
                        let val_ref = self.arc_reference(value);
                        if matches!(inst, Instruction::ArcCopy { .. }) {
                            writeln!(self.out, "  call void @dartforge_arc_retain(i64 {val_ref})").unwrap();
                        }
                        writeln!(self.out, "  %v{v} = add i64 {val_ref}, 0").unwrap();
                    }
                    Instruction::ArcDrop { value } => {
                        let val_ref = self.arc_reference(value);
                        writeln!(self.out, "  call void @dartforge_arc_release(i64 {val_ref})").unwrap();
                    }
                    Instruction::ArcLoadStrong { slot: StrongSlot::Global { symbol: sym_name } } => {
                        self.check_arc_global(sym_name);
                        self.global_address(v, sym_name);
                        writeln!(self.out, "  %v{v} = load i64, ptr %ga{v}").unwrap();
                        writeln!(self.out, "  call void @dartforge_arc_retain(i64 %v{v})").unwrap();
                    }
                    Instruction::ArcStoreStrong { slot: StrongSlot::Global { symbol: sym_name }, value, mode } => {
                        self.check_arc_global(sym_name);
                        let val_ref = self.arc_reference(value);
                        self.global_address(v, sym_name);
                        if *mode == StrongStoreMode::Copy {
                            writeln!(self.out, "  call void @dartforge_arc_retain(i64 {val_ref})").unwrap();
                        }
                        writeln!(self.out, "  store i64 {val_ref}, ptr %ga{v}").unwrap();
                        writeln!(self.out, "  %gr{v} = ptrtoint ptr %ga{v} to i64").unwrap();
                        if *mode == StrongStoreMode::Move {
                            writeln!(self.out, "  call void @dartforge_arc_global_receber_v1(i64 %gr{v}, i64 {val_ref})").unwrap();
                        } else {
                            writeln!(self.out, "  call void @dartforge_gc_global_root(i64 %gr{v}, i64 {val_ref}) nounwind").unwrap();
                            writeln!(self.out, "  call void @dartforge_arc_release(i64 {val_ref})").unwrap();
                        }
                    }
                    Instruction::ArcLoadStrong { slot } => {
                        let (frame, idx_value) = self.slot_arc(slot);
                        writeln!(self.out, "  %v{v} = call i64 @dartforge_arc_quadro_carregar_v1(i64 {frame}, i64 {idx_value})").unwrap();
                    }
                    Instruction::ArcStoreStrong { slot, value, mode } => {
                        let (frame, idx_value) = self.slot_arc(slot);
                        let val_ref = self.arc_reference(value);
                        let name_str = if *mode == StrongStoreMode::Copy { "copiar" } else { "receber" };
                        writeln!(self.out, "  call void @dartforge_arc_quadro_{name_str}_v1(i64 {frame}, i64 {idx_value}, i64 {val_ref})").unwrap();
                    }
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
                    Instruction::Const(Constant::String(s)) => self.emit_const_string(v, s.as_bytes()),
                    Instruction::Const(Constant::StringWtf8(s)) => self.emit_const_string(v, s),
                    Instruction::Add(a, b) => {
                        let sa = self.coerce(a, Type::I64);
                        let sb = self.coerce(b, Type::I64);
                        writeln!(self.out, "  %v{v} = add i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::Sub(a, b) => {
                        let sa = self.coerce(a, Type::I64);
                        let sb = self.coerce(b, Type::I64);
                        writeln!(self.out, "  %v{v} = sub i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::Mul(a, b) => {
                        let sa = self.coerce(a, Type::I64);
                        let sb = self.coerce(b, Type::I64);
                        writeln!(self.out, "  %v{v} = mul i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::SDiv(a, b) => {
                        let sa = self.coerce(a, Type::I64);
                        let sb = self.coerce(b, Type::I64);
                        writeln!(self.out, "  %v{v} = sdiv i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::SRem(a, b) => {
                        let sa = self.coerce(a, Type::I64);
                        let sb = self.coerce(b, Type::I64);
                        writeln!(self.out, "  %v{v} = srem i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::Shl(a, b) => {
                        let sa = self.coerce(a, Type::I64);
                        let sb = self.coerce(b, Type::I64);
                        writeln!(self.out, "  %v{v} = shl i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::AShr(a, b) => {
                        let sa = self.coerce(a, Type::I64);
                        let sb = self.coerce(b, Type::I64);
                        writeln!(self.out, "  %v{v} = ashr i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::And(a, b) => {
                        let sa = self.coerce(a, Type::I64);
                        let sb = self.coerce(b, Type::I64);
                        writeln!(self.out, "  %v{v} = and i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::Or(a, b) => {
                        let sa = self.coerce(a, Type::I64);
                        let sb = self.coerce(b, Type::I64);
                        writeln!(self.out, "  %v{v} = or i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::Xor(a, b) => {
                        let sa = self.coerce(a, Type::I64);
                        let sb = self.coerce(b, Type::I64);
                        writeln!(self.out, "  %v{v} = xor i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::Neg(a) => {
                        let sa = self.coerce(a, Type::I64);
                        writeln!(self.out, "  %v{v} = sub i64 0, {sa}").unwrap();
                    }
                    Instruction::Not(a) => {
                        let sa = self.coerce(a, Type::I64);
                        writeln!(self.out, "  %v{v} = xor i64 {sa}, -1").unwrap();
                    }
                    Instruction::FAdd(a, b) => {
                        let sa = self.coerce(a, Type::F64);
                        let sb = self.coerce(b, Type::F64);
                        writeln!(self.out, "  %v{v} = fadd double {sa}, {sb}").unwrap();
                    }
                    Instruction::FSub(a, b) => {
                        let sa = self.coerce(a, Type::F64);
                        let sb = self.coerce(b, Type::F64);
                        writeln!(self.out, "  %v{v} = fsub double {sa}, {sb}").unwrap();
                    }
                    Instruction::FMul(a, b) => {
                        let sa = self.coerce(a, Type::F64);
                        let sb = self.coerce(b, Type::F64);
                        writeln!(self.out, "  %v{v} = fmul double {sa}, {sb}").unwrap();
                    }
                    Instruction::FDiv(a, b) => {
                        let sa = self.coerce(a, Type::F64);
                        let sb = self.coerce(b, Type::F64);
                        writeln!(self.out, "  %v{v} = fdiv double {sa}, {sb}").unwrap();
                    }
                    Instruction::FNeg(a) => {
                        let sa = self.coerce(a, Type::F64);
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
                        let sa = self.coerce(a, Type::I64);
                        let sb = self.coerce(b, Type::I64);
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
                        let sa = self.coerce(a, Type::F64);
                        let sb = self.coerce(b, Type::F64);
                        writeln!(self.out, "  %v{v} = fcmp {op_str} double {sa}, {sb}").unwrap();
                    }
                    Instruction::LNot(a) => {
                        let sa = self.coerce(a, Type::I1);
                        writeln!(self.out, "  %v{v} = xor i1 {sa}, true").unwrap();
                    }
                    Instruction::IntToDouble(a) => {
                        let sa = self.coerce(a, Type::I64);
                        writeln!(self.out, "  %v{v} = sitofp i64 {sa} to double").unwrap();
                    }
                    Instruction::DoubleToInt(a) => {
                        let sa = self.coerce(a, Type::F64);
                        writeln!(self.out, "  %v{v} = fptosi double {sa} to i64").unwrap();
                    }
                    Instruction::ZExt { op, from, to } => {
                        let sop = self.coerce(op, *from);
                        let f = from.llvm_ir();
                        let t = to.llvm_ir();
                        writeln!(self.out, "  %v{v} = zext {f} {sop} to {t}").unwrap();
                    }
                    Instruction::Trunc { op, from, to } => {
                        let sop = self.coerce(op, *from);
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
                            self.emit_field_addresses_of(v, &format!("%v{v}"), !self.module.arc_memory);
                        }
                        for (idx, field) in fields.iter().enumerate() {
                            // E1: `is_ref` pela representação do valor.
                            let is_ref = u8::from(self.type_of(field) == Type::Ref).to_string();
                            let sf = self.coerce(field, Type::I64);
                            self.emit_field_write(v, &format!(".{idx}"), idx, &sf, &is_ref);
                        }
                        // No ARC, os campos gravados em linha contam.
                        if fields.iter().any(|f| self.type_of(f) == Type::Ref) {
                            self.count_initial_in_arc(v);
                        }
                    }
                    // O campo em linha: a palavra de 8 bytes depois do
                    // cabeçalho do corpo do objeto (`heap::Cabecalho`).
                    Instruction::GetField { object, index } => {
                        let only = self.coerce(object, Type::I64);
                        let bits = if matches!(ty, Type::Ref | Type::I64) {
                            format!("%v{v}")
                        } else { format!("%fb{v}") };
                        if *index < INLINE_FIELDS {
                            let direct = self.direct_body(object);
                            self.emit_field_addresses_of(v, &only, direct);
                            writeln!(self.out, "  %fg{v} = getelementptr inbounds i64, ptr %fp{v}, i64 {index}").unwrap();
                            writeln!(self.out, "  {bits} = load i64, ptr %fg{v}, align 8").unwrap();
                        } else {
                            writeln!(self.out, "  {bits} = call i64 @dartforge_object_get(i64 {only}, i64 {index})").unwrap();
                        }
                        if !matches!(ty, Type::Ref | Type::I64) {
                            self.emit_named_conversion(&format!("%v{v}"), Type::I64, &bits, *ty);
                        }
                    }
                    // No ARC a gravação vai ao runtime (`dartforge_object_set`), que conta a troca.
                    Instruction::SetField { object, index, value } if (*index as usize) < INLINE_FIELDS && !self.module.arc_memory => {
                        let is_ref = u8::from(self.type_of(value) == Type::Ref);
                        let only = self.coerce(object, Type::I64);
                        let sv = self.coerce(value, Type::I64);
                        let fresh = self.fresh(object);
                        let direct = self.direct_body(object);
                        self.emit_field_addresses_of(v, &only, direct);
                        self.emit_field_write(v, "", *index, &sv, &is_ref.to_string());
                        if self.inline_barrier(inst) && !fresh {
                            self.emit_barrier(v, &only, &sv, None);
                        }
                    }
                    Instruction::SetField { object, index, value } => {
                        let is_ref = u8::from(self.type_of(value) == Type::Ref);
                        let only = self.coerce(object, Type::I64);
                        let sv = self.coerce(value, Type::I64);
                        writeln!(
                            self.out,
                            "  call void @dartforge_object_set(i64 {only}, i64 {index}, i64 {sv}, i8 {is_ref})"
                        ).unwrap();
                    }
                    Instruction::CallStatic { symbol, args, ret_ty } => {
                        let target_func = self.function_by_symbol.get(symbol.as_str()).copied();
                        let mut args_str: Vec<String> = Vec::with_capacity(args.len());
                        let mut args_types = Vec::with_capacity(args.len());
                        for (idx, a) in args.iter().enumerate() {
                            // Função de outro módulo (SDK da fonte): o tipo do
                            // operando, que o lowering já coagiu para a
                            // representação do parâmetro.
                            let alvo = target_func
                                .and_then(|f| f.params.get(idx))
                                .map_or_else(|| self.type_of(a), |p| p.2);
                            let alvo = if alvo == Type::Void { Type::I64 } else { alvo };
                            let s = self.coerce(a, alvo);
                            args_str.push(format!("{} {s}", alvo.llvm_ir()));
                            args_types.push(alvo);
                        }
                        if target_func.is_none() {
                            self.annotate_external(symbol, *ret_ty, &args_types);
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
                            && matches!(args.get(1), Some((Operand::Constant(Constant::Int(i)), _)) if (0..INLINE_FIELDS as i64).contains(i)) =>
                    {
                        let Some((Operand::Constant(Constant::Int(i)), _)) = args.get(1) else { unreachable!() };
                        let only = self.coerce(&args[0].0, Type::I64);
                        let direct = self.direct_body(&args[0].0);
                        self.emit_field_addresses_of(v, &only, direct);
                        writeln!(self.out, "  %fg{v} = getelementptr inbounds i64, ptr %fp{v}, i64 {i}").unwrap();
                        writeln!(self.out, "  %v{v} = load i64, ptr %fg{v}, align 8").unwrap();
                    }
                    Instruction::CallRuntime { name, args, .. }
                        if name == "dartforge_object_set"
                            && !self.module.arc_memory
                            && args.len() == 4
                            && matches!(args.get(1), Some((Operand::Constant(Constant::Int(i)), _)) if (0..INLINE_FIELDS as i64).contains(i)) =>
                    {
                        let Some((Operand::Constant(Constant::Int(i)), _)) = args.get(1) else { unreachable!() };
                        let only = self.coerce(&args[0].0, Type::I64);
                        let sv = self.coerce(&args[2].0, Type::I64);
                        // `is_ref` é um `bool` do Rust: só 0 ou 1.
                        let is_ref = match &args[3].0 {
                            Operand::Constant(Constant::Int(n)) => u8::from(*n != 0).to_string(),
                            Operand::Constant(Constant::Bool(b)) => u8::from(*b).to_string(),
                            other => {
                                let x = self.coerce(other, Type::I64);
                                writeln!(self.out, "  %fb{v} = icmp ne i64 {x}, 0").unwrap();
                                writeln!(self.out, "  %fz{v} = zext i1 %fb{v} to i8").unwrap();
                                format!("%fz{v}")
                            }
                        };
                        let fresh = self.fresh(&args[0].0);
                        let direct = self.direct_body(&args[0].0);
                        self.emit_field_addresses_of(v, &only, direct);
                        self.emit_field_write(v, "", *i as usize, &sv, &is_ref);
                        if self.inline_barrier(inst) && !fresh {
                            let dynamic = is_ref.starts_with('%').then_some(is_ref.as_str());
                            self.emit_barrier(v, &only, &sv, dynamic);
                        }
                    }
                    Instruction::CallRuntime { .. } if Self::inline_allocation(inst).is_some() => {
                        let (c, n) = Self::inline_allocation(inst).expect("conferido na guarda");
                        self.emit_inline_allocation(v, c, n);
                    }
                    Instruction::CallRuntime { name, args, ret_ty }
                        if (name == "dartforge_object_new" || name == "dartforge_arc_objeto_owned_v1")
                            && matches!(args.first(), Some((Operand::Constant(Constant::Int(c)), _))
                                if self.module.table_functions.contains_key(&(*c as u32))) =>
                    {
                        // SDK da fonte: a primeira alocação registra a tabela
                        // de métodos da classe (`selectors.rs`).
                        let Some((Operand::Constant(Constant::Int(c)), _)) = args.first() else { unreachable!() };
                        let f = self.module.table_functions[&(*c as u32)].clone();
                        self.annotate_external(&f, Type::Ptr, &[]);
                        let n = self.coerce(&args[1].0, Type::I64);
                        let factory = if name == "dartforge_arc_objeto_owned_v1" {
                            "dartforge_arc_objeto_owned_t_v1"
                        } else { "dartforge_object_new_t" };
                        writeln!(self.out, "  %v{v} = call i64 @{factory}(i64 {c}, i64 {n}, ptr @{f})").unwrap();
                        let _ = ret_ty;
                    }
                    Instruction::CallRuntime { name, args, ret_ty }
                        if (name == "dartforge_typed_novo" || name == "dartforge_view_nova" || name == "dartforge_typed_externo")
                            && matches!(args.first(), Some((Operand::Constant(Constant::Int(c)), _))
                                if self.module.table_functions.contains_key(&(*c as u32))) =>
                    {
                        // A primeira lista tipada (ou visão) de uma classe
                        // registra a tabela de métodos dela, como
                        // `dartforge_object_new_t`.
                        let Some((Operand::Constant(Constant::Int(c)), _)) = args.first() else { unreachable!() };
                        let f = self.module.table_functions[&(*c as u32)].clone();
                        self.annotate_external(&f, Type::Ptr, &[]);
                        let rest: Vec<String> = args.iter().map(|(a, t)| format!("{} {}", t.llvm_ir(), self.coerce(a, *t))).collect();
                        writeln!(self.out, "  %v{v} = call i64 @{name}_t({}, ptr @{f})", rest.join(", ")).unwrap();
                        let _ = ret_ty;
                    }
                    // O mesmo pelo thunk Owned genérico (`optimize::arc::natives`):
                    // o thunk da variante `_t`, que registra a tabela.
                    Instruction::CallRuntime { name, args, ret_ty }
                        if name.strip_prefix(crate::optimize::arc::GENERIC_OWNED_PREFIX).is_some_and(|b|
                            matches!(b, "dartforge_typed_novo" | "dartforge_view_nova" | "dartforge_typed_externo"))
                            && matches!(args.first(), Some((Operand::Constant(Constant::Int(c)), _))
                                if self.module.table_functions.contains_key(&(*c as u32))) =>
                    {
                        let Some((Operand::Constant(Constant::Int(c)), _)) = args.first() else { unreachable!() };
                        let f = self.module.table_functions[&(*c as u32)].clone();
                        self.annotate_external(&f, Type::Ptr, &[]);
                        let base = format!("{}_t", &name[crate::optimize::arc::GENERIC_OWNED_PREFIX.len()..]);
                        let thunk = format!("{}{base}", crate::optimize::arc::GENERIC_OWNED_PREFIX);
                        let mut types: Vec<Type> = args.iter().map(|(_, t)| *t).collect();
                        types.push(Type::Ptr);
                        self.owned_thunks.insert(thunk.clone(), (base, types));
                        let rest: Vec<String> = args.iter().map(|(a, t)| format!("{} {}", t.llvm_ir(), self.coerce(a, *t))).collect();
                        writeln!(self.out, "  %v{v} = call i64 @{thunk}({}, ptr @{f})", rest.join(", ")).unwrap();
                        let _ = ret_ty;
                    }
                    // A exceção pendente: o espelho no contexto da thread.
                    Instruction::CallRuntime { name, .. } if name == "dartforge_exception_pending" && self.has_ctx => {
                        writeln!(self.out, "  %v{v} = load i8, ptr %ctx, align 8").unwrap();
                    }
                    // O `dartforge_exception_clear` de cada `return` e salto:
                    // sem exceção pendente (o espelho no contexto), não há o
                    // que limpar — a entrada do `catch` e o `finally` já
                    // limparam o rastro junto com a exceção. Só a pendente
                    // (o `finally` que sai por `return`) chama o runtime.
                    Instruction::CallRuntime { name, .. } if name == "dartforge_exception_clear" && self.has_ctx => {
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
                        self.current_label = format!("xc{v}.fim");
                    }
                    // O pedido de interrupção (J01): o byte no contexto
                    // (deslocamento 32), lido atômico — outra thread o liga, e
                    // a carga não pode sair do laço.
                    Instruction::CallRuntime { name, .. } if name == "dartforge_interrupcao_pendente" && self.has_ctx => {
                        writeln!(self.out, "  %ip{v} = getelementptr inbounds i8, ptr %ctx, i64 32").unwrap();
                        writeln!(self.out, "  %iv{v} = load atomic i8, ptr %ip{v} monotonic, align 8").unwrap();
                        // O pedido é raro: o caminho lento fica fora do corpo
                        // do laço.
                        writeln!(self.out, "  %v{v} = call i8 @llvm.expect.i8(i8 %iv{v}, i8 0)").unwrap();
                    }
                    Instruction::CallRuntime { args, .. } if Self::rti_evaluate_cached(inst) => {
                        self.emit_rti_evaluate_cached(v, args);
                    }
                    // A classe do receptor: a do objeto do espaço lida no
                    // cabeçalho, em linha (`df.class`); o resto, o runtime.
                    Instruction::CallRuntime { name, args, .. } if name == "dartforge_value_class" && args.len() == 1 => {
                        let h = self.coerce(&args[0].0, Type::I64);
                        writeln!(self.out, "  %v{v} = call i64 @df.classe(i64 {h})").unwrap();
                    }
                    // `cid <: C` pelo mapa de bits do alvo (`df.subclasse`),
                    // sem chamada depois da primeira consulta.
                    Instruction::CallRuntime { name, args, .. } if name == "dartforge_is_subclass" && args.len() == 2 => {
                        let a = self.coerce(&args[0].0, Type::I64);
                        let b = self.coerce(&args[1].0, Type::I64);
                        writeln!(self.out, "  %v{v} = call i8 @df.subclasse(i64 {a}, i64 {b})").unwrap();
                    }
                    Instruction::CallRuntime { name, args, ret_ty } => {
                        if name.starts_with("dartforge_nativo_") {
                            let types: Vec<Type> = args.iter().map(|(_, t)| *t).collect();
                            self.annotate_external(name, *ret_ty, &types);
                        }
                        // O código do tear-off pode morar em outro módulo.
                        if name == "dartforge_arc_tearoff_owned_v1"
                            && let [(Operand::Constant(Constant::Function(code)), _)] = args.as_slice()
                        {
                            self.annotate_external(code, Type::Ref, &[Type::Ref, Type::Ptr, Type::Ptr]);
                        }
                        if let Some(base) = name.strip_prefix(crate::optimize::arc::GENERIC_OWNED_PREFIX) {
                            let types: Vec<Type> = args.iter().map(|(_, t)| *t).collect();
                            if base.starts_with("dartforge_nativo_") {
                                self.annotate_external(base, Type::Ref, &types);
                            }
                            self.owned_thunks.insert(name.clone(), (base.to_string(), types));
                        }
                        if let Some(base) = name.strip_prefix(crate::optimize::arc::NATIVE_SCOPE_PREFIX) {
                            let types: Vec<Type> = args.iter().map(|(_, t)| *t).collect();
                            if base.starts_with("dartforge_nativo_") {
                                self.annotate_external(base, *ret_ty, &types);
                            }
                            self.scope_thunks.insert(name.clone(), (base.to_string(), types, *ret_ty));
                        }
                        let mut args_formatted = Vec::new();
                        for (a, ty) in args {
                            let s = self.coerce(a, *ty);
                            let t = ty.llvm_ir();
                            args_formatted.push(format!("{t} {s}"));
                        }
                        let joined = args_formatted.join(", ");
                        let r = ret_ty.llvm_ir();
                        // `DARTFORGE_EFEITOS=check`: a extern marcada sem
                        // coletar ou sem lançar roda vigiada pelo runtime.
                        let marks = if externs::check_effects() { externs::marks_to_check(name) } else { 0 };
                        if marks != 0 {
                            let k = match self.checked_externs.iter().position(|n| n == name) {
                                Some(k) => k,
                                None => {
                                    self.checked_externs.push(name.clone());
                                    self.checked_externs.len() - 1
                                }
                            };
                            writeln!(self.out, "  call void @dartforge_efeitos_antes(ptr @df.efn.{k}, i64 {}, i64 {marks})", name.len()).unwrap();
                        }
                        if *ret_ty == Type::Void {
                            writeln!(self.out, "  call {r} @{name}({joined})").unwrap();
                        } else {
                            writeln!(self.out, "  %v{v} = call {r} @{name}({joined})").unwrap();
                        }
                        if marks != 0 {
                            writeln!(self.out, "  call void @dartforge_efeitos_depois()").unwrap();
                        }
                    }
                    Instruction::NativeCall { alvo, args, ret } => {
                        // A chamada C: cada argumento convertido ao tipo C
                        // (estreitos com a extensão da ABI), o retorno
                        // estendido de volta explicitamente (o Windows x64
                        // não garante os bits altos de um retorno estreito).
                        let a = self.coerce(alvo, Type::I64);
                        writeln!(self.out, "  %fn{v} = inttoptr i64 {a} to ptr").unwrap();
                        let mut parts = Vec::with_capacity(args.len());
                        for (i, (op, tc)) in args.iter().enumerate() {
                            parts.push(self.c_argument(v, i, op, *tc));
                        }
                        let list = parts.join(", ");
                        let conv = match ret {
                            CType::I8 | CType::I16 | CType::I32 => Some(format!("sext {} %nr{v} to i64", ret.llvm())),
                            CType::U8 | CType::U16 | CType::U32 => Some(format!("zext {} %nr{v} to i64", ret.llvm())),
                            CType::F32 => Some(format!("fpext float %nr{v} to double")),
                            CType::Ptr | CType::Handle => Some(format!("ptrtoint ptr %nr{v} to i64")),
                            CType::I64 | CType::U64 | CType::F64 | CType::Bool | CType::Void => None,
                        };
                        match (ret, conv) {
                            (CType::Void, _) => writeln!(self.out, "  call void %fn{v}({list})").unwrap(),
                            (_, None) => writeln!(self.out, "  %v{v} = call {} %fn{v}({list})", ret.llvm()).unwrap(),
                            (_, Some(c)) => {
                                writeln!(self.out, "  %nr{v} = call {} %fn{v}({list})", ret.llvm()).unwrap();
                                writeln!(self.out, "  %v{v} = {c}").unwrap();
                            }
                        }
                    }
                    Instruction::NativeLoad { address, index: idx_value, ty: typ } => {
                        // Sem alinhamento suposto (os bytes de uma lista
                        // tipada são de um `Vec<u8>`).
                        let e = self.coerce(address, Type::I64);
                        let i = self.coerce(idx_value, Type::I64);
                        let t = typ.llvm();
                        let inv = "";
                        writeln!(self.out, "  %cp{v} = inttoptr i64 {e} to ptr").unwrap();
                        writeln!(self.out, "  %cg{v} = getelementptr {t}, ptr %cp{v}, i64 {i}").unwrap();
                        let conv = match typ {
                            CType::I8 | CType::I16 | CType::I32 => Some(format!("sext {t} %cl{v} to i64")),
                            CType::U8 | CType::U16 | CType::U32 => Some(format!("zext {t} %cl{v} to i64")),
                            CType::F32 => Some(format!("fpext float %cl{v} to double")),
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
                    Instruction::NativeWrite { address, index: idx_value, ty: typ, value: val_ref } => {
                        let e = self.coerce(address, Type::I64);
                        let i = self.coerce(idx_value, Type::I64);
                        let t = typ.llvm();
                        let x = match typ {
                            CType::F32 => {
                                let d = self.coerce(val_ref, Type::F64);
                                writeln!(self.out, "  %gx{v} = fptrunc double {d} to float").unwrap();
                                format!("%gx{v}")
                            }
                            CType::F64 => self.coerce(val_ref, Type::F64),
                            CType::I64 | CType::U64 => self.coerce(val_ref, Type::I64),
                            _ => {
                                let n = self.coerce(val_ref, Type::I64);
                                writeln!(self.out, "  %gx{v} = trunc i64 {n} to {t}").unwrap();
                                format!("%gx{v}")
                            }
                        };
                        writeln!(self.out, "  %gp{v} = inttoptr i64 {e} to ptr").unwrap();
                        writeln!(self.out, "  %gg{v} = getelementptr {t}, ptr %gp{v}, i64 {i}").unwrap();
                        writeln!(self.out, "  store {t} {x}, ptr %gg{v}, align 1").unwrap();
                    }
                    Instruction::CompositeNativeCall { alvo, args, ret, dest, variadic_flag } => {
                        self.composite_native_call(v, alvo, args, ret, dest.as_ref(), *variadic_flag);
                    }
                    Instruction::AllocList { elements } => self.emit_alloc_list(v, elements),
                    Instruction::AllocRecord { elements } => self.emit_alloc_record(v, elements),
                    // O `alloca` nasce no bloco de entrada (`emit_closure_buffers`).
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
                        // uma função local direta, `direct_functions.rs`)
                        // grava na largura do valor, a do local de lá: um
                        // `bool` gravado como `i64` pisava 7 bytes do quadro
                        // de quem chamou (o `listenerHasError` do
                        // `_Future._propagateToListeners`).
                        let t = match ptr {
                            Operand::Val(p) => self.pointee.get(p).copied().unwrap_or_else(|| match self.type_of(val) {
                                Type::I1 => Type::I1,
                                Type::I8 => Type::I8,
                                Type::F64 => Type::F64,
                                // Local SIMD sem caixa de quem chama (16 bytes).
                                t if t.is_vector() => t,
                                _ => Type::I64,
                            }),
                            _ => Type::I64,
                        };
                        let sp = self.operand_str(ptr);
                        let sv = self.coerce(val, t);
                        writeln!(self.out, "  store {} {sv}, ptr {sp}", t.llvm_ir()).unwrap();
                        // G2: o local `Ref` tem slot próprio, atualizado a
                        // cada gravação — ele vive mais que o SSA que o gravou.
                        if let Operand::Val(pv) = ptr
                            && !self.frame_allocas.contains(pv)
                            && let Some(&slot) = self.slots.get(pv)
                        {
                            writeln!(self.out, "  store i64 {sv}, ptr %gcs{slot}").unwrap();
                        }
                    }
                    Instruction::Box { op, from } if from.is_vector() => self.emit_simd_box(v, op, *from),
                    Instruction::Unbox { op, to } if to.is_vector() => self.emit_simd_unbox(v, op, *to),
                    Instruction::Simd { op, args } => {
                        let ty = self.types.get(vid).copied().unwrap_or(Type::Void);
                        self.emit_simd(v, *op, args, ty);
                    }
                    Instruction::Box { op, from } => self.emit_box(v, op, *from),
                    Instruction::Unbox { op, to } => self.emit_unbox(v, op, *to),
                    Instruction::LShr(a, b) => {
                        let sa = self.coerce(a, Type::I64);
                        let sb = self.coerce(b, Type::I64);
                        writeln!(self.out, "  %v{v} = lshr i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::Bitcast { op, to } => {
                        if *to == Type::F64 {
                            let only = self.coerce(op, Type::I64);
                            writeln!(self.out, "  %v{v} = bitcast i64 {only} to double").unwrap();
                        } else {
                            let only = self.coerce(op, Type::F64);
                            writeln!(self.out, "  %v{v} = bitcast double {only} to i64").unwrap();
                        }
                    }
                    Instruction::LoadGlobal { symbol: sym_name, ty } => {
                        self.global_address(v, sym_name);
                        writeln!(self.out, "  %v{v} = load {}, ptr %ga{v}", ty.llvm_ir()).unwrap();
                    }
                    Instruction::StoreGlobal { symbol: sym_name, val, ty, root } => {
                        let sv = self.coerce(val, *ty);
                        self.global_address(v, sym_name);
                        writeln!(self.out, "  store {} {sv}, ptr %ga{v}", ty.llvm_ir()).unwrap();
                        if root.is_some() {
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
                                    .constant_in_type(op, *ty)
                                    .or_else(|| match op {
                                        Operand::Val(v) => self
                                            .conv_phi
                                            .iter()
                                            .find(|(bb, _, _, vv, to_repr)| {
                                                *bb == b.0 && vv == v && to_repr.llvm_ir() == ty.llvm_ir()
                                            })
                                            .map(|(_, name_str, _, _, _)| name_str.clone()),
                                        _ => None,
                                    })
                                    .unwrap_or_else(|| self.operand_str(op));
                                match self.exit_labels.get(&b.0) {
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
                if let Some(landing_pad) = tab.and_then(|t| t.invocations.get(vid)) {
                    self.make_invoke(instruction_start, v, landing_pad.0);
                }
                // G1: a raiz logo depois da definição — nada aloca entre
                // o retorno da chamada e este `set_root` (G5).
                if let Some(&slot) = self.slots.get(vid) {
                    if matches!(inst, Instruction::Phi { .. }) {
                        phi_roots.push((slot, v));
                    } else if !matches!(inst, Instruction::Alloca(_)) {
                        writeln!(self.out, "  store i64 %v{v}, ptr %gcs{slot}").unwrap();
                    }
                }
                // Raízes por mapas: o que estava vivo na entrada deste ponto
                // de coleta (os operandos inclusive) vai no `"deopt"` de cada
                // chamada dele que pode coletar — é o que o põe no mapa da
                // chamada (§14.12).
                if !self.rooted_set.is_empty()
                    && let Some(live_set) = self.function_roots.live_at.get(vid).cloned()
                {
                    self.attach_deopt(instruction_start, &live_set);
                }
                if !self.module.arc_memory
                    && (matches!(inst, Instruction::AllocObject { .. }) || Self::inline_allocation(inst).is_some()
                        || matches!(inst, Instruction::CallRuntime { name, .. } if name == "dartforge_object_new"))
                {
                    self.fresh_set.insert(v);
                }
                let _ = ty;
            }
            for (slot, pv) in std::mem::take(&mut phi_roots) {
                self.phi_root(slot, pv);
            }

            // O build de conferência: o ponto de coleta do fim do bloco.
            if self.check.is_some() {
                let live_set = self.function_roots.live_at_end.get(&block.id).cloned().unwrap_or_default();
                self.store_check(&live_set);
            }
            let end_start = self.out.len();
            for (b, name_str, de, v, to_repr) in self.conv_phi.clone() {
                if b == block.id.0 {
                    let origin = format!("%v{}", v.0);
                    self.emit_named_conversion(&name_str, de, &origin, to_repr);
                }
            }
            // Raízes por mapas: o ponto de coleta do fim do bloco (as
            // conversões acima podem encaixotar; o `throw` é tratado no
            // terminador, depois da chamada dele).
            if !self.rooted_set.is_empty()
                && !matches!(block.terminator, Terminator::Throw(_))
                && let Some(live_set) = self.function_roots.live_at_end.get(&block.id).cloned()
            {
                self.attach_deopt(end_start, &live_set);
            }

            if let Some(p) = func.debug_info.as_ref().and_then(|d| d.exits.get(&block.id))
                && written_position != Some(*p)
            {
                writeln!(self.out, "{}{} {}", debug_info::MARKER, p.0, p.1).unwrap();
                written_position = Some(*p);
                written_embedded = None;
            }
            // Exceções por tabelas: o `Return` com a exceção pendente não
            // retorna — desenrola até o pouso de quem a trata (que restaura
            // o topo da pilha-sombra: o quadro de raízes não é fechado
            // aqui). Com a pendência incerta, confere antes.
            if matches!(block.terminator, Terminator::Return(_)) {
                match tab.and_then(|t| t.exits.get(&block.id)) {
                    Some(ExceptionExit::Throws) => {
                        writeln!(self.out, "  call void @df.lancar()").unwrap();
                        writeln!(self.out, "  unreachable").unwrap();
                        continue;
                    }
                    Some(ExceptionExit::Resume) => {
                        if self.stack_trace.is_some() {
                            writeln!(self.out, "  call void @dartforge_rastro_saida()").unwrap();
                        }
                        if self.has_frame {
                            writeln!(self.out, "  store ptr %gcant, ptr %ctxtopo, align 8").unwrap();
                        }
                        writeln!(self.out, "  resume {{ ptr, i32 }} %lpad{}", block.id.0).unwrap();
                        continue;
                    }
                    Some(ExceptionExit::Guard) => {
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
            if self.stack_trace.is_some() && matches!(block.terminator, Terminator::Return(_)) {
                writeln!(self.out, "  call void @dartforge_rastro_saida()").unwrap();
            }
            // G3: o frame de raízes fecha antes de TODO `ret`, inclusive o
            // das saídas por exceção (que retornam o valor padrão).
            if self.has_frame && matches!(block.terminator, Terminator::Return(_)) {
                writeln!(self.out, "  %gcvolta{} = load ptr, ptr %gcq, align 8", block.id.0).unwrap();
                writeln!(self.out, "  store ptr %gcvolta{}, ptr %ctxtopo, align 8", block.id.0).unwrap();
            }
            match &block.terminator {
                Terminator::Return(Some(op)) => {
                    if func.return_ty == Type::Void {
                        writeln!(self.out, "  ret void").unwrap();
                    } else {
                        let sop = self.coerce(op, func.return_ty);
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
                // (`optimize/tables.rs`): aqui só segue a continuação.
                Terminator::CondBranch { then_block, else_block, .. } if tab.is_some_and(|t| t.landing_pads.contains(then_block)) => {
                    writeln!(self.out, "  br label %b{}", else_block.0).unwrap();
                }
                Terminator::CondBranch { cond, then_block, else_block } => {
                    let sc = self.coerce(cond, Type::I1);
                    writeln!(self.out, "  br i1 {sc}, label %b{}, label %b{}", then_block.0, else_block.0).unwrap();
                }
                Terminator::Switch { val, default, cases } => {
                    let sv = self.coerce(val, Type::I64);
                    write!(self.out, "  switch i64 {sv}, label %b{} [", default.0).unwrap();
                    for (c, b) in cases {
                        write!(self.out, " i64 {c}, label %b{}", b.0).unwrap();
                    }
                    writeln!(self.out, " ]").unwrap();
                }
                Terminator::Throw(op) => {
                    let tag = match self.type_of(op) {
                        Type::I64 => 1,
                        Type::I1 | Type::I8 => 2,
                        Type::F64 => 4,
                        _ => 3,
                    };
                    let sop = self.coerce(op, Type::I64);
                    let throw_start = self.out.len();
                    writeln!(self.out, "  call void @dartforge_exception_throw(i64 {sop}, i8 {tag})").unwrap();
                    if !self.rooted_set.is_empty()
                        && let Some(live_set) = self.function_roots.live_at_end.get(&block.id).cloned()
                    {
                        self.attach_deopt(throw_start, &live_set);
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
        if (func.debug_info.is_some() || shared)
            && let Some(mut r) = self.vm_stack_trace.take()
        {
            let text = self.out.split_off(function_start);
            let labeled = r.label(&text, func, func.debug_info.as_deref(), self.debug_info.is_some(), shared);
            self.out.push_str(&labeled);
            self.vm_stack_trace = Some(r);
        }
        if let Some(d) = func.debug_info.as_deref()
            && let Some(mut dep) = self.debug_info.take()
        {
            let text = self.out.split_off(function_start);
            let annotated = dep.annotate(&text, func, d);
            self.out.push_str(&annotated);
            self.debug_info = Some(dep);
        }
    }

    /// O número de slots do quadro de raízes da função em emissão: os de
    /// sempre e, no build de conferência, os de conferência depois deles.
    fn frame_size(&self) -> usize {
        match self.check {
            Some((first_one, k)) => first_one + k,
            None => self.slots.values().max().map_or(0, |m| m + 1),
        }
    }

    /// O campo `n` do quadro (`QuadroDeRaizes` do runtime): o número de
    /// slots e, nos bits 40 em diante, quantos dos últimos são de
    /// conferência.
    fn frame_header(&self, n: usize) -> u64 {
        // O bit 62: o quadro está num quadro nativo próprio
        // ([`Self::ordered_frames`]).
        let order = if self.ordered_frames { 1u64 << 62 } else { 0 };
        match self.check {
            Some((_, k)) => n as u64 | ((k as u64) << 40) | order,
            None => n as u64 | order,
        }
    }

    /// O build de conferência: grava `live_set` nos slots de conferência e zera
    /// os que sobram.
    fn store_check(&mut self, live_set: &[ValueId]) {
        let Some((first_one, k)) = self.check else { return };
        for i in 0..k {
            match live_set.get(i) {
                Some(x) => writeln!(self.out, "  store i64 %v{}, ptr %gcs{}", x.0, first_one + i).unwrap(),
                None => writeln!(self.out, "  store i64 0, ptr %gcs{}", first_one + i).unwrap(),
            }
        }
    }

    /// A raiz de um `phi`, emitida depois do último `phi` do bloco: o `store`
    /// no slot do quadro.
    fn phi_root(&mut self, slot: usize, v: u32) {
        writeln!(self.out, "  store i64 %v{v}, ptr %gcs{slot}").unwrap();
    }

    /// Raízes por mapas (§14.12): os valores enraizados `live_set` no operando
    /// `"deopt"` de cada chamada que pode coletar emitida desde `start` (o
    /// texto de um ponto de coleta). O operando faz o valor estar vivo *na*
    /// chamada e põe o lugar dele no mapa; vale também para o argumento cujo
    /// último uso é a própria chamada (o runtime conta com quem chama para
    /// mantê-lo vivo enquanto ela roda: o LLVM o guarda num registrador
    /// preservado ou num slot).
    fn attach_deopt(&mut self, start: usize, live_set: &[ValueId]) {
        self.attach_deopt_with(start, live_set, true);
    }

    /// [`Self::attach_deopt`]; `sabotageable` diz se o valor da sabotagem
    /// `bruto_no_mapa` (definido depois do prólogo) já existe aqui.
    fn attach_deopt_with(&mut self, start: usize, live_set: &[ValueId], sabotageable: bool) {
        let mut operands: Vec<String> = if crate::alvo::sabotage("sem_uso_ficticio") {
            Vec::new()
        } else {
            live_set.iter().map(|x| format!("i64 %v{}", x.0)).collect()
        };
        if sabotageable && crate::alvo::sabotage("bruto_no_mapa") {
            operands.push("i64 %dfsabv".to_string());
        }
        let beam = format!("\"deopt\"({})", operands.join(", "));
        let text = self.out.split_off(start);
        for line in text.split_inclusive('\n') {
            self.out.push_str(&with_deopt(line, &beam));
        }
    }

    /// Exceções por tabelas: a chamada Dart que a instrução `v` acabou de
    /// emitir (a última linha desde `start`: a de `CallStatic`,
    /// `TypedCall`, da closure ou do seletor) vira
    /// `invoke … to label %inv<v>.end unwind label %b<landing_pad>`, e a emissão
    /// continua no rótulo do caminho normal — o bloco da HIR passa a
    /// terminar nele ([`LlvmEmitter::exit_labels`]). As chamadas ao
    /// runtime que a mesma instrução emite antes (a entrada da closure, a
    /// busca do seletor) não desenrolam e continuam `call`.
    fn make_invoke(&mut self, start: usize, v: u32, landing_pad: u32) {
        let text = self.out.split_off(start);
        let body = text.strip_suffix('\n').unwrap_or(&text);
        let (before, call) = match body.rfind('\n') {
            Some(i) => (&body[..=i], &body[i + 1..]),
            None => ("", body),
        };
        assert!(
            call.starts_with("  call ") || call.contains(" = call "),
            "bug do compilador: a instrução com pouso não termina numa chamada: {call}"
        );
        self.out.push_str(before);
        self.out.push_str(&call.replacen("call ", "invoke ", 1));
        writeln!(self.out, " to label %inv{v}.fim unwind label %b{landing_pad}").unwrap();
        writeln!(self.out, "inv{v}.fim:").unwrap();
        self.current_label = format!("inv{v}.fim");
    }

    /// Os invocadores uniformes ARC (closures, seletores, chamadas tipadas):
    /// em corpos preparados pela passada ARC, somente com o certificado de
    /// portas AOT. Sem preparação, a imagem ARC segue o regime legado do CLI,
    /// com a chamada direta da entrada.
    ///
    /// # Panics
    /// Corpos ARC preparados sem o certificado de portas.
    pub(super) fn arc_uniform_abi(&self) -> bool {
        if !self.module.arc_memory || !(self.module.arc_prepared || self.module.arc_aot_ports.is_some()) {
            return false;
        }
        assert!(self.module.arc_aot_ports.is_some(), "invocação ARC preparada exige portas AOT certificadas");
        true
    }

    /// As fábricas com owner do código (literal de lista, interpolação): só
    /// quando a preparação ARC inseriu a liberação desse owner. No regime
    /// legado ninguém o soltaria, e o objeto nunca seria liberado.
    pub(super) fn arc_owned_factories(&self) -> bool {
        self.module.arc_memory && self.module.arc_prepared
    }

    /// `%cf<v>`: o ponteiro da entrada uniforme da closure `c` — o código
    /// dela, ou `@df_clo_invalido` quando o valor não é closure (o runtime
    /// devolve 0 e deixa o `NoSuchMethodError` pendente).
    fn closure_entry(&mut self, v: u32, c: &str) {
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
    fn descriptor(args: usize, names: &[String]) -> Vec<i64> {
        let mut d = vec![(args - names.len()) as i64, names.len() as i64];
        d.extend(names.iter().map(|n| crate::lower::closures::hash_name(n)));
        d
    }

    fn register_vector(&mut self, v: Vec<i64>) -> usize {
        if let Some(&k) = self.vector_of.get(&v) {
            return k;
        }
        let k = self.vectors.len();
        self.vector_of.insert(v.clone(), k);
        self.vectors.push(v);
        k
    }

    /// `@df_clo_invalido` (a entrada que a chamada usa quando o valor não é
    /// closure — o runtime já deixou o `NoSuchMethodError` pendente) e os
    /// vetores constantes.
    fn emit_closures(&mut self) {
        let module = self.module;
        for func in &module.functions {
            for block in &func.blocks {
                for (_, inst, _) in &block.instructions {
                    match inst {
                        Instruction::ConstArray(v) => {
                            self.register_vector(v.clone());
                        }
                        Instruction::FunctionTable(v) => {
                            if !self.function_table_of.contains_key(v) {
                                self.function_table_of.insert(v.clone(), self.function_tables.len());
                                self.function_tables.push(v.clone());
                            }
                            for s in v {
                                if !self.function_by_symbol.contains_key(s.as_str()) {
                                    self.annotate_external(s, Type::Ref, &[]);
                                }
                            }
                        }
                        Instruction::CallClosure { args, names, .. } | Instruction::CallSelector { args, names, .. } => {
                            self.register_vector(Self::descriptor(args.len(), names));
                            self.argument_names.extend(names.iter().cloned());
                        }
                        _ => {}
                    }
                }
            }
        }
        if self.module.sdk_mode {
            // O descritor sem argumentos do `dartforge_dispatch_toString`.
            self.register_vector(vec![0, 0]);
        }
        self.out.push_str("; Closures: entrada inválida e vetores constantes\n");
        self.out.push_str("define internal i64 @df_clo_invalido(i64 %c, ptr %a, ptr %d) {\nb0:\n  ret i64 0\n}\n");
        for (k, v) in self.vectors.iter().enumerate() {
            let items: Vec<String> = v.iter().map(|x| format!("i64 {x}")).collect();
            writeln!(
                self.out,
                "@df.arr.{k} = private unnamed_addr constant [{} x i64] [{}]",
                v.len(),
                items.join(", ")
            )
            .unwrap();
        }
        for (k, v) in self.function_tables.iter().enumerate() {
            let items: Vec<String> = v.iter().map(|s| format!("ptr @{s}")).collect();
            writeln!(self.out, "@df.fns.{k} = private unnamed_addr constant [{} x ptr] [{}]", v.len(), items.join(", ")).unwrap();
        }
        self.out.push('\n');
    }

    /// Converte os bits i64 lidos de uma célula/ambiente (%u) para a
    /// representação `ty` do resultado `%v<v>`.
    fn bits_to_repr(&mut self, v: u32, bits: &str, ty: Type) {
        match ty {
            Type::F64 => writeln!(self.out, "  %v{v} = bitcast i64 {bits} to double").unwrap(),
            Type::I1 => writeln!(self.out, "  %v{v} = icmp ne i64 {bits}, 0").unwrap(),
            Type::I8 => writeln!(self.out, "  %v{v} = trunc i64 {bits} to i8").unwrap(),
            _ => writeln!(self.out, "  %v{v} = add i64 {bits}, 0").unwrap(),
        }
    }

    /// Tag da ABI (bits, tag) de um valor pela representação (E1).
    fn tag_de(&self, op: &Operand) -> u8 {
        match self.type_of(op) {
            Type::I64 => 1,
            Type::I1 | Type::I8 => 2,
            Type::F64 => 4,
            _ => 3,
        }
    }

    /// Emite uma instrução das closures (P1); `false` se não é uma delas.
    fn emit_closure_inst(&mut self, v: u32, inst: &Instruction, ty: Type) -> bool {
        match inst {
            Instruction::AllocCell { value } => self.emit_alloc_cell(v, value),
            Instruction::CellGet { cell } => self.emit_cell_get(v, cell, ty),
            Instruction::CellSet { cell, value } => self.emit_cell_set(cell, value),
            Instruction::EnvGet { env, index } => self.emit_env_get(v, env, *index, ty),
            Instruction::ConcatStrings { parts } => self.emit_concat_strings(v, parts),
            Instruction::AllocEnv { values } => self.emit_alloc_env(v, values),
            Instruction::AllocClosure { code_symbol, env } => self.emit_alloc_closure(v, code_symbol, env),
            Instruction::AllocClosureTyped { code_symbol, env, typed, abi, direct } => {
                self.emit_alloc_typed_closure(v, code_symbol, env, typed, *abi, *direct);
            }
            Instruction::TypedCall { alvo, args, ret } => {
                // A chamada é a mesma nos dois regimes; o predicado só confere
                // o certificado quando os corpos foram preparados.
                self.arc_uniform_abi();
                let a = self.coerce(alvo, Type::I64);
                let mut parts = Vec::with_capacity(args.len());
                for (x, t) in args {
                    let s = self.coerce(x, *t);
                    parts.push(format!("{} {s}", t.llvm_ir()));
                }
                writeln!(self.out, "  %ct{v} = inttoptr i64 {a} to ptr").unwrap();
                if *ret == Type::Void {
                    writeln!(self.out, "  call void %ct{v}({})", parts.join(", ")).unwrap();
                } else {
                    writeln!(self.out, "  %v{v} = call {} %ct{v}({})", ret.llvm_ir(), parts.join(", ")).unwrap();
                }
            }
            Instruction::TearOff { code_symbol } => self.emit_tearoff(v, code_symbol),
            Instruction::CallClosure { closure, args, names, tuple_types, .. } => {
                let k = self.vector_of[&Self::descriptor(args.len(), names)];
                // O slot depois dos argumentos leva a tupla de tipos (a
                // mesma convenção da chamada por seletor).
                let n = args.len() + 1;
                for (i, a) in args.iter().enumerate() {
                    let s = self.coerce(a, Type::I64);
                    writeln!(self.out, "  %ca{v}_{i} = getelementptr [{n} x i64], ptr %cargs{v}, i64 0, i64 {i}").unwrap();
                    writeln!(self.out, "  store i64 {s}, ptr %ca{v}_{i}").unwrap();
                }
                let tuple = self.coerce(tuple_types, Type::I64);
                writeln!(self.out, "  %cat{v} = getelementptr [{n} x i64], ptr %cargs{v}, i64 0, i64 {}", args.len()).unwrap();
                writeln!(self.out, "  store i64 {tuple}, ptr %cat{v}").unwrap();
                let c = self.coerce(closure, Type::Ref);
                if self.arc_uniform_abi() {
                    writeln!(self.out, "  %cav{v} = ptrtoint ptr %cargs{v} to i64").unwrap();
                    writeln!(self.out, "  %cdv{v} = ptrtoint ptr @df.arr.{k} to i64").unwrap();
                    writeln!(self.out, "  %v{v} = call i64 @dartforge_arc_invocar_closure_uniforme_v1(i64 {c}, i64 %cav{v}, i64 %cdv{v})").unwrap();
                } else {
                    self.closure_entry(v, &c);
                    writeln!(self.out, "  %v{v} = call i64 %cf{v}(i64 {c}, ptr %cargs{v}, ptr @df.arr.{k})").unwrap();
                }
            }
            Instruction::CallClosureForward { arc_owned, closure, args, desc } => {
                let c = self.coerce(closure, Type::Ref);
                let a = self.operand_str(args);
                let d = self.operand_str(desc);
                if *arc_owned {
                    assert!(self.module.arc_memory && self.module.arc_aot_ports.is_some(),
                        "repasse de closure ARC exige portas AOT certificadas");
                    writeln!(self.out, "  %cav{v} = ptrtoint ptr {a} to i64").unwrap();
                    writeln!(self.out, "  %cdv{v} = ptrtoint ptr {d} to i64").unwrap();
                    writeln!(self.out, "  %v{v} = call i64 @dartforge_arc_invocar_closure_uniforme_v1(i64 {c}, i64 %cav{v}, i64 %cdv{v})").unwrap();
                } else {
                    self.closure_entry(v, &c);
                    writeln!(self.out, "  %v{v} = call i64 %cf{v}(i64 {c}, ptr {a}, ptr {d})").unwrap();
                }
            }
            Instruction::CallSelector { selector, recv, args, names, tuple_types } => {
                self.emit_selector_call(v, selector, recv, args, names, tuple_types);
            }
            Instruction::CallSelectorForward { arc_owned, selector, recv, args, desc } => {
                self.emit_selector_forward(v, selector, recv, args, desc, *arc_owned);
            }
            Instruction::LoadIndexed { base, index } => {
                let b = self.operand_str(base);
                let i = self.coerce(index, Type::I64);
                writeln!(self.out, "  %li{v} = getelementptr i64, ptr {b}, i64 {i}").unwrap();
                writeln!(self.out, "  %v{v} = load i64, ptr %li{v}").unwrap();
            }
            Instruction::ConstArray(vals) => {
                let k = self.vector_of[vals];
                writeln!(self.out, "  %v{v} = getelementptr i64, ptr @df.arr.{k}, i64 0").unwrap();
            }
            Instruction::FunctionTable(fns) => {
                let k = self.function_table_of[fns];
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
    fn emit_closure_buffers(&mut self, func: &Function) {
        for block in &func.blocks {
            for (vid, inst, _) in &block.instructions {
                match inst {
                    Instruction::Alloca(_) if self.frame_allocas.contains(vid) => {}
                    Instruction::Alloca(ty) => {
                        writeln!(self.out, "  %v{} = alloca {}", vid.0, ty.llvm_ir()).unwrap();
                    }
                    Instruction::AllocList { elements } => self.list_buffer(vid.0, elements),
                    Instruction::AllocRecord { elements } => self.record_buffer(vid.0, elements),
                    Instruction::CallClosure { args, .. } => {
                        writeln!(self.out, "  %cargs{} = alloca [{} x i64]", vid.0, args.len() + 1).unwrap();
                    }
                    Instruction::CallSelector { args, .. } => {
                        writeln!(self.out, "  %sargs{} = alloca [{} x i64]", vid.0, args.len() + 1).unwrap();
                    }
                    Instruction::AllocEnv { values } => self.env_buffer(vid.0, values),
                    Instruction::ConcatStrings { parts } => self.concat_strings_buffer(vid.0, parts),
                    _ => {}
                }
            }
        }
    }

    /// Os globais do módulo (`@dfg_<id>` e a bandeira `$ok`) viram slots da
    /// área de globais do isolado: a VM guarda os estáticos na *field table*
    /// de cada isolado, e aqui cada isolado (uma thread) tem a sua área, que
    /// o runtime cria na primeira vez (`dartforge_area_de_globais`, zerada).
    /// Os caches dos seletores (`selectors.rs`) ganham slots depois destes.
    ///
    /// Numa geração de hot reload ([`LlvmEmitter::with_previous_area`]), a
    /// área começa com o layout da geração viva inteiro — inclusive os slots
    /// que sumiram e os caches de seletor dela, que o código antigo ainda
    /// usa — e só acrescenta: um global que continua fica no mesmo índice.
    fn emit_globals(&mut self) {
        let previous: std::collections::HashMap<i64, usize> = match &self.previous_area {
            Some(a) => {
                self.slot_hashes = a.clone();
                a.iter().enumerate().filter(|(_, h)| **h != 0).map(|(i, h)| (*h, i)).collect()
            }
            None => std::collections::HashMap::new(),
        };
        self.slot_index.clone_from(&previous);
        for (_, _, sym_name) in &self.module.globals {
            for name_str in [sym_name.clone(), format!("{sym_name}$ok")] {
                let h = slot_hash(&name_str);
                let n = match previous.get(&h) {
                    Some(&i) => i,
                    None => {
                        self.slot_hashes.push(h);
                        self.slot_hashes.len() - 1
                    }
                };
                self.global_slots.insert(name_str, n);
            }
        }
    }

    /// O descritor da área de globais do módulo: `[key, n, nome_0…]`, com
    /// a chave estável do módulo e o hash do nome de cada slot. Os slots de
    /// cache de seletor têm [`CACHE_BIT`] no nome: não migram para uma
    /// área nova e a publicação de uma recarga os zera (guardam endereços de
    /// código, e as tabelas de métodos acabaram de mudar). Em produção,
    /// `[key, -n]` ([`LlvmEmitter::with_lean_area`]).
    fn emit_area_descriptor(&mut self) {
        let key = self.module.record.clone().unwrap_or_else(|| "df.programa".to_string());
        if self.lean_area {
            // Produção (`with_lean_area`): `[key, -n]`, sem os nomes.
            let n = self.slot_hashes.len() as i64;
            writeln!(self.out, "@df.area = private unnamed_addr constant [2 x i64] [i64 {}, i64 {}]", slot_hash(&key), -n)
                .unwrap();
            return;
        }
        let mut values = vec![slot_hash(&key).to_string(), String::new()];
        for h in &self.slot_hashes {
            values.push(h.to_string());
        }
        values[1] = (values.len() - 2).to_string();
        let items: Vec<String> = values.iter().map(|v| format!("i64 {v}")).collect();
        writeln!(self.out, "@df.area = private unnamed_addr constant [{} x i64] [{}]", items.len(), items.join(", ")).unwrap();
    }

    /// O slot do próximo cache de seletor da função em emissão: as duas
    /// palavras (id de classe, entrada) com o nome `(função, posição)`, no
    /// mesmo índice da geração viva quando ela já o tinha — o código de uma
    /// função que não mudou é o mesmo texto nas duas gerações (J04).
    pub(super) fn cache_slot(&mut self) -> usize {
        let name_str = format!("{}#{}", self.current_function, self.cache_in_function);
        self.cache_in_function += 1;
        let (h0, h1) = (cache_hash(&name_str, 0), cache_hash(&name_str, 1));
        if let Some(&i) = self.slot_index.get(&h0)
            && self.slot_hashes.get(i + 1) == Some(&h1)
        {
            return i;
        }
        let i = self.slot_hashes.len();
        self.slot_hashes.push(h0);
        self.slot_hashes.push(h1);
        i
    }

    /// Os `alloca` `Ref` usados fora do `load`/`store` deles (o endereço
    /// passado a uma função local direta): ver `frame_allocas`.
    fn escaping_ref_allocas(func: &Function) -> std::collections::HashSet<ValueId> {
        let mut refs = std::collections::HashSet::new();
        for b in &func.blocks {
            for (vid, inst, _) in &b.instructions {
                if matches!(inst, Instruction::Alloca(Type::Ref)) {
                    refs.insert(*vid);
                }
            }
        }
        let mut escape = std::collections::HashSet::new();
        if refs.is_empty() {
            return escape;
        }
        for b in &func.blocks {
            for (_, inst, _) in &b.instructions {
                let uses = match inst {
                    Instruction::Load { .. } => Vec::new(),
                    Instruction::Store { val: Operand::Val(v), .. } => vec![*v],
                    Instruction::Store { .. } => Vec::new(),
                    other => crate::lower::async_sm::uses_of(other),
                };
                escape.extend(uses.into_iter().filter(|v| refs.contains(v)));
            }
            escape.extend(crate::lower::async_sm::terminator_uses(&b.terminator).into_iter().filter(|v| refs.contains(v)));
        }
        escape
    }

    /// A função usa a área de globais (global, bandeira ou cache de seletor)?
    fn uses_area(func: &Function) -> bool {
        func.blocks.iter().any(|b| {
            b.instructions.iter().any(|(_, i, _)| {
                matches!(
                    i,
                    Instruction::LoadGlobal { .. }
                        | Instruction::StoreGlobal { .. }
                        | Instruction::ArcLoadStrong { slot: StrongSlot::Global { .. } }
                        | Instruction::ArcStoreStrong { slot: StrongSlot::Global { .. }, .. }
                        | Instruction::CallSelector { .. }
                        | Instruction::CallSelectorForward { .. }
                        | Instruction::Const(Constant::String(_) | Constant::StringWtf8(_))
                ) || Self::rti_evaluate_cached(i)
            })
        })
    }

    /// O quadro de raízes no topo do bloco de entrada (ver o prólogo).
    fn frame_at_entry() -> bool {
        static OFF: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        !*OFF.get_or_init(|| std::env::var_os("DARTFORGE_SEM_QUADRO_NA_ENTRADA").is_some_and(|v| !v.is_empty() && v != "0"))
    }

    /// `dartforge_rti_avaliar` sem tupla e com `this`: vai pelo cache do
    /// ponto de uso na área ([`Self::emit_rti_evaluate_cached`]).
    /// `DARTFORGE_SEM_CACHE_RTI=1` na compilação desliga (medida).
    fn rti_evaluate_cached(i: &Instruction) -> bool {
        static OFF: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        matches!(i, Instruction::CallRuntime { name, args, .. }
            if name == "dartforge_rti_avaliar"
                && args.len() == 4
                && matches!(args[3].0, Operand::Constant(Constant::Int(0)))
                && !matches!(args[1].0, Operand::Constant(_)))
            && !*OFF.get_or_init(|| std::env::var_os("DARTFORGE_SEM_CACHE_RTI").is_some_and(|v| !v.is_empty() && v != "0"))
    }

    /// `P<i>` avaliado com cache no ponto de uso (N17): duas palavras da
    /// área do isolado guardam a chave do tipo de `this` — o metadado do
    /// cabeçalho do objeto (`heap::Cabecalho`, deslocamento 12 do bloco, 10
    /// do handle), ou `-16 - class` sem metadado, a mesma chave da memória
    /// do runtime (`chave_do_valor`) — e o resultado. Um objeto do espaço
    /// com a chave gravada responde com duas cargas; o resto (valores do
    /// runtime, a primeira vez, outra chave) chama
    /// `dartforge_rti_avaliar_cache`, que responde e grava. O modelo e a
    /// classe são constantes do ponto; a tupla é 0. A área é por isolado (os
    /// ids de tipo são do universo dele) e a recarga zera os caches.
    fn emit_rti_evaluate_cached(&mut self, v: u32, args: &[(Operand, Type)]) {
        let model = self.coerce(&args[0].0, Type::I64);
        let this = self.coerce(&args[1].0, Type::I64);
        let class = self.coerce(&args[2].0, Type::I64);
        let slot = self.cache_slot();
        let o = &mut self.out;
        writeln!(o, "  %ra{v}c = getelementptr i64, ptr %area, i64 {slot}").unwrap();
        writeln!(o, "  %ra{v}m = and i64 {this}, {OBJECT_MASK}").unwrap();
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
        writeln!(o, "  %ra{v}r2 = call i64 @dartforge_rti_avaliar_cache(ptr %ra{v}c, i64 {model}, i64 {this}, i64 {class})").unwrap();
        writeln!(o, "  br label %ra{v}.fim").unwrap();
        writeln!(o, "ra{v}.fim:").unwrap();
        writeln!(o, "  %v{v} = phi i64 [ %ra{v}r1, %ra{v}.acerto ], [ %ra{v}r2, %ra{v}.lenta ]").unwrap();
        self.current_label = format!("ra{v}.fim");
    }

    /// O endereço do slot de um global, em `%ga<v>`.
    fn global_address(&mut self, v: u32, sym_name: &str) {
        let slot = *self
            .global_slots
            .get(sym_name)
            .unwrap_or_else(|| panic!("bug do compilador: global @{sym_name} sem slot na área"));
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

    /// Referência já avaliada: não usar coagir, que poderia alocar uma caixa.
    /// HIR inválida é falha interna; o verificador confere esta precondição.
    fn arc_reference(&self, value: &Operand) -> String {
        match value {
            Operand::Val(v) => {
                assert_eq!(self.types.get(v), Some(&Type::Ref), "operação ARC exige SSA Ref");
                format!("%v{}", v.0)
            }
            Operand::Constant(Constant::Null) => "0".into(),
            _ => panic!("operação ARC exige referência já avaliada"),
        }
    }

    /// Descritor de quadro classificado; nunca coagir um ponteiro para um ID.
    fn slot_arc(&self, slot: &StrongSlot) -> (String, u32) {
        let StrongSlot::Frame { frame, index: idx_value } = slot else { panic!("descritor não é quadro") };
        let Operand::Val(v) = frame else {
            panic!("slot ARC exige quadro SSA")
        };
        assert_eq!(
            self.types.get(v),
            Some(&Type::I64),
            "quadro ARC exige ID escalar I64"
        );
        (format!("%v{}", v.0), *idx_value)
    }

    /// Schema do armazenamento real; não inferir Ref por largura ou endereço.
    fn check_arc_global(&self, sym_name: &str) {
        let mut declarations = self.module.globals.iter().filter(|(_, _, s)| s == sym_name);
        assert_eq!(declarations.next().map(|(_, ty, _)| *ty), Some(Type::Ref), "global ARC deve ser Ref declarado");
        assert!(declarations.next().is_none(), "global ARC deve ter declaração única");
    }

    /// Confere a ABI de tokens antes dos registros e de qualquer código Dart.
    /// A falha é interna e fatal: não pode virar uma pendência ignorada.
    fn start_arc(&mut self) {
        if !self.module.arc_memory {
            return;
        }
        self.externals.insert("llvm.trap".into(), "declare void @llvm.trap()".into());
        self.out.push_str("  call void @dartforge_memoria_arc_v1()\n\
            \x20 %df.arc.abi = call i8 @dartforge_arc_verificar_abi(i64 1)\n\
            \x20 %df.arc.abi.ok = icmp eq i8 %df.arc.abi, 1\n\
            \x20 br i1 %df.arc.abi.ok, label %df.arc.compativel, label %df.arc.incompativel\n\
            df.arc.incompativel:\n\
            \x20 call void @llvm.trap()\n\
            \x20 unreachable\n\
            df.arc.compativel:\n");
        if !self.module.sdk_mode {
            self.register_arc_aot_ports();
        }
    }

    fn emit_entry(&mut self) {
        if self.module.excecoes_por_tabelas {
            self.emit_ports();
        }
        if self.module.sdk_mode {
            self.emit_registration("df.registrar.programa");
            let ids: Vec<String> = self.module.runtime_cids.iter().map(|c| format!("i64 {c}")).collect();
            writeln!(
                self.out,
                "@df.cids = private unnamed_addr constant [{} x i64] [{}]",
                ids.len(),
                ids.join(", ")
            )
            .unwrap();
            if let Some(v) = &self.module.sdk_version {
                writeln!(
                    self.out,
                    "@df.versao_do_sdk = private unnamed_addr constant [{} x i8] c\"{}\"",
                    v.len(),
                    selectors::llvm_bytes(v)
                )
                .unwrap();
            }
            // A preparação de um isolado (a principal e a de cada
            // `Isolate.spawn`, que o runtime chama na thread nova): os
            // registros das bibliotecas, a RTI e o embedder.
            writeln!(self.out, "define void @df.preparar_isolado() {{").unwrap();
            // A memória ARC, ligada antes de todo código Dart do isolado.
            self.start_arc();
            // Imagem ARC preparada: as portas Dart que o runtime chama aqui
            // (registros, RTI, embedder) devolvem owners a um escopo nativo.
            let root_scope = self.module.arc_memory && self.module.arc_prepared;
            if root_scope {
                writeln!(self.out, "  %escopo.prep = call i64 @dartforge_arc_escopo_nativo_abrir_v1()").unwrap();
            }
            writeln!(self.out, "  call void @dartforge_registrar_cids(ptr @df.cids, i64 {})", ids.len()).unwrap();
            if let Some(v) = &self.module.sdk_version {
                writeln!(self.out, "  call void @dartforge_registrar_versao_do_sdk(ptr @df.versao_do_sdk, i64 {})", v.len()).unwrap();
            }
            // As tabelas das classes dos valores do runtime (que não passam
            // por `dartforge_object_new_t`).
            for c in self.module.runtime_cids.clone() {
                if let Some(f) = self.module.table_functions.get(&(c as u32)).cloned() {
                    self.annotate_external(&f, Type::Ptr, &[]);
                    writeln!(self.out, "  call void @dartforge_registrar_tabela(i64 {c}, ptr @{f})").unwrap();
                }
            }
            for r in &self.module.sdk_records {
                writeln!(self.out, "  call void @{r}()").unwrap();
                self.externals.insert(r.clone(), format!("declare void @{r}()"));
            }
            writeln!(self.out, "  call void @df.registrar.programa()").unwrap();
            // RTI e laço de eventos, como na entrada de sempre (abaixo).
            if let Some(start) = &self.module.start_rti {
                writeln!(self.out, "  {}", entry_call(self.module.excecoes_por_tabelas, start, &[])).unwrap();
            }
            // O que o embedder da VM prepara antes do `main` (o script de
            // `dart:io`, o `Uri.base`), já com as bibliotecas registradas.
            // `dart:ffi`: os tipos nativos e os trampolins das assinaturas
            // (`lower/ffi.rs`).
            for c in self.module.ffi_composites.clone() {
                writeln!(
                    self.out,
                    "  call void @dartforge_ffi_registrar_composto(i64 {}, i64 {}, i64 {}, i64 {}, i64 {}, i64 {}, i64 {})",
                    c.rti, c.class, c.fields, c.base_index, c.offset_index, c.size, c.alignment
                )
                .unwrap();
            }
            for (c, letter) in &self.module.ffi_types {
                writeln!(self.out, "  call void @dartforge_ffi_registrar_tipo(i64 {c}, i64 {})", u32::from(*letter)).unwrap();
            }
            for (i, (key, sym_name)) in self.module.ffi_trampolines.iter().enumerate() {
                writeln!(
                    self.out,
                    "  call void @dartforge_ffi_registrar_trampolim(ptr @df.ffi.chave.{i}, i64 {}, ptr @\"{sym_name}\")",
                    key.len()
                )
                .unwrap();
            }
            for (i, cb) in self.module.ffi_callbacks.iter().enumerate() {
                writeln!(
                    self.out,
                    "  call void @dartforge_ffi_registrar_callback(ptr @df.ffi.cbchave.{i}, i64 {}, ptr @df.ffi.cbentrada.{i})",
                    cb.key.len()
                )
                .unwrap();
            }
            writeln!(self.out, "  call void @dartforge_preparar_embedder()").unwrap();
            if root_scope {
                writeln!(self.out, "  call void @dartforge_arc_escopo_nativo_fechar_v1(i64 %escopo.prep)").unwrap();
            }
            writeln!(self.out, "  ret void\n}}\n").unwrap();
            for (i, (key, _)) in self.module.ffi_trampolines.iter().enumerate() {
                writeln!(self.out, "@df.ffi.chave.{i} = private unnamed_addr constant [{} x i8] c\"{key}\"", key.len()).unwrap();
            }
            self.emit_callbacks_ffi();
            writeln!(self.out, "define void @dartforge_entry() {{").unwrap();
            // Exceções por tabelas: as portas pelas quais o runtime chama
            // código Dart, entregues antes de qualquer código Dart rodar.
            if self.module.excecoes_por_tabelas {
                writeln!(self.out, "  call void @dartforge_registrar_portas(ptr @df.portas, i64 {N_PORTS})").unwrap();
            }
            let call = self.module.call_dart.as_ref().map_or("null".to_string(), |c| format!("@{c}"));
            writeln!(self.out, "  call void @dartforge_registrar_isolados(ptr @df.preparar_isolado, ptr {call})").unwrap();
            writeln!(self.out, "  call void @df.preparar_isolado()").unwrap();
            // O escopo raiz do isolado principal: o `main` e o laço de
            // eventos, cujas chamadas do runtime ao Dart devolvem owners.
            if root_scope {
                writeln!(self.out, "  %escopo.raiz = call i64 @dartforge_arc_escopo_nativo_abrir_v1()").unwrap();
            }
            self.call_main();
            if let Some(call) = &self.module.call_dart {
                writeln!(self.out, "  call void @dartforge_laco_de_eventos(ptr @{call})").unwrap();
            }
            if root_scope {
                writeln!(self.out, "  call void @dartforge_arc_escopo_nativo_fechar_v1(i64 %escopo.raiz)").unwrap();
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
        self.start_arc();
        // Exceções por tabelas: as portas pelas quais o runtime chama
        // código Dart, entregues antes de qualquer código Dart rodar.
        if self.module.excecoes_por_tabelas {
            writeln!(self.out, "  call void @dartforge_registrar_portas(ptr @df.portas, i64 {N_PORTS})").unwrap();
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

        if self.maps && !self.maps_in_jit {
            self.out.push_str(Self::maps_registration_call());
        }
        if let Some(c) = self.trace_registration_call() {
            self.out.push_str(&c);
            let fields = self.trace_field_registration_calls();
            self.out.push_str(&fields);
        }
        // Registra grafo de subtipagem
        for (sub, sup) in &self.module.subtyping_edges {
            writeln!(
                self.out,
                "  call void @dartforge_register_subclass(i64 {sub}, i64 {sup})"
            ).unwrap();
        }

        // RTI: o universo de tipos (classes citadas e regras de supertipo).
        if let Some(start) = &self.module.start_rti {
            writeln!(self.out, "  {}", entry_call(self.module.excecoes_por_tabelas, start, &[])).unwrap();
        }
        self.call_main();
        // P6: microtarefas e timers depois do `main` (runtime, `events.rs`).
        if let Some(call) = &self.module.call_dart {
            writeln!(self.out, "  call void @dartforge_laco_de_eventos(ptr @{call})").unwrap();
        }
        writeln!(self.out, "  ret void").unwrap();
        writeln!(self.out, "}}\n").unwrap();
    }

    /// A chamada do `main` na entrada: com parâmetros, o primeiro é a lista
    /// dos argumentos da linha de comando (`dartforge_argumentos_do_main`) e
    /// o segundo, `null`.
    /// Um argumento primitivo de chamada C: o operando convertido ao tipo C
    /// (estreitos com a extensão da ABI), como `ty attributes value`.
    fn c_argument(&mut self, v: u32, i: usize, op: &Operand, tc: CType) -> String {
        let x = match tc {
            CType::F32 => {
                let d = self.coerce(op, Type::F64);
                writeln!(self.out, "  %na{v}_{i} = fptrunc double {d} to float").unwrap();
                format!("%na{v}_{i}")
            }
            CType::F64 => self.coerce(op, Type::F64),
            CType::Bool => self.coerce(op, Type::I1),
            CType::Ptr | CType::Handle => {
                let n = self.coerce(op, Type::I64);
                writeln!(self.out, "  %na{v}_{i} = inttoptr i64 {n} to ptr").unwrap();
                format!("%na{v}_{i}")
            }
            CType::I64 | CType::U64 => self.coerce(op, Type::I64),
            CType::Void => unreachable!("argumento nativo void"),
            narrow => {
                let n = self.coerce(op, Type::I64);
                writeln!(self.out, "  %na{v}_{i} = trunc i64 {n} to {}", narrow.llvm()).unwrap();
                format!("%na{v}_{i}")
            }
        };
        format!("{} {}{x}", tc.llvm(), tc.extension())
    }

    /// O texto `call …` de uma chamada C por `%fn<v>`. Numa variádica com as
    /// raízes por mapas, a chamada passa por um intermediário
    /// `@df.vararg.<k>` de assinatura fixa, `noinline` e fora da estratégia de
    /// coleta: o `rewrite-statepoints-for-gc` não embrulha chamadas variádicas
    /// que devolvem valor (`gc.statepoint doesn't support wrapping non-void
    /// vararg functions yet`), e a chamada ao intermediário vira um statepoint
    /// comum, com as raízes vivas no `"deopt"` dela (§14.12: toda chamada que
    /// coleta leva o operando, também a de uma função sem raízes, que pode ser
    /// copiada pelo inliner numa com raízes). O intermediário repassa os
    /// argumentos com os mesmos tipos e atributos.
    fn c_call_text(&mut self, v: u32, ret: &str, typ: &str, parts: &[String], variadic_flag: bool) -> String {
        let list = parts.join(", ");
        if !variadic_flag || !self.maps {
            return format!("call {typ} %fn{v}({list})");
        }
        let name_str = format!("df.vararg.{}", self.variadic_set.len());
        let mut params = vec!["ptr %f".to_string()];
        let mut forward = Vec::with_capacity(parts.len());
        for (j, p) in parts.iter().enumerate() {
            let head = p.rsplit_once(' ').map_or(p.as_str(), |(c, _)| c);
            params.push(format!("{head} %p{j}"));
            forward.push(format!("{head} %p{j}"));
        }
        let internal = format!("call {typ} %f({})", forward.join(", "));
        let body = if ret == "void" { format!("  {internal}\n  ret void\n") } else { format!("  %r = {internal}\n  ret {ret} %r\n") };
        self.variadic_set.push(format!("define internal {ret} @{name_str}({}) noinline {{\n{body}}}\n\n", params.join(", ")));
        let mut args = vec![format!("ptr %fn{v}")];
        args.extend(parts.iter().cloned());
        format!("call {ret} @{name_str}({})", args.join(", "))
    }

    /// Uma chamada C com structs/unions por valor (`llvm/c_abi.rs`): cada
    /// composto é copiado dos bytes do operando para uma temporária alinhada
    /// da pilha (liberada por `stackrestore` depois da chamada), e passado
    /// em peças, `byval` ou por ponteiro; um retorno composto vai para
    /// `dest` (direto: pelas peças gravadas numa temporária; `sret`: o
    /// próprio destino).
    fn composite_native_call(
        &mut self,
        v: u32,
        alvo: &Operand,
        args: &[(Operand, NativeType)],
        ret: &NativeType,
        dest: Option<&Operand>,
        variadic_flag: Option<usize>,
    ) {
        use c_abi::{ArgPassing, RetPassing};
        let conv = c_abi::Convention::of_target();
        let mut regs = c_abi::Registers::new_set();
        let a = self.coerce(alvo, Type::I64);
        writeln!(self.out, "  %fn{v} = inttoptr i64 {a} to ptr").unwrap();
        writeln!(self.out, "  %pilha{v} = call ptr @llvm.stacksave.p0()").unwrap();
        // Retorno primeiro: um `sret` ocupa o primeiro registrador inteiro.
        let ret_passing = match ret {
            NativeType::Composite(l) => Some((c_abi::ret(conv, l, &mut regs), l.clone())),
            NativeType::Prim(_) => None,
        };
        let mut parts = Vec::with_capacity(args.len() + 1);
        let dest_ptr = dest.map(|d| {
            let d = self.coerce(d, Type::I64);
            writeln!(self.out, "  %dst{v} = inttoptr i64 {d} to ptr").unwrap();
            format!("%dst{v}")
        });
        if let (Some((RetPassing::Sret { alignment }, l)), Some(d)) = (&ret_passing, &dest_ptr) {
            parts.push(format!("ptr sret([{} x i8]) align {alignment} {d}", l.size));
        }
        let mut fixed_types: Vec<String> = Vec::new();
        if matches!(&ret_passing, Some((RetPassing::Sret { .. }, _))) {
            fixed_types.push("ptr".to_string());
        }
        for (i, (op, t)) in args.iter().enumerate() {
            let variadic = variadic_flag.is_some_and(|n| i >= n);
            match t {
                // Promoções de argumento padrão do C num argumento variádico:
                // `float` vai como `double`; inteiros menores que `int`,
                // como `int` (com o sinal do tipo).
                NativeType::Prim(tc) if variadic => {
                    regs.consume_primitive(*tc);
                    let p = match tc {
                        CType::F32 => self.c_argument(v, i, op, CType::F64),
                        CType::I8 | CType::I16 => self.c_argument(v, i, op, CType::I32),
                        CType::U8 | CType::U16 | CType::Bool => {
                            let n = self.coerce(op, if *tc == CType::Bool { Type::I1 } else { Type::I64 });
                            let ext = if *tc == CType::Bool { format!("zext i1 {n} to i32") } else { format!("trunc i64 {n} to i32") };
                            writeln!(self.out, "  %na{v}_{i} = {ext}").unwrap();
                            format!("i32 %na{v}_{i}")
                        }
                        _ => self.c_argument(v, i, op, *tc),
                    };
                    parts.push(p);
                }
                NativeType::Prim(tc) => {
                    regs.consume_primitive(*tc);
                    let p = self.c_argument(v, i, op, *tc);
                    fixed_types.push(p.split(' ').next().unwrap_or("i64").to_string());
                    parts.push(p);
                }
                NativeType::Composite(l) => {
                    // A cópia: alinhada, zerada até a palavra, com os bytes.
                    let n = self.coerce(op, Type::I64);
                    let sz = l.size.div_ceil(16) * 16;
                    let al = l.alignment.max(16);
                    writeln!(self.out, "  %src{v}_{i} = inttoptr i64 {n} to ptr").unwrap();
                    writeln!(self.out, "  %tmp{v}_{i} = alloca [{sz} x i8], align {al}").unwrap();
                    writeln!(self.out, "  call void @llvm.memset.p0.i64(ptr %tmp{v}_{i}, i8 0, i64 {sz}, i1 false)").unwrap();
                    writeln!(self.out, "  call void @llvm.memcpy.p0.p0.i64(ptr %tmp{v}_{i}, ptr %src{v}_{i}, i64 {}, i1 false)", l.size).unwrap();
                    // Na parte variádica as peças não entram no tipo da
                    // chamada (`ret (fixed_set, ...)`).
                    let passing = if variadic {
                        c_abi::variadic_argument(conv, l, &mut regs)
                    } else {
                        c_abi::argument(conv, l, &mut regs)
                    };
                    let mut fixed = |t: String| {
                        if !variadic {
                            fixed_types.push(t);
                        }
                    };
                    match passing {
                        ArgPassing::Direct(pieces) => {
                            for (k, p) in pieces.iter().enumerate() {
                                writeln!(self.out, "  %pp{v}_{i}_{k} = getelementptr i8, ptr %tmp{v}_{i}, i64 {}", p.offset).unwrap();
                                writeln!(self.out, "  %pc{v}_{i}_{k} = load {}, ptr %pp{v}_{i}_{k}, align 1", p.ty).unwrap();
                                parts.push(format!("{} {}%pc{v}_{i}_{k}", p.ty, p.attributes));
                                fixed(p.ty.clone());
                            }
                        }
                        ArgPassing::Byval { alignment } => {
                            parts.push(format!("ptr byval([{} x i8]) align {alignment} %tmp{v}_{i}", l.size));
                            fixed("ptr".to_string());
                        }
                        ArgPassing::Indirect => {
                            parts.push(format!("ptr %tmp{v}_{i}"));
                            fixed("ptr".to_string());
                        }
                    }
                }
            }
        }
        // O tipo da chamada: numa variádica, `ret (fixed_set, ...)`.
        let call_type = |ret: &str| match variadic_flag {
            Some(_) if fixed_types.is_empty() => format!("{ret} (...)"),
            Some(_) => format!("{ret} ({}, ...)", fixed_types.join(", ")),
            None => ret.to_string(),
        };
        let return_type = match (ret, &ret_passing) {
            (NativeType::Prim(CType::Void), _) | (NativeType::Composite(_), Some((RetPassing::Sret { .. }, _))) => "void".to_string(),
            (NativeType::Prim(tc), _) => tc.llvm().to_string(),
            (NativeType::Composite(_), Some((RetPassing::Direct(pieces), _))) => c_abi::return_ty(pieces),
            (NativeType::Composite(_), None) => unreachable!("retorno composto sem passagem"),
        };
        let call = self.c_call_text(v, &return_type, &call_type(&return_type), &parts, variadic_flag.is_some());
        match (ret, &ret_passing) {
            (NativeType::Prim(CType::Void), _) => writeln!(self.out, "  {call}").unwrap(),
            (NativeType::Prim(tc), _) => {
                let conv_ret = match tc {
                    CType::I8 | CType::I16 | CType::I32 => Some(format!("sext {} %nr{v} to i64", tc.llvm())),
                    CType::U8 | CType::U16 | CType::U32 => Some(format!("zext {} %nr{v} to i64", tc.llvm())),
                    CType::F32 => Some(format!("fpext float %nr{v} to double")),
                    CType::Ptr | CType::Handle => Some(format!("ptrtoint ptr %nr{v} to i64")),
                    _ => None,
                };
                match conv_ret {
                    Some(c) => {
                        writeln!(self.out, "  %nr{v} = {call}").unwrap();
                        writeln!(self.out, "  %v{v} = {c}").unwrap();
                    }
                    None => writeln!(self.out, "  %v{v} = {call}").unwrap(),
                }
            }
            (NativeType::Composite(_), Some((RetPassing::Sret { .. }, _))) => {
                writeln!(self.out, "  {call}").unwrap();
            }
            (NativeType::Composite(_), Some((RetPassing::Direct(pieces), l))) => {
                let typ = c_abi::return_ty(pieces);
                let sz = l.size.div_ceil(16) * 16;
                writeln!(self.out, "  %rr{v} = {call}").unwrap();
                writeln!(self.out, "  %rt{v} = alloca [{sz} x i8], align 16").unwrap();
                for (k, p) in pieces.iter().enumerate() {
                    let val = if pieces.len() == 1 {
                        format!("%rr{v}")
                    } else {
                        writeln!(self.out, "  %re{v}_{k} = extractvalue {typ} %rr{v}, {k}").unwrap();
                        format!("%re{v}_{k}")
                    };
                    writeln!(self.out, "  %rp{v}_{k} = getelementptr i8, ptr %rt{v}, i64 {}", p.offset).unwrap();
                    writeln!(self.out, "  store {} {val}, ptr %rp{v}_{k}, align 1", p.ty).unwrap();
                }
                if let Some(d) = &dest_ptr {
                    writeln!(self.out, "  call void @llvm.memcpy.p0.p0.i64(ptr {d}, ptr %rt{v}, i64 {}, i1 false)", l.size).unwrap();
                }
            }
            (NativeType::Composite(_), None) => unreachable!("retorno composto sem passagem"),
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
        use c_abi::{ArgPassing, RetPassing};
        if self.module.ffi_callbacks.is_empty() {
            return;
        }
        let conv = c_abi::Convention::of_target();
        for (i, cb) in self.module.ffi_callbacks.clone().iter().enumerate() {
            writeln!(self.out, "@df.ffi.cbchave.{i} = private unnamed_addr constant [{} x i8] c\"{}\"", cb.key.len(), cb.key).unwrap();
            let mut regs = c_abi::Registers::new_set();
            // O retorno primeiro: um `sret` ocupa o primeiro registrador.
            let ret = match &cb.ret {
                NativeType::Composite(l) => Some((c_abi::ret(conv, l, &mut regs), l.clone())),
                NativeType::Prim(_) => None,
            };
            let mut decls = vec!["ptr nest %ctx".to_string()];
            if let Some((RetPassing::Sret { alignment }, l)) = &ret {
                decls.push(format!("ptr sret([{} x i8]) align {alignment} %sret", l.size));
            }
            // Como cada composto chega: em peças (remontadas numa
            // temporária) ou já em memória (`byval`/ponteiro).
            let mut pieces_of: Vec<Option<(Vec<c_abi::Piece>, usize)>> = Vec::with_capacity(cb.params.len());
            for (j, t) in cb.params.iter().enumerate() {
                match t {
                    NativeType::Prim(tc) => {
                        regs.consume_primitive(*tc);
                        decls.push(format!("{} {}%a{j}", tc.llvm(), tc.extension()));
                        pieces_of.push(None);
                    }
                    NativeType::Composite(l) => match c_abi::argument(conv, l, &mut regs) {
                        ArgPassing::Direct(pieces) => {
                            for (k, p) in pieces.iter().enumerate() {
                                decls.push(format!("{} {}%a{j}_{k}", p.ty, p.attributes));
                            }
                            pieces_of.push(Some((pieces, l.size)));
                        }
                        ArgPassing::Byval { alignment } => {
                            decls.push(format!("ptr byval([{} x i8]) align {alignment} %a{j}", l.size));
                            pieces_of.push(None);
                        }
                        ArgPassing::Indirect => {
                            decls.push(format!("ptr %a{j}"));
                            pieces_of.push(None);
                        }
                    },
                }
            }
            let (ret_type, ret_ext) = match (&cb.ret, &ret) {
                (NativeType::Prim(tc), _) => (
                    tc.llvm().to_string(),
                    match tc {
                        CType::I8 | CType::I16 => "signext ",
                        CType::U8 | CType::U16 | CType::Bool => "zeroext ",
                        _ => "",
                    },
                ),
                (_, Some((RetPassing::Direct(pieces), _))) => (c_abi::return_ty(pieces), ""),
                _ => ("void".to_string(), ""),
            };
            let r = ret_type.as_str();
            // Exceções por tabelas: a entrada pousa o desenrolamento do corpo
            // (abaixo) — código C não é atravessado. A sabotagem
            // `callback_sem_pouso` (o D9 do §7.3) tira o pouso.
            let by_tables = self.module.excecoes_por_tabelas && !crate::alvo::sabotage("callback_sem_pouso");
            let personality = if by_tables { " personality ptr @dartforge_personalidade" } else { "" };
            writeln!(self.out, "define internal {ret_ext}{r} @df.ffi.cbentrada.{i}({}){personality} {{", decls.join(", "))
                .unwrap();
            let n = cb.params.len().max(1);
            writeln!(self.out, "entrada:\n  %saida = alloca i64\n  %buf = alloca [{n} x i64]").unwrap();
            if let Some((_, l)) = &ret {
                writeln!(self.out, "  %rt = alloca [{} x i8], align 16", l.size.div_ceil(16) * 16).unwrap();
            }
            // O endereço de cada composto (`%end{j}`, i64).
            for (j, t) in cb.params.iter().enumerate() {
                let NativeType::Composite(_) = t else { continue };
                match &pieces_of[j] {
                    Some((pieces, size)) => {
                        writeln!(self.out, "  %m{j} = alloca [{} x i8], align 16", size.div_ceil(16) * 16).unwrap();
                        for (k, p) in pieces.iter().enumerate() {
                            writeln!(self.out, "  %mp{j}_{k} = getelementptr i8, ptr %m{j}, i64 {}", p.offset).unwrap();
                            writeln!(self.out, "  store {} %a{j}_{k}, ptr %mp{j}_{k}, align 1", p.ty).unwrap();
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
                    NativeType::Composite(_) => format!("add i64 %end{j}, 0"),
                    NativeType::Prim(tc) => match tc {
                        CType::I8 | CType::I16 | CType::I32 => format!("sext {} %a{j} to i64", tc.llvm()),
                        CType::U8 | CType::U16 | CType::U32 | CType::Bool => format!("zext {} %a{j} to i64", tc.llvm()),
                        CType::I64 | CType::U64 => format!("add i64 %a{j}, 0"),
                        CType::F32 => {
                            writeln!(self.out, "  %of{j} = fpext float %a{j} to double").unwrap();
                            format!("bitcast double %of{j} to i64")
                        }
                        CType::F64 => format!("bitcast double %a{j} to i64"),
                        CType::Ptr | CType::Handle => format!("ptrtoint ptr %a{j} to i64"),
                        CType::Void => unreachable!("parâmetro nativo void"),
                    },
                };
                writeln!(self.out, "  %o{j} = {bits}\n  %g{j} = getelementptr [{n} x i64], ptr %buf, i64 0, i64 {j}\n  store i64 %o{j}, ptr %g{j}").unwrap();
            }
            writeln!(self.out, "  call void @dartforge_ffi_callback_postar(ptr %ctx, ptr %buf, i64 {})", cb.params.len()).unwrap();
            match &cb.ret {
                NativeType::Prim(CType::Void) => writeln!(self.out, "  ret void").unwrap(),
                NativeType::Prim(CType::Ptr | CType::Handle) => writeln!(self.out, "  ret ptr null").unwrap(),
                NativeType::Prim(CType::F32 | CType::F64) => writeln!(self.out, "  ret {r} 0.0").unwrap(),
                NativeType::Prim(_) => writeln!(self.out, "  ret {r} 0").unwrap(),
                NativeType::Composite(_) if r == "void" => writeln!(self.out, "  ret void").unwrap(),
                NativeType::Composite(_) => writeln!(self.out, "  ret {r} zeroinitializer").unwrap(),
            }
            // Local: a chamada ao corpo HIR na representação Dart.
            writeln!(self.out, "local:\n  %c = ptrtoint ptr %ctx to i64").unwrap();
            let mut args = vec!["i64 %c".to_string()];
            for (j, t) in cb.params.iter().enumerate() {
                let tc = match t {
                    NativeType::Composite(_) => {
                        args.push(format!("i64 %end{j}"));
                        continue;
                    }
                    NativeType::Prim(tc) => *tc,
                };
                let (conv_arg, ty) = match tc {
                    CType::I8 | CType::I16 | CType::I32 => (Some(format!("sext {} %a{j} to i64", tc.llvm())), "i64"),
                    CType::U8 | CType::U16 | CType::U32 => (Some(format!("zext {} %a{j} to i64", tc.llvm())), "i64"),
                    CType::F32 => (Some(format!("fpext float %a{j} to double")), "double"),
                    CType::Ptr | CType::Handle => (Some(format!("ptrtoint ptr %a{j} to i64")), "i64"),
                    CType::I64 | CType::U64 => (None, "i64"),
                    CType::F64 => (None, "double"),
                    CType::Bool => (None, "i1"),
                    CType::Void => unreachable!("parâmetro nativo void"),
                };
                match conv_arg {
                    Some(c) => {
                        writeln!(self.out, "  %d{j} = {c}").unwrap();
                        args.push(format!("{ty} %d{j}"));
                    }
                    None => args.push(format!("{ty} %a{j}")),
                }
            }
            let hr = cb.ret.hir_type().llvm_ir();
            let empty = matches!(cb.ret, NativeType::Prim(CType::Void));
            if by_tables {
                // O corpo que lança desenrola: o pouso restaura o topo da
                // pilha-sombra e segue com o valor padrão e a exceção
                // pendente, como o corpo devolvia no modelo de conferência.
                self.out.push_str(
                    "  %tctx = call ptr @dartforge_contexto()\n  %ttp = getelementptr inbounds i8, ptr %tctx, i64 8\n  %ttopo = load ptr, ptr %ttp, align 8\n",
                );
                let landing_pad = "tlp:\n  %tx = landingpad { ptr, i32 } catch ptr null\n  store ptr %ttopo, ptr %ttp, align 8\n  br label %tcont\n";
                if empty {
                    writeln!(self.out, "  invoke void @{}({}) to label %tcont unwind label %tlp", cb.body, args.join(", ")).unwrap();
                    self.out.push_str(landing_pad);
                    self.out.push_str("tcont:\n");
                } else {
                    let zero = match hr {
                        "double" => "0.0",
                        "i1" => "false",
                        _ => "0",
                    };
                    writeln!(self.out, "  %r.ok = invoke {hr} @{}({}) to label %tok unwind label %tlp", cb.body, args.join(", ")).unwrap();
                    self.out.push_str(landing_pad);
                    self.out.push_str("tok:\n  br label %tcont\ntcont:\n");
                    writeln!(self.out, "  %r = phi {hr} [ %r.ok, %tok ], [ {zero}, %tlp ]").unwrap();
                }
            } else if empty {
                writeln!(self.out, "  call void @{}({})", cb.body, args.join(", ")).unwrap();
            } else {
                writeln!(self.out, "  %r = call {hr} @{}({})", cb.body, args.join(", ")).unwrap();
            }
            // Uma struct devolvida: os bytes copiados já (antes de qualquer
            // alocação); com exceção o corpo devolve 0 e o retorno é zerado.
            if let Some((_, l)) = &ret {
                let sz = l.size.div_ceil(16) * 16;
                writeln!(self.out, "  call void @llvm.memset.p0.i64(ptr %rt, i8 0, i64 {sz}, i1 false)").unwrap();
                writeln!(self.out, "  call void @dartforge_ffi_copiar_composto(ptr %rt, i64 %r, i64 {})", l.size).unwrap();
            }
            writeln!(self.out, "  %x = call i8 @dartforge_ffi_callback_sair(ptr %ctx, ptr %saida)").unwrap();
            match (&cb.ret, &ret) {
                (NativeType::Prim(CType::Void), _) => {
                    writeln!(self.out, "  ret void\n}}\n").unwrap();
                    continue;
                }
                (_, Some((RetPassing::Sret { .. }, l))) => {
                    writeln!(self.out, "  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %rt, i64 {}, i1 false)", l.size).unwrap();
                    writeln!(self.out, "  ret void\n}}\n").unwrap();
                    continue;
                }
                (_, Some((RetPassing::Direct(pieces), _))) => {
                    let mut current = "undef".to_string();
                    for (k, p) in pieces.iter().enumerate() {
                        writeln!(self.out, "  %rp{k} = getelementptr i8, ptr %rt, i64 {}", p.offset).unwrap();
                        writeln!(self.out, "  %rv{k} = load {}, ptr %rp{k}, align 1", p.ty).unwrap();
                        if pieces.len() == 1 {
                            current = format!("%rv{k}");
                        } else {
                            writeln!(self.out, "  %ra{k} = insertvalue {r} {current}, {} %rv{k}, {k}", p.ty).unwrap();
                            current = format!("%ra{k}");
                        }
                    }
                    writeln!(self.out, "  ret {r} {current}\n}}\n").unwrap();
                    continue;
                }
                _ => {}
            }
            let NativeType::Prim(tr) = cb.ret else { unreachable!("retorno composto sem passagem") };
            writeln!(self.out, "  %e_excecao = icmp ne i8 %x, 0\n  br i1 %e_excecao, label %excecao, label %normal").unwrap();
            // O retorno excepcional (bits do tipo C) e o normal (Dart → C).
            writeln!(self.out, "excecao:\n  %e = load i64, ptr %saida").unwrap();
            let exc = match tr {
                CType::I64 | CType::U64 => "add i64 %e, 0".to_string(),
                CType::F64 => "bitcast i64 %e to double".to_string(),
                CType::F32 => {
                    writeln!(self.out, "  %ed = bitcast i64 %e to double").unwrap();
                    "fptrunc double %ed to float".to_string()
                }
                CType::Ptr | CType::Handle => "inttoptr i64 %e to ptr".to_string(),
                narrow => format!("trunc i64 %e to {}", narrow.llvm()),
            };
            writeln!(self.out, "  %ev = {exc}\n  ret {r} %ev").unwrap();
            let normal = match tr {
                CType::I64 | CType::U64 | CType::F64 | CType::Bool => None,
                CType::F32 => Some("fptrunc double %r to float".to_string()),
                CType::Ptr | CType::Handle => Some("inttoptr i64 %r to ptr".to_string()),
                narrow => Some(format!("trunc i64 %r to {}", narrow.llvm())),
            };
            match normal {
                Some(c) => writeln!(self.out, "normal:\n  %nv = {c}\n  ret {r} %nv\n}}\n").unwrap(),
                None => writeln!(self.out, "normal:\n  ret {r} %r\n}}\n").unwrap(),
            }
        }
    }

    fn call_main(&mut self) {
        let Some(entry) = self.module.entry_symbol.clone() else { return };
        let by_tables = self.module.excecoes_por_tabelas;
        let n = self.module.entry_params.min(2);
        if n == 0 {
            writeln!(self.out, "  {}", entry_call(by_tables, &entry, &[])).unwrap();
            return;
        }
        writeln!(self.out, "  %df.args = call i64 @dartforge_argumentos_do_main()").unwrap();
        let args: &[&str] = if n == 1 { &["i64 %df.args"] } else { &["i64 %df.args", "i64 0"] };
        writeln!(self.out, "  {}", entry_call(by_tables, &entry, args)).unwrap();
    }

    /// Exceções por tabelas: as portas pelas quais o runtime chama código
    /// Dart (`runtime/src/excecoes_tabelas.rs`), uma por assinatura —
    /// `@df.porta.<v|r><n>(função, n words)`, `v` sem valor e `r`
    /// devolvendo uma palavra — e a tabela `@df.ports`, no índice
    /// `2n + (1 se devolve)`, que a entrada do programa registra. A porta
    /// chama a função com `invoke`; um desenrolamento pousa nela, que
    /// restaura o topo da pilha-sombra e volta com a exceção pendente — o
    /// que o runtime espera de uma função Dart que lançou. Assim nenhum
    /// quadro do runtime fica no caminho de um desenrolamento.
    fn emit_ports(&mut self) {
        let mut table = Vec::with_capacity(N_PORTS);
        for n in 0..N_PORTS / 2 {
            for returns in [false, true] {
                let name_str = format!("df.porta.{}{n}", if returns { 'r' } else { 'v' });
                let params: String = (0..n).map(|i| format!(", i64 %a{i}")).collect();
                let args: Vec<String> = (0..n).map(|i| format!("i64 %a{i}")).collect();
                let (typ, result, back, pattern) =
                    if returns { ("i64", "%r = ", "ret i64 %r", "ret i64 0") } else { ("void", "", "ret void", "ret void") };
                writeln!(self.out, "define internal {typ} @{name_str}(ptr %f{params}) personality ptr @dartforge_personalidade {{")
                    .unwrap();
                self.out.push_str(
                    "entrada:\n  %ctx = call ptr @dartforge_contexto()\n  %tp = getelementptr inbounds i8, ptr %ctx, i64 8\n  %topo = load ptr, ptr %tp, align 8\n",
                );
                writeln!(self.out, "  {result}invoke {typ} %f({}) to label %volta unwind label %pouso", args.join(", ")).unwrap();
                writeln!(
                    self.out,
                    "volta:\n  {back}\npouso:\n  %lp = landingpad {{ ptr, i32 }} catch ptr null\n  store ptr %topo, ptr %tp, align 8\n  {pattern}\n}}"
                )
                .unwrap();
                table.push(format!("ptr @{name_str}"));
            }
        }
        writeln!(self.out, "@df.portas = private unnamed_addr constant [{N_PORTS} x ptr] [{}]\n", table.join(", ")).unwrap();
    }

    /// Tipo do valor que o emissor de fato imprime para uma instrucao.
    ///
    /// Nao basta acreditar no tipo registrado na HIR: varios arms imprimem um
    /// tipo fixo (todo Add sai como i64, todo ICmp como i1) e um registro
    /// divergente faria a coercao trabalhar com a informacao errada. O tipo
    /// registrado so vale onde o emissor o usa (Load, Phi, Alloca e as
    /// instrucoes ainda nao expandidas).
    pub(crate) fn result_type(inst: &Instruction, registered: Type) -> Type {
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
            Instruction::ArcLoadStrong { .. }
            | Instruction::ArcCopy { .. }
            | Instruction::ArcMove { .. }
            | Instruction::AllocObject { .. }
            | Instruction::AllocList { .. }
            | Instruction::AllocRecord { .. }
            | Instruction::Box { .. }
            | Instruction::AllocCell { .. }
            | Instruction::AllocEnv { .. }
            | Instruction::ConcatStrings { .. }
            | Instruction::AllocClosure { .. }
            | Instruction::AllocClosureTyped { .. }
            | Instruction::TearOff { .. }
            | Instruction::CallClosure { .. }
            | Instruction::CallSelector { .. }
            | Instruction::CallSelectorForward { .. }
            | Instruction::CallClosureForward { .. } => Type::Ref,
            Instruction::NativeCall { ret, .. } => ret.hir_type(),
            Instruction::TypedCall { ret, .. } => *ret,
            Instruction::NativeLoad { ty: typ, .. } => typ.hir_type(),
            Instruction::NativeWrite { .. } => Type::Void,
            Instruction::CompositeNativeCall { ret, dest, .. } => {
                if dest.is_some() { Type::Void } else { ret.hir_type() }
            }
            Instruction::CellSet { .. } | Instruction::ArcDrop { .. } | Instruction::ArcStoreStrong { .. } => Type::Void,
            Instruction::ConstArray(_) | Instruction::FunctionTable(_) => Type::Ptr,
            Instruction::Unbox { to, .. } => *to,
            Instruction::Alloca(_) => Type::Ptr,
            Instruction::GetField { .. } => registered,
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
            _ => registered,
        }
    }


    /// A conferência da pilha no prólogo (o `stack_overflow_check` da VM,
    /// `runtime/vm/compiler/backend/il.h:9693`, `CheckStackOverflowInstr`): o
    /// endereço de um `alloca` do quadro abaixo de
    /// `Context::limite_da_pilha` lança `StackOverflowError`
    /// (`dartforge_estouro_de_pilha`) e a função volta com a exceção
    /// pendente, antes de encadear o quadro de raízes. Só nas funções que
    /// chamam (as que têm `%ctx`): uma folha não aprofunda a pilha.
    fn emit_stack_check(&mut self, func: &Function) {
        let by_tables = self.tab.is_some();
        let o = &mut self.out;
        writeln!(o, "  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 {}", layout::contexto::LIMITE_DA_PILHA).unwrap();
        writeln!(o, "  %pilhal = load ptr, ptr %pilhalp, align 8").unwrap();
        writeln!(o, "  %pilhab = icmp ult ptr %pilhaq, %pilhal").unwrap();
        writeln!(o, "  %pilhax = call i1 @llvm.expect.i1(i1 %pilhab, i1 false)").unwrap();
        writeln!(o, "  br i1 %pilhax, label %pilha.estouro, label %pilha.ok").unwrap();
        writeln!(o, "pilha.estouro:").unwrap();
        writeln!(o, "  call void @dartforge_estouro_de_pilha()").unwrap();
        if self.stack_trace.is_some() {
            writeln!(o, "  call void @dartforge_rastro_saida()").unwrap();
        }
        // Exceções por tabelas: o `StackOverflowError` desenrola daqui.
        if by_tables {
            writeln!(o, "  call void @df.lancar()").unwrap();
            writeln!(o, "  unreachable").unwrap();
            writeln!(o, "pilha.ok:").unwrap();
            self.current_label = "pilha.ok".to_string();
            self.exit_labels.entry(0).or_insert_with(|| "pilha.ok".to_string());
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
        self.current_label = "pilha.ok".to_string();
        self.exit_labels.entry(0).or_insert_with(|| "pilha.ok".to_string());
    }

    /// O corpo do objeto `only` (`%fcb{v}`, o cabeçalho) e os campos dele
    /// (`%fp{v}`), em linha: um objeto do espaço de objetos (`h & 3 == 2` e
    /// `h > 0`; `crates/runtime/src/heap.rs`, `Cabecalho`) tem o cabeçalho em
    /// `h - 2`; qualquer outra coisa — null, `Smi`, valor do runtime — usa o
    /// `OBJETO_VAZIO` (o ponteiro em `Context::vazios`, deslocamento 40 do
    /// contexto), campos zerados, como `dartforge_object_campos` dava. Com o
    /// bit `FORA` nas `flags`, o corpo é o endereço guardado no primeiro
    /// campo (o objeto que mudou de número de campos): uma seleção, sem
    /// desvio. Sem chamada.
    /// O operando é um objeto recém-alocado neste bloco, sem ponto de
    /// coleta desde a alocação (`fresh_set`).
    fn fresh(&self, op: &Operand) -> bool {
        !self.module.arc_memory && matches!(op, Operand::Val(x) if self.fresh_set.contains(&x.0))
    }

    /// O corpo do objeto `op` é o próprio bloco: o recém-alocado, ou o
    /// `this` quando nenhum objeto tem corpo de fora (`this_object`).
    fn direct_body(&self, op: &Operand) -> bool {
        self.fresh(op) || matches!(op, Operand::Val(x) if self.this_object == Some(x.0))
    }

    /// [`Self::emit_field_addresses`]; `direct`: o corpo é o próprio
    /// bloco ([`Self::direct_body`], `handle − 2`), sem a seleção de
    /// null/`Smi` nem o corpo de fora.
    fn emit_field_addresses_of(&mut self, v: u32, only: &str, direct: bool) {
        if !direct {
            return self.emit_field_addresses(v, only);
        }
        let o = &mut self.out;
        writeln!(o, "  %fxa{v} = add i64 {only}, -{}", layout::DESLOCAMENTO_DO_HANDLE).unwrap();
        writeln!(o, "  %fcb{v} = inttoptr i64 %fxa{v} to ptr").unwrap();
        writeln!(o, "  %fp{v} = getelementptr inbounds i8, ptr %fcb{v}, i64 16").unwrap();
    }

    fn emit_field_addresses(&mut self, v: u32, only: &str) {
        let o = &mut self.out;
        if self.fields_per_call {
            writeln!(o, "  %fcb{v} = call ptr @df.corpo(i64 {only}, ptr %ctx)").unwrap();
            writeln!(o, "  %fp{v} = getelementptr inbounds i8, ptr %fcb{v}, i64 16").unwrap();
            return;
        }
        // O bit de sinal entra na máscara: negativo nunca é objeto.
        writeln!(o, "  %fxk{v} = and i64 {only}, {OBJECT_MASK}").unwrap();
        writeln!(o, "  %fxo{v} = icmp eq i64 %fxk{v}, 2").unwrap();
        writeln!(o, "  %fxa{v} = add i64 {only}, -2").unwrap();
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
    fn emit_field_write(&mut self, v: u32, suffix: &str, idx: usize, sv: &str, is_ref: &str) {
        let o = &mut self.out;
        let t = format!("{v}{suffix}");
        writeln!(o, "  %fg{t} = getelementptr inbounds i64, ptr %fp{v}, i64 {idx}").unwrap();
        writeln!(o, "  store i64 {sv}, ptr %fg{t}, align 8").unwrap();
        let (word, width, bit) = if idx < 32 {
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
        let (cleans, lights_up) = if width == "i32" {
            (i64::from(!(1u32 << bit) as i32), i64::from((1u32 << bit) as i32))
        } else {
            (!(1u64 << bit) as i64, (1u64 << bit) as i64)
        };
        writeln!(o, "  %fm{t} = load {width}, ptr {word}, align 4").unwrap();
        writeln!(o, "  %fmc{t} = and {width} %fm{t}, {cleans}").unwrap();
        match is_ref {
            "0" => writeln!(o, "  store {width} %fmc{t}, ptr {word}, align 4").unwrap(),
            "1" => {
                writeln!(o, "  %fms{t} = or {width} %fm{t}, {lights_up}").unwrap();
                writeln!(o, "  store {width} %fms{t}, ptr {word}, align 4").unwrap();
            }
            r => {
                writeln!(o, "  %fmz{t} = zext i8 {r} to {width}").unwrap();
                writeln!(o, "  %fmb{t} = shl {width} %fmz{t}, {bit}").unwrap();
                writeln!(o, "  %fms{t} = or {width} %fmc{t}, %fmb{t}").unwrap();
                writeln!(o, "  store {width} %fms{t}, ptr {word}, align 4").unwrap();
            }
        }
    }

    /// `dartforge_object_new(c, n)` com classe e número de campos
    /// constantes e `n ≤ TLAB_N`: a alocação sai em linha
    /// ([`LlvmEmitter::emit_inline_allocation`]). Devolve `(c, n)`.
    fn inline_allocation(inst: &Instruction) -> Option<(i64, i64)> {
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
    /// da faixa da TLAB de `n` campos do isolado (`Context::tlab`: cursor
    /// em `64 + 16n`, fim em `72 + 16n`; os blocos já foram contados como
    /// alocados e têm cabeçalho e campos zerados, menos o encadeamento no
    /// primeiro campo), grava o cabeçalho (jovem, `n`, a classe) e devolve o
    /// handle (`block + 2`). Faixa esgotada — ou,
    /// para a classe que registra a tabela de métodos na primeira alocação
    /// (`dartforge_object_new_t`), classe ainda não registrada — vai ao
    /// runtime, que coleta se preciso e reabastece. O bloco da HIR termina
    /// no rótulo `ao{v}.end` ([`LlvmEmitter::exit_labels`]).
    fn emit_inline_allocation(&mut self, v: u32, c: i64, n: i64) {
        let table = self.module.table_functions.get(&(c as u32)).cloned();
        if let Some(f) = &table {
            self.annotate_external(f, Type::Ptr, &[]);
        }
        // A TLAB é por palavras do corpo: `n ≤ TLAB_N` campos ocupam
        // `max(n, 1)` palavras (sem extensão do mapa).
        let w = layout::palavras_de_instancia(n as usize);
        let size = layout::bytes_do_bloco(w);
        if self.out_of_line_allocation {
            // Produção do programa grande: a mesma conta, numa chamada ao
            // `@df.new_instance` (docs/NATIVO-PRODUCAO-GRANDE.md §3).
            let header = layout::palavra_do_cabecalho(layout::estado::JOVEM, layout::flags::INSTANCIA, n as usize, c as i32);
            let t = table.as_ref().map_or("null".to_string(), |f| format!("@{f}"));
            writeln!(
                self.out,
                "  %v{v} = call i64 @df.nova_instancia(ptr %ctx, i64 {}, i64 {size}, i64 {}, i64 {c}, i64 {n}, ptr {t})",
                layout::contexto::tlab_cursor(w),
                header as i64
            )
            .unwrap();
            return;
        }
        let o = &mut self.out;
        writeln!(o, "  %ta{v} = getelementptr inbounds i8, ptr %ctx, i64 {}", layout::contexto::tlab_cursor(w)).unwrap();
        writeln!(o, "  %tb{v} = load ptr, ptr %ta{v}, align 8").unwrap();
        writeln!(o, "  %tfa{v} = getelementptr inbounds i8, ptr %ctx, i64 {}", layout::contexto::tlab_fim(w)).unwrap();
        writeln!(o, "  %tf{v} = load ptr, ptr %tfa{v}, align 8").unwrap();
        writeln!(o, "  %tnx{v} = getelementptr i8, ptr %tb{v}, i64 {size}").unwrap();
        writeln!(o, "  %tz{v} = icmp ugt ptr %tnx{v}, %tf{v}").unwrap();
        if table.is_some() {
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
        let header = layout::palavra_do_cabecalho(layout::estado::JOVEM, layout::flags::INSTANCIA, n as usize, c as i32);
        writeln!(o, "ao{v}.rapido:").unwrap();
        writeln!(o, "  store ptr %tnx{v}, ptr %ta{v}, align 8").unwrap();
        writeln!(o, "  %tpp{v} = getelementptr inbounds i8, ptr %tb{v}, i64 16").unwrap();
        writeln!(o, "  store i64 0, ptr %tpp{v}, align 8").unwrap();
        writeln!(o, "  store i64 {}, ptr %tb{v}, align 8", header as i64).unwrap();
        writeln!(o, "  %thb{v} = ptrtoint ptr %tb{v} to i64").unwrap();
        writeln!(o, "  %th{v} = add i64 %thb{v}, 2").unwrap();
        writeln!(o, "  br label %ao{v}.fim").unwrap();
        writeln!(o, "ao{v}.lento:").unwrap();
        match &table {
            Some(f) => writeln!(o, "  %tl{v} = call i64 @dartforge_object_new_t(i64 {c}, i64 {n}, ptr @{f})").unwrap(),
            None => writeln!(o, "  %tl{v} = call i64 @dartforge_object_new(i64 {c}, i64 {n})").unwrap(),
        }
        writeln!(o, "  br label %ao{v}.fim").unwrap();
        writeln!(o, "ao{v}.fim:").unwrap();
        writeln!(o, "  %v{v} = phi i64 [ %th{v}, %ao{v}.rapido ], [ %tl{v}, %ao{v}.lento ]").unwrap();
        self.current_label = format!("ao{v}.fim");
    }

    /// A gravação em linha de campo que pode pôr um `Ref` num objeto: leva a
    /// barreira de escrita ([`LlvmEmitter::emit_barrier`]). Uma constante
    /// escalar ou null não precisa.
    fn inline_barrier(&self, inst: &Instruction) -> bool {
        // No ARC a gravação vai ao runtime, com a barreira dentro.
        if self.module.arc_memory {
            return false;
        }
        let may_be_ref = |op: &Operand| {
            !matches!(op, Operand::Constant(Constant::Null | Constant::Int(_) | Constant::Bool(_) | Constant::Double(_)))
        };
        match inst {
            Instruction::SetField { index, value, .. } => {
                *index < INLINE_FIELDS && self.type_of(value) == Type::Ref && may_be_ref(value)
            }
            Instruction::CallRuntime { name, args, .. }
                if name == "dartforge_object_set"
                    && args.len() == 4
                    && matches!(args.get(1), Some((Operand::Constant(Constant::Int(i)), _)) if (0..INLINE_FIELDS as i64).contains(i)) =>
            {
                let scalar = matches!(&args[3].0, Operand::Constant(Constant::Int(0)) | Operand::Constant(Constant::Bool(false)));
                !scalar && may_be_ref(&args[2].0)
            }
            _ => false,
        }
    }

    /// A barreira de escrita depois de gravar o `Ref` `sv` no objeto `only`
    /// (`@df.barrier`, docs/NATIVO-ESPACO-UNIFICADO.md §2.7): o objeto velho
    /// que recebe um filho jovem vai para os lembrados da próxima coleta menor —
    /// o *store buffer* da barreira da VM. `dynamic`: o `is_ref` só conhecido em
    /// execução (0/1, `i8`). O bloco da HIR passa a terminar em `wb{v}.end`.
    fn emit_barrier(&mut self, v: u32, only: &str, sv: &str, dynamic: Option<&str>) {
        let o = &mut self.out;
        match dynamic {
            Some(r) => {
                writeln!(o, "  %wbr{v} = icmp ne i8 {r}, 0").unwrap();
                writeln!(o, "  br i1 %wbr{v}, label %wb{v}.ref, label %wb{v}.fim").unwrap();
                writeln!(o, "wb{v}.ref:").unwrap();
                writeln!(o, "  call void @df.barreira(i64 {only}, i64 {sv})").unwrap();
                writeln!(o, "  br label %wb{v}.fim").unwrap();
            }
            None => {
                writeln!(o, "  call void @df.barreira(i64 {only}, i64 {sv})").unwrap();
                writeln!(o, "  br label %wb{v}.fim").unwrap();
            }
        }
        writeln!(o, "wb{v}.fim:").unwrap();
        self.current_label = format!("wb{v}.fim");
    }

    /// Contexto exigido pelo corpo ou pelas tabelas, antes de decidir os
    /// quadros de raízes. Também exige conferência de pilha no prólogo;
    /// os resumos ARC compartilham esta decisão para não omitir unwind.
    /// Quadros calculados por vivacidade podem exigir contexto adicional.
    pub(crate) fn requires_explicit_context(func: &Function, tab: Option<&FunctionTables>) -> bool {
        func.blocks.iter().any(|b| {
            b.instructions.iter().any(|(_, i, _)| {
                matches!(i, Instruction::CallRuntime { name, .. } if name == "dartforge_exception_pending")
                    || Self::uses_context(i)
            })
        }) || tab.is_some_and(|t| {
            !t.landing_pads.is_empty() || t.checks_stack
                || t.exits.values().any(|s| *s == ExceptionExit::Guard)
        })
    }

    /// A instrução lê o contexto da thread (`%ctx`): campos em linha ou
    /// alocação em linha.
    fn uses_context(inst: &Instruction) -> bool {
        match inst {
            Instruction::GetField { index, .. } | Instruction::SetField { index, .. } => *index < INLINE_FIELDS,
            Instruction::AllocObject { fields, .. } => !fields.is_empty(),
            Instruction::CallRuntime { name, args, .. } => {
                Self::inline_allocation(inst).is_some()
                    || name == "dartforge_exception_clear"
                    || ((name == "dartforge_object_get" || (name == "dartforge_object_set" && args.len() == 4))
                        && matches!(args.get(1), Some((Operand::Constant(Constant::Int(i)), _)) if (0..INLINE_FIELDS as i64).contains(i)))
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
    fn may_collect(&self, inst: &Instruction) -> bool {
        let scalar = |op: &Operand| {
            self.type_of(op) != Type::Ref && !matches!(op, Operand::Constant(Constant::String(_) | Constant::StringWtf8(_)))
        };
        let exact = |op: &Operand, t: Type| match op {
            Operand::Constant(Constant::String(_) | Constant::StringWtf8(_)) => false,
            Operand::Constant(Constant::Null) => t == Type::Ref,
            _ => {
                let a = self.type_of(op);
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
            | Instruction::FCmp(_, a, b) => !(scalar(a) && scalar(b)),
            Instruction::Neg(a)
            | Instruction::Not(a)
            | Instruction::FNeg(a)
            | Instruction::LNot(a)
            | Instruction::IntToDouble(a)
            | Instruction::DoubleToInt(a) => !scalar(a),
            Instruction::Alloca(_) | Instruction::Load { .. } | Instruction::Phi { .. } => false,
            // Aritmética de vetores e o desencaixe (carga direta): não alocam.
            Instruction::Simd { .. } => false,
            Instruction::Unbox { to, .. } if to.is_vector() => false,
            // Em linha (load/store no vetor de campos), sem conversão que
            // aloque: um `Ref` já é `i64`, um `double` é `bitcast`.
            Instruction::GetField { index, .. } => (*index as usize) >= INLINE_FIELDS,
            Instruction::SetField { index, value, .. } => {
                (*index as usize) >= INLINE_FIELDS
                    || matches!(value, Operand::Constant(Constant::String(_) | Constant::StringWtf8(_)))
            }
            Instruction::Store { ptr, val } => {
                let t = match ptr {
                    Operand::Val(p) => self.pointee.get(p).copied().unwrap_or(Type::I64),
                    _ => return true,
                };
                !exact(val, t)
            }
            Instruction::NativeLoad { address, index: idx_value, .. } => !(scalar(address) && scalar(idx_value)),
            Instruction::NativeWrite { address, index: idx_value, value: val_ref, .. } => {
                !(scalar(address) && scalar(idx_value) && scalar(val_ref))
            }
            Instruction::CallRuntime { name, args, .. } => {
                let e = externs::effects_of(name);
                let does_not_allocate = !e.allocates && !e.calls_dart;
                !(does_not_allocate && args.iter().all(|(a, t)| exact(a, *t)))
            }
            // Na memória por rastreamento: o texto constante é um objeto
            // estático da imagem (`ir_strings.rs`, `emit_const_string`), e
            // os vetores constantes e a leitura de um vetor de argumentos são
            // endereços e cargas. Nenhum aloca; antes, cada literal de texto
            // obrigava a enraizar tudo o que estava vivo nele.
            Instruction::Const(Constant::String(_) | Constant::StringWtf8(_)) => {
                !(self.static_objects && !self.module.arc_memory)
            }
            Instruction::LoadIndexed { .. } | Instruction::ConstArray(_) | Instruction::FunctionTable(_) => {
                self.module.arc_memory
            }
            _ => true,
        }
    }

    /// Se a chamada `inst` enraíza os próprios argumentos
    /// (`roots::analyze_with`): uma chamada direta a uma função Dart — deste
    /// módulo, ou do SDK da fonte (`df.dart$3a…`) — cujos operandos chegam
    /// sem conversão que aloque (o tipo do parâmetro, nenhum texto
    /// constante). Só na pilha-sombra da memória por rastreamento: as raízes
    /// por mapas e o ARC seguem a regra de sempre. Só com
    /// `DARTFORGE_RAIZES_NO_CHAMADO=1` ([`roots_in_callee`]).
    fn callee_roots_the_arguments(&self, inst: &Instruction) -> bool {
        if self.maps || self.module.arc_memory || !roots_in_callee() {
            return false;
        }
        let Instruction::CallStatic { symbol, args, .. } = inst else { return false };
        let alvo = self.function_by_symbol.get(symbol.as_str()).copied();
        if alvo.is_none() && !symbol.starts_with("df.dart$3a") {
            return false;
        }
        args.iter().enumerate().all(|(i, a)| {
            if matches!(a, Operand::Constant(Constant::String(_) | Constant::StringWtf8(_))) {
                return false;
            }
            let t = self.type_of(a);
            match alvo.and_then(|f| f.params.get(i)).map(|p| p.2) {
                Some(p) => p == t || (p != Type::Ref && t != Type::Ref && p != Type::Void),
                None => true,
            }
        })
    }

    fn type_of(&self, op: &Operand) -> Type {
        match op {
            Operand::Val(v) => self.types.get(v).copied().unwrap_or(Type::I64),
            Operand::Constant(Constant::Int(_)) => Type::I64,
            Operand::Constant(Constant::Double(_)) => Type::F64,
            Operand::Constant(Constant::Bool(_)) => Type::I1,
            Operand::Constant(Constant::Null) => Type::Ref,
            Operand::Constant(Constant::String(_) | Constant::StringWtf8(_)) => Type::Ref,
            Operand::Constant(Constant::Function(_)) => Type::I64,
        }
    }

    fn width(t: Type) -> u32 {
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
    fn constant_in_type(&self, op: &Operand, alvo: Type) -> Option<String> {
        let Operand::Constant(c) = op else { return None };
        let s = match (c, alvo) {
            (Constant::String(_) | Constant::StringWtf8(_), _) => panic!("string deve ser carregada via Instruction::Const"),
            (Constant::Function(f), Type::I64) => format!("ptrtoint (ptr @{f} to i64)"),
            (Constant::Function(_), _) => panic!("endereço de função só como i64"),
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
    fn coerce(&mut self, op: &Operand, alvo: Type) -> String {
        if alvo == Type::Void {
            return self.operand_str(op);
        }
        if let Some(s) = self.constant_in_type(op, alvo) {
            return s;
        }
        let mut current = self.type_of(op);
        let mut text = self.operand_str(op);
        if current.llvm_ir() == alvo.llvm_ir() && (current == Type::F64) == (alvo == Type::F64) {
            return text;
        }
        // double vira i64 (e vice-versa) sempre pelos bits; larguras menores
        // passam antes por i64 porque bitcast exige tamanho igual.
        if current == Type::F64 && alvo != Type::F64 {
            text = self.emit_conversion("bitcast", Type::F64, &text, Type::I64);
            current = Type::I64;
        }
        if alvo == Type::F64 {
            if current != Type::I64 && current != Type::Ref {
                text = self.emit_conversion("zext", current, &text, Type::I64);
                current = Type::I64;
            }
            if current != Type::F64 {
                text = self.emit_conversion("bitcast", Type::I64, &text, Type::F64);
            }
            return text;
        }
        if Self::width(current) < Self::width(alvo) {
            self.emit_conversion("zext", current, &text, alvo)
        } else if Self::width(current) > Self::width(alvo) {
            self.emit_conversion("trunc", current, &text, alvo)
        } else {
            text
        }
    }

    /// Mesma conversao, mas gravando num nome escolhido por quem chama
    /// (usado pelas entradas de phi, que precisam de um nome estavel).
    fn emit_named_conversion(&mut self, name_str: &str, de: Type, origin: &str, to_repr: Type) {
        let mut current = de;
        let mut text = origin.to_string();
        if current == Type::F64 && to_repr != Type::F64 && Self::width(to_repr) != 64 {
            text = self.emit_conversion("bitcast", Type::F64, &text, Type::I64);
            current = Type::I64;
        }
        if to_repr == Type::F64 && current != Type::F64 && Self::width(current) != 64 {
            text = self.emit_conversion("zext", current, &text, Type::I64);
            current = Type::I64;
        }
        let op = if current == Type::F64 || to_repr == Type::F64 {
            "bitcast"
        } else if Self::width(current) < Self::width(to_repr) {
            "zext"
        } else {
            "trunc"
        };
        writeln!(self.out, "  {name_str} = {op} {} {text} to {}", current.llvm_ir(), to_repr.llvm_ir()).unwrap();
    }

    fn emit_conversion(&mut self, op: &str, de: Type, text: &str, to_repr: Type) -> String {
        let c = self.next_coercion;
        self.next_coercion += 1;
        writeln!(self.out, "  %c{c} = {op} {} {text} to {}", de.llvm_ir(), to_repr.llvm_ir()).unwrap();
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
            Operand::Constant(Constant::Function(f)) => format!("ptrtoint (ptr @{f} to i64)"),
        }
    }
}

/// Raízes por mapas: as chamadas que nunca coletam além das externs que a
/// tabela de efeitos marca assim — os ajudantes folha ([`leaf_helpers`]),
/// o desenrolamento e a moldura da conferência dos efeitos.
const MAP_LEAVES: &[&str] = &[
    "df.corpo",
    "df.barreira",
    "df.barreira_elemento",
    "df.e_objeto",
    "df.filho_jovem",
    "df.lancar",
    "dartforge_efeitos_antes",
    "dartforge_efeitos_depois",
    "dartforge_efeitos_nivel",
    "dartforge_efeitos_restaurar",
];

/// A regra das raízes pelo chamado (`LlvmEmitter::callee_roots_the_arguments`)
/// está ligada: `DARTFORGE_RAIZES_NO_CHAMADO=1` (medida; desligada por
/// padrão). Ela tira ~8% das gravações de raiz do new_sali/backend, mas
/// muda a coloração dos slots: um slot compartilhado guarda o último valor
/// até ser reusado ou o quadro fechar, e com menos valores enraizados o
/// valor morto fica mais tempo — o núcleo `json` do `bench/comparison`
/// terminava com 1,05 milhão de objetos vivos em vez de 650 mil (pico de
/// 121 contra 95 MB). Falta zerar o slot do valor que morre antes de ligar.
fn roots_in_callee() -> bool {
    static ENABLED_IN_CALLEE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED_IN_CALLEE.get_or_init(|| std::env::var("DARTFORGE_RAIZES_NO_CHAMADO").is_ok_and(|v| v == "1"))
}

/// Raízes por mapas: a chamada da linha pode coletar — a indireta, e a
/// direta a uma função que não é intrínseco do LLVM nem folha (a extern que
/// a tabela de efeitos marca sem coletar nem chamar Dart, salvo a sabotagem
/// `leaf:<name>`, que a trata como folha; os ajudantes de
/// [`MAP_LEAVES`]). Um nome fora das tabelas (a função Dart, o
/// ajudante que aloca) coleta.
fn collecting_call(line: &str) -> bool {
    match stack_trace::call_target(line) {
        None => false,
        Some(None) => true,
        Some(Some(name_str)) => {
            if name_str.starts_with("llvm.") || MAP_LEAVES.contains(&name_str) || crate::alvo::sabotaged_leaf(name_str) {
                return false;
            }
            let ef = externs::effects_of(name_str);
            ef.allocates || ef.calls_dart
        }
    }
}

/// Raízes por mapas (§14.12): a linha (com ou sem o `\n`) com o operando
/// `beam` (`"deopt"(…)`), se ela é uma chamada que pode coletar ainda sem
/// ele; senão, a própria linha. O operando vai depois dos atributos da
/// chamada e antes do rótulo de um `invoke` e dos metadados.
fn with_deopt<'a>(line: &'a str, beam: &str) -> std::borrow::Cow<'a, str> {
    if line.contains("\"deopt\"(") || !collecting_call(line) {
        return std::borrow::Cow::Borrowed(line);
    }
    let (body, end) = match line.strip_suffix('\n') {
        Some(c) => (c, "\n"),
        None => (line, ""),
    };
    let pos = [body.find(" to label "), body.find(", !")].into_iter().flatten().min().unwrap_or(body.len());
    std::borrow::Cow::Owned(format!("{} [ {beam} ]{}{end}", &body[..pos], &body[pos..]))
}

/// Raízes por mapas (§14.12): o `"deopt"()` vazio em toda chamada que pode
/// coletar e ainda não tem o dela — a dos ajudantes `@df.*`, da entrada, das
/// funções sem raízes no mapa. O inliner do LLVM só junta os vivos de quem
/// chama ao `"deopt"` que a chamada copiada já tem: sem ele, a chamada do
/// corpo copiado sairia sem as raízes de quem chamou. Os intermediários
/// `@df.vararg.<k>` ficam como estão (`noinline`; a chamada variádica não
/// vira statepoint). Toda linha entre o `define` e o `}` conta: o corpo dos
/// ajudantes sai sem indentação (a continuação `\` das strings Rust come os
/// espaços).
fn empty_deopt(text: &str) -> String {
    let mut exit = String::with_capacity(text.len() + text.len() / 16);
    let mut inside = false;
    let mut intermediate = false;
    for line in text.split_inclusive('\n') {
        if line.starts_with("define ") {
            inside = true;
            intermediate = line.contains("@df.vararg.");
            exit.push_str(line);
            continue;
        }
        if line.starts_with('}') {
            inside = false;
        }
        if inside && !intermediate {
            exit.push_str(&with_deopt(line, "\"deopt\"()"));
        } else {
            exit.push_str(line);
        }
    }
    exit
}

/// Raízes por mapas: os ajudantes `@df.*` que comprovadamente não coletam
/// ganham `"gc-leaf-function"` — fora de linha (o programa grande), a
/// chamada a eles não vira ponto de coleta. Em linha o atributo não muda
/// nada. Só os que não chamam o runtime, ou chamam uma extern que não aloca
/// no heap do coletor (`dartforge_lembrar`).
fn leaf_helpers(text: &str) -> String {
    const LEAVES: &[&str] = &["@df.corpo(", "@df.barreira(", "@df.barreira_elemento(", "@df.e_objeto(", "@df.filho_jovem("];
    let mut exit = String::with_capacity(text.len() + 256);
    for line in text.split_inclusive('\n') {
        let alvo = line.starts_with("define internal ") && LEAVES.iter().any(|f| line.contains(f));
        match line.rfind(" {") {
            Some(i) if alvo => {
                exit.push_str(&line[..i]);
                exit.push_str(" \"gc-leaf-function\"");
                exit.push_str(&line[i..]);
            }
            _ => exit.push_str(line),
        }
    }
    exit
}

/// Quantas portas Rust → Dart o módulo do programa define
/// ([`LlvmEmitter::emit_ports`]): aridades 0 a 7, sem valor e com valor. O
/// runtime tem o mesmo número (`PORTAS_DART`, `excecoes_tabelas.rs`).
const N_PORTS: usize = 16;

/// O que todo módulo declara nas exceções por tabelas
/// (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13):
///
/// * a personalidade das funções com pouso (`dartforge_personalidade`, do
///   runtime) e o registro das portas;
/// * `@df.throw`, o desenrolamento: uma exceção estruturada do sistema com
///   o código próprio `0xE0444652` (o inteiro abaixo), não continuável, sem
///   parâmetros — a exceção Dart em si é a pendência do runtime. `internal`:
///   cada módulo (e cada parte de um módulo dividido) tem a sua cópia.
const EXCEPTIONS_BY_TABLES: &str = "; Exceções por tabelas (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md)\n\
declare i32 @dartforge_personalidade(...)\n\
declare void @dartforge_registrar_portas(ptr, i64)\n\
declare dllimport void @RaiseException(i32, i32, i32, ptr)\n\
define internal void @df.lancar() noreturn noinline cold \"gc-leaf-function\" {\n\
  call void @RaiseException(i32 -532396462, i32 1, i32 0, ptr null)\n\
  unreachable\n\
}\n";

/// O mesmo nos alvos Itanium (Linux, macOS): `@df.throw` pede ao runtime
/// o objeto de exceção da thread (`excecoes_tabelas.rs`) e o entrega ele
/// mesmo ao desenrolador do sistema (`_Unwind_RaiseException`), como o
/// `RaiseException` do Windows: nenhum quadro Rust fica entre o lançamento
/// e o pouso. Na variante do runtime com `panic=unwind` (a DLL do SDK da
/// fonte), uma função Rust `extern "C"` no caminho tem a guarda de abortar,
/// e a personalidade do Rust devolvia `_URC_FATAL_PHASE1_ERROR` (3) à
/// exceção estrangeira. Se o desenrolador voltar, o código vai ao runtime,
/// que encerra.
const EXCEPTIONS_BY_TABLES_ITANIUM: &str = "; Exceções por tabelas (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md)
declare i32 @dartforge_personalidade(...)
declare i32 @dartforge_personalidade_cleanup_itanium(...)
declare i32 @llvm.eh.typeid.for(ptr)
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
/// ([`LlvmEmitter::emit_ports`]) — a entrada é chamada pelo runtime, e um
/// desenrolamento não pode chegar a ele.
fn entry_call(by_tables: bool, sym_name: &str, args: &[&str]) -> String {
    if by_tables {
        let mut list = format!("ptr @{sym_name}");
        for a in args {
            list.push_str(", ");
            list.push_str(a);
        }
        format!("call void @df.porta.v{}({list})", args.len())
    } else {
        format!("call void @{sym_name}({})", args.join(", "))
    }
}

/// O hash (FNV-1a de 64 bits) do nome de um slot ou da chave do módulo na
/// área de globais; nunca 0, que marca o slot que não migra.
fn slot_hash(name_str: &str) -> i64 {
    let h = fnv(name_str.bytes()) & !(CACHE_BIT as u64);
    (if h == 0 { 1 } else { h }) as i64
}

/// O bit que marca, no descritor da área, o nome de um slot de cache de
/// seletor (o runtime tem a mesma constante, `gc_raizes.rs`).
pub(crate) const CACHE_BIT: i64 = 1 << 62;

/// O nome da palavra `word` (0 ou 1) do cache `name`, com [`CACHE_BIT`].
fn cache_hash(name_str: &str, word: u8) -> i64 {
    (fnv(name_str.bytes().chain([0, word])) | CACHE_BIT as u64) as i64
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
const GET_AREA: &str = "@df.area_id = internal global i64 0, align 8\n\
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
/// `Cabecalho`) tem o `class_id` no cabeçalho (`block + 4`, e o handle é
/// `block + 2`): uma carga, como o `LoadClassId` da VM. Um valor do runtime
/// num slot (`h > 0`, `h & 3 == 0`, índice `h / 4 - 1`) tem a classe no
/// vetor denso do heap (`Heap::classes`, 8 bytes por slot, o `cid` nos 4
/// primeiros), cujo endereço e comprimento o contexto da thread publica
/// (deslocamentos 336 e 344, `runtime/src/heap.rs`, `Context`): relidos a
/// cada uso, porque o vetor cresce (muda de endereço) quando o heap ganha
/// slots. Índice fora da faixa ou classe desconhecida (`i32::MIN`), e o
/// resto (`null` e `Smi` sem o SDK da fonte), perguntam ao runtime
/// (`dartforge_value_class`).
///
/// `df.subclasse` é o `cid <: C` pelo mapa de bits do alvo que o runtime
/// monta na primeira consulta (`runtime/src/core.rs`, `MapasDeSubtipo`;
/// deslocamentos 352–368 do contexto), o *type testing stub* da VM: sem
/// chamada quando o mapa existe e o `cid` está na largura dele.
/// O cache (`cache[0]` = classe + 1, `cache[1]` = entrada, na área do
/// isolado) é conferido aqui; só a falha chama `dartforge_seletor`, que
/// busca na tabela da classe e regrava o cache — o *inline cache*
/// monomórfico da VM (`ICData`), sem a chamada ao runtime no acerto.
///
/// Com o SDK da fonte, `null` e o `Smi` também saem sem o runtime: as
/// classes deles (`Null`, `_Smi`) são as da tabela `cids` do módulo
/// (`sdk_modulo::runtime_cids`, a que o runtime recebe na partida).
fn value_class(cids: &[i64]) -> String {
    let text = VALUE_CLASS
        .replace("@@MASCARA@@", &OBJECT_MASK.to_string())
        .replace("@@CLASSE@@", &(layout::desl::CLASSE as i64 - layout::DESLOCAMENTO_DO_HANDLE).to_string())
        .replace("@@SUBTIPOS@@", &layout::contexto::SUBTIPOS.to_string())
        .replace("@@N_SUBTIPOS@@", &layout::contexto::N_SUBTIPOS.to_string())
        .replace("@@LARGURA@@", &layout::contexto::LARGURA_SUBTIPOS.to_string());
    // Sem o SDK da fonte (o caminho legado), `null` e o `Smi` vão ao runtime.
    if !matches!(cids, [n, s, ..] if *n >= 0 && *s >= 0) {
        return text.replace("@@RAPIDOS@@", "");
    }
    let fast_set = format!(
        "  %z = icmp eq i64 %h, 0\n\
  br i1 %z, label %nulo, label %s0\n\
nulo:\n\
  ret i64 {null}\n\
s0:\n\
  %b = and i64 %h, 1\n\
  %i = icmp ne i64 %b, 0\n\
  br i1 %i, label %smi, label %s1\n\
smi:\n\
  ret i64 {smi}\n\
s1:\n",
        null = layout::cid::NULL,
        smi = layout::cid::SMI
    );
    text.replace("@@RAPIDOS@@", &fast_set)
}

const VALUE_CLASS: &str = "define internal i64 @df.classe(i64 %h) alwaysinline {\n\
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

/// Ajudantes `@df.*` que vão fora de linha (com o `selector` e a `class`, o
/// despacho dinâmico, em [`out_of_line_helpers`]): cada um é uma
/// sequência que se repete em centenas de milhares de lugares do programa
/// grande (docs/NATIVO-PRODUCAO-GRANDE.md §1.4).
pub const OUTLINED_HELPERS_LARGE_PROGRAM: &[&str] =
    &["subclasse", "alocar", "barreira", "barreira_elemento", "obter_area", "nova_instancia"];

/// A lista de ajudantes fora de linha: `DARTFORGE_AJUDANTES_FORA` (lista
/// separada por vírgula; `1`, todos; vazio, nenhum); senão todos no
/// programa grande, no desenvolvimento (`-O0`, onde o `alwaysinline` copia
/// o ajudante em cada uso e a geração de código paga por cada cópia:
/// `large_development`) e na produção (`large_program`), e nenhum
/// nos demais — o programa pequeno roda o dobro do tempo no `-O0` sem os
/// ajudantes em linha (o corpus/native: 53 → 116 s).
pub fn out_of_line_helpers(large_development: bool, large_program: bool) -> Vec<String> {
    let all = || ["seletor", "classe"].iter().chain(OUTLINED_HELPERS_LARGE_PROGRAM).map(|s| (*s).to_string()).collect();
    match std::env::var("DARTFORGE_AJUDANTES_FORA") {
        Ok(v) if v == "1" => all(),
        Ok(v) => v.split(',').map(str::trim).filter(|s| !s.is_empty()).map(str::to_string).collect(),
        Err(_) if large_development || large_program => all(),
        Err(_) => Vec::new(),
    }
}

/// O despacho por seletor com o descritor estático (`@df.seld.<k>`), fora
/// de linha ([`LlvmEmitter::compact_selector`]).
const SELECTOR_BY_DESCRIPTOR: &str = "define internal ptr @df.seletor_d(ptr %c, i64 %r, ptr %d) noinline {
  %h = load i64, ptr %d, align 8
  %np = getelementptr inbounds i8, ptr %d, i64 8
  %n = load ptr, ptr %np, align 8
  %lp = getelementptr inbounds i8, ptr %d, i64 16
  %l = load i64, ptr %lp, align 8
  %f = call ptr @df.seletor(ptr %c, i64 %r, i64 %h, ptr %n, i64 %l)
  ret ptr %f
}
";

/// A alocação de instância de `emit_inline_allocation` fora de linha:
/// `%cur` é o deslocamento do cursor da TLAB das palavras do objeto no
/// contexto (o fim vem 8 bytes depois), `%sz` os bytes do bloco, `%hdr` a
/// palavra do cabeçalho, `%tab` a função da tabela de métodos (null quando a
/// classe não registra a tabela na primeira alocação).
fn new_instance() -> String {
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

/// `text` com os `define internal … @df.<name>(…) alwaysinline` de `names`
/// trocados para `noinline`.
fn out_of_line(text: &str, names: &[String]) -> String {
    if names.is_empty() {
        return text.to_string();
    }
    let mut exit = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        let alvo = line.starts_with("define internal ")
            && names.iter().any(|n| line.contains(&format!("@df.{n}(")));
        if alvo {
            exit.push_str(&line.replacen(" alwaysinline", " noinline", 1));
        } else {
            exit.push_str(line);
        }
    }
    exit
}

/// Os ajudantes da fundação do espaço unificado (docs/NATIVO-ESPACO-UNIFICADO.md
/// §3.5), com os deslocamentos do contrato de layout:
///
/// * `@df.e_objeto(h)`: `h & (7 | i64::MIN) == 2`;
/// * `@df.filho_jovem(v)`: o filho gravado pede a barreira (objeto jovem);
/// * `@df.barrier(o, v)`: `o` velho recebendo filho jovem → `dartforge_lembrar`
///   (peso 1:9 no ramo lento, como a barreira da Julia);
/// * `@df.barreira_elemento(o, i, v)`: a mesma com cartões (`o` velho ou
///   lembrado com `CARTOES`: suja o cartão do elemento `i`);
/// * `@df.allocate(header, w)`: a TLAB de `w ≤ TLAB_N` palavras (o cursor
///   avança `16 + 8w` e grava a palavra 0); esgotada ou `w` maior,
///   `dartforge_alocar(cid, w, header >> 8)`.
/// `@df.body(h, ctx)`: o começo do corpo de um objeto (o cabeçalho; os
/// campos 16 bytes depois), a mesma conta de `emit_field_addresses` em
/// linha — um valor que não é objeto lê o objeto de reserva do contexto
/// (`ctx + 40`), e um corpo fora do bloco (anexo, `flags & 1`) é seguido pelo
/// ponteiro em `+16`. `alwaysinline` quando o emissor está em linha (para o
/// otimizador ver igual); sem o atributo no desenvolvimento, uma chamada.
fn body_helper(per_call: bool, production: bool) -> String {
    // Na produção por chamada: fora de linha de verdade, e sem efeito nem
    // dependência de memória (`memory(none)`): o corpo de um objeto não
    // muda durante a vida dele — o coletor não move, e o corpo de fora só
    // nasce na migração da recarga do JIT (que nunca é produção) —, então o
    // otimizador junta todas as chamadas sobre o mesmo objeto, mesmo com
    // gravações e chamadas no meio (o quadro do `$async`, o `this`).
    // Antes, `memory(read)`: duas chamadas só se juntavam sem escrita entre
    // elas. docs/NATIVO-PRODUCAO-GRANDE.md §6.3.
    let attribute = match (per_call, production) {
        (false, _) => " alwaysinline",
        (true, false) => "",
        (true, true) => " noinline nounwind willreturn memory(none)",
    };
    format!(
        "define internal ptr @df.corpo(i64 %o, ptr %ctx){attribute} {{
  %k = and i64 %o, {OBJECT_MASK}
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

fn space_helpers() -> String {
    use layout::{contexto, desl, estado, flags};
    let d = |x: usize| x as i64 - layout::DESLOCAMENTO_DO_HANDLE;
    format!(
        "define internal i1 @df.e_objeto(i64 %h) alwaysinline {{\n\
  %m = and i64 %h, {mask}\n\
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
  %m = and i64 %v, {mask}\n\
  %o = icmp eq i64 %m, 2\n\
  br i1 %o, label %obj, label %nao\n\
obj:\n\
  %p = inttoptr i64 %v to ptr\n\
  %ep = getelementptr inbounds i8, ptr %p, i64 {state_h}\n\
  %e = load i8, ptr %ep, align 8\n\
  %j = icmp eq i8 %e, {young}\n\
  ret i1 %j\n\
nao:\n\
  ret i1 false\n\
}}\n\
define internal void @df.barreira(i64 %o, i64 %v) alwaysinline {{\n\
  %p = inttoptr i64 %o to ptr\n\
  %ep = getelementptr inbounds i8, ptr %p, i64 {state_h}\n\
  %e = load i8, ptr %ep, align 8\n\
  %velho = icmp eq i8 %e, {old_one}\n\
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
  %ep = getelementptr inbounds i8, ptr %p, i64 {state_h}\n\
  %e = load i8, ptr %ep, align 8\n\
  %e3 = sub i8 %e, {old_one}\n\
  %velho = icmp ult i8 %e3, 2\n\
  %x = call i1 @llvm.expect.i1(i1 %velho, i1 false)\n\
  br i1 %x, label %filho, label %fim\n\
filho:\n\
  %j = call i1 @df.filho_jovem(i64 %v)\n\
  br i1 %j, label %lento, label %fim\n\
lento:\n\
  %fp = getelementptr inbounds i8, ptr %p, i64 {flags_h}\n\
  %f = load i8, ptr %fp, align 1\n\
  %fc = and i8 %f, {cards}\n\
  %temc = icmp ne i8 %fc, 0\n\
  br i1 %temc, label %cartao, label %lembrar\n\
cartao:\n\
  %lp = getelementptr inbounds i8, ptr %p, i64 {length_h}\n\
  %len = load i64, ptr %lp, align 8\n\
  %w = lshr i64 %i, {log_word}\n\
  %k = add i64 %len, %w\n\
  %kb = shl i64 %k, 3\n\
  %kd = add i64 %kb, {elements_h}\n\
  %cp = getelementptr inbounds i8, ptr %p, i64 %kd\n\
  %cv = load i64, ptr %cp, align 8\n\
  %s = lshr i64 %i, {log_card}\n\
  %s63 = and i64 %s, 63\n\
  %bit = shl i64 1, %s63\n\
  %nv = or i64 %cv, %bit\n\
  store i64 %nv, ptr %cp, align 8\n\
  br label %lembrar\n\
lembrar:\n\
  %ev = icmp eq i8 %e, {old_one}\n\
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
  %tam = add i64 %w8, {header}\n\
  %tnx = getelementptr i8, ptr %tb, i64 %tam\n\
  %esgotada = icmp ugt ptr %tnx, %tf\n\
  %x = call i1 @llvm.expect.i1(i1 %esgotada, i1 false)\n\
  br i1 %x, label %lento, label %rapido\n\
rapido:\n\
  store ptr %tnx, ptr %ta, align 8\n\
  store i64 %cab, ptr %tb, align 8\n\
  %hb = ptrtoint ptr %tb to i64\n\
  %h = add i64 %hb, {offset}\n\
  ret i64 %h\n\
lento:\n\
  %cid = lshr i64 %cab, 32\n\
  %fl = lshr i64 %cab, 8\n\
  %fn = and i64 %fl, 16777215\n\
  %r = call i64 @dartforge_alocar(i64 %cid, i64 %w, i64 %fn)\n\
  ret i64 %r\n\
}}\n",
        mask = OBJECT_MASK,
        state_h = d(desl::ESTADO),
        flags_h = d(desl::FLAGS),
        length_h = d(desl::COMPRIMENTO),
        elements_h = d(desl::ELEMENTOS),
        young = estado::JOVEM,
        old_one = estado::VELHO,
        cards = flags::CARTOES,
        log_word = layout::ELEMENTOS_POR_PALAVRA_DE_CARTAO.trailing_zeros(),
        log_card = layout::ELEMENTOS_POR_CARTAO.trailing_zeros(),
        tlab_n = layout::TLAB_N,
        tlab = contexto::TLAB,
        header = layout::TAMANHO_DO_CABECALHO,
        offset = layout::DESLOCAMENTO_DO_HANDLE,
    )
}

// `@df.caixa_int`, `@df.desencaixa_int` e `@df.env_ref` são da P2
// (`ir_boxes::HELPERS`, docs/NATIVO-ESPACO-UNIFICADO.md §3.5).

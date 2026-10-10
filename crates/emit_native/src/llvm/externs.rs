//! Tabela das externs do runtime: a declaração LLVM e os EFEITOS de cada uma.
//!
//! Fonte única de verdade (docs/NATIVO-PLANO.md §6.5, "tabela de efeitos"):
//! o emissor declara as externs a partir daqui, e o passo 6 do plano vai
//! usar os efeitos para (a) não exigir raízes vivas nem quadro em volta de
//! uma extern que não aloca — o equivalente das entradas LEAF da VM
//! (`runtime_entry.cc:778`) e do `gc-leaf-function` do Dartino — e (b)
//! dispensar a verificação de exceção pendente depois de uma extern que não
//! lança ("não aloca ⇒ não lança": lançar aloca o erro e o rastro).
//!
//! Nesta etapa todas estão marcadas de forma conservadora (aloca, lança):
//! uma marca otimista errada é exatamente o defeito dos stack maps errados,
//! só que do nosso lado — ela só pode ser trocada junto com um teste que
//! roda a extern sob `DARTFORGE_GC_STRESS=1` e exige zero coletas.

/// O que uma chamada à extern pode fazer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Efeitos {
    /// Pode alocar no heap (logo, coletar): é ponto de coleta.
    pub aloca: bool,
    /// Pode deixar uma exceção pendente.
    pub lanca: bool,
    /// Pode chamar código Dart gerado (que, por sua vez, aloca e lança).
    pub chama_dart: bool,
}

/// Aloca (ponto de coleta), mas não lança nem chama código Dart: as
/// alocações e leituras simples do runtime (conferidas no código de cada
/// uma). `otimizar/efeitos.rs` conta com isso para saber quem não lança.
pub const ALOCA_SEM_LANCAR: Efeitos = Efeitos {
    aloca: true,
    lanca: false,
    chama_dart: false,
};

/// Marca conservadora: aloca e lança.
pub const CONSERVADOR: Efeitos = Efeitos {
    aloca: true,
    lanca: true,
    chama_dart: false,
};

/// Uma extern do runtime: a declaração LLVM e os efeitos.
pub struct Extern {
    pub decl: &'static str,
    pub efeitos: Efeitos,
}

impl Extern {
    /// O nome do símbolo (`dartforge_…`), entre o `@` e o `(`.
    pub fn nome(&self) -> &'static str {
        let ini = self.decl.find('@').map_or(0, |i| i + 1);
        let fim = self.decl[ini..]
            .find('(')
            .map_or(self.decl.len(), |i| ini + i);
        &self.decl[ini..fim]
    }
}

/// Todas as externs que o código gerado pode chamar.
pub const EXTERNS: &[Extern] = &[
    Extern {
        decl: "declare i64 @dartforge_string_new(ptr, i64)",
        efeitos: ALOCA_SEM_LANCAR,
    },
    Extern {
        decl: "declare i64 @dartforge_string_concat(i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i8 @dartforge_equal(i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_register_class_name(i64, ptr, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_object_new(i64, i64)",
        efeitos: ALOCA_SEM_LANCAR,
    },
    Extern {
        decl: "declare i64 @dartforge_object_get(i64, i64)",
        // Lê/grava um campo: não aloca (o emissor a expande em linha com
        // índice constante).
        efeitos: Efeitos { aloca: false, lanca: false, chama_dart: false },
    },
    Extern {
        // `_StackTrace.toString` (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md
        // §13.14): o texto guardado, ou os endereços do `throw`
        // simbolizados agora (aloca o texto).
        decl: "declare i64 @dartforge_rastro_texto(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        // O registro da tabela do rastro da imagem (§13.14): não aloca nem
        // lança.
        decl: "declare void @dartforge_registrar_rastro(ptr, ptr)",
        efeitos: Efeitos { aloca: false, lanca: false, chama_dart: false },
    },
    Extern {
        // A posição de um campo do `dart:async` que o rastro percorre
        // (§13.14): não aloca no heap do coletor nem lança.
        decl: "declare void @dartforge_registrar_campo_do_rastro(i64, ptr, i64, i64)",
        efeitos: Efeitos { aloca: false, lanca: false, chama_dart: false },
    },
    Extern {
        // As entradas de tear-off que os ramos de stream do rastro
        // reconhecem (§13.14). Não aloca no heap do coletor nem lança.
        decl: "declare void @dartforge_registrar_tearoff_do_rastro(ptr, i64)",
        efeitos: Efeitos { aloca: false, lanca: false, chama_dart: false },
    },
    Extern {
        // A pilha do rastro (§13.14): o quadro de um corpo `async` ou o
        // ouvinte de `handleValue`, empurrado na entrada; devolve a
        // profundidade. Não aloca no heap do coletor nem lança.
        decl: "declare i64 @dartforge_rastro_entrar(i64, i64)",
        efeitos: Efeitos { aloca: false, lanca: false, chama_dart: false },
    },
    Extern {
        // A saída: corta a pilha do rastro na profundidade da entrada.
        decl: "declare void @dartforge_rastro_sair(i64)",
        efeitos: Efeitos { aloca: false, lanca: false, chama_dart: false },
    },
    Extern {
        // A barreira de escrita (`llvm/mod.rs`, `emitir_barreira`): lembra o
        // objeto velho; não aloca nem lança.
        decl: "declare void @dartforge_lembrar(i64) nounwind",
        efeitos: Efeitos { aloca: false, lanca: false, chama_dart: false },
    },
    Extern {
        // A memória ARC (docs/ARC-IMPLEMENTACAO.md): liga o ARC na entrada
        // (uma coleta completa por rastreamento e o registro dos vivos).
        decl: "declare void @dartforge_memoria_arc_v1()",
        efeitos: Efeitos { aloca: true, lanca: false, chama_dart: false },
    },
    Extern {
        // A gravação contada de um `Ref` numa palavra de corpo `REFS` (o
        // elemento de lista no ARC): não aloca nem lança.
        decl: "declare void @dartforge_arc_gravar_ref(i64, i64, i64) nounwind",
        efeitos: Efeitos { aloca: false, lanca: false, chama_dart: false },
    },
    Extern {
        // ARC puro: as referências gravadas em linha num objeto recém
        // alocado passam a contar. Não aloca nem lança.
        decl: "declare void @dartforge_arc_inicial(i64) nounwind",
        efeitos: Efeitos { aloca: false, lanca: false, chama_dart: false },
    },
    Extern {
        decl: "declare void @dartforge_arc_global_receber_v1(i64, i64) nounwind",
        efeitos: Efeitos { aloca: false, lanca: false, chama_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_box_double_owned_v1(double) nounwind",
        efeitos: Efeitos { aloca: true, lanca: false, chama_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_ler_campo_escalar_v1(i64, i64) nounwind",
        efeitos: Efeitos { aloca: false, lanca: false, chama_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_box_int_owned_v1(i64) nounwind",
        efeitos: Efeitos { aloca: true, lanca: false, chama_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_excecao_owned_v1() nounwind",
        efeitos: Efeitos { aloca: true, lanca: false, chama_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_rastro_owned_v1() nounwind",
        efeitos: Efeitos { aloca: true, lanca: false, chama_dart: false },
    },
    Extern {
        decl: "declare void @dartforge_arc_lancar_com_rastro_ref_v1(i64, i64)",
        efeitos: Efeitos { aloca: true, lanca: true, chama_dart: false },
    },
    Extern {
        decl: "declare void @dartforge_arc_lancar_ref_v1(i64)",
        efeitos: Efeitos { aloca: true, lanca: true, chama_dart: false },
    },
    Extern {
        decl: "declare i8 @dartforge_arc_observar_heap_v1(i64) nounwind",
        efeitos: Efeitos { aloca: false, lanca: false, chama_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_quadro_abrir_v1(i64) nounwind",
        efeitos: Efeitos { aloca: false, lanca: false, chama_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_quadro_carregar_v1(i64, i64) nounwind",
        efeitos: Efeitos { aloca: false, lanca: false, chama_dart: false },
    },
    Extern {
        decl: "declare void @dartforge_arc_quadro_receber_v1(i64, i64, i64) nounwind",
        efeitos: Efeitos { aloca: false, lanca: false, chama_dart: false },
    },
    Extern {
        decl: "declare void @dartforge_arc_quadro_copiar_v1(i64, i64, i64) nounwind",
        efeitos: Efeitos { aloca: false, lanca: false, chama_dart: false },
    },
    Extern {
        decl: "declare void @dartforge_arc_quadro_mover_v1(i64, i64, i64, i64) nounwind",
        efeitos: Efeitos { aloca: false, lanca: false, chama_dart: false },
    },
    Extern {
        decl: "declare void @dartforge_arc_quadro_fechar_v1(i64) nounwind",
        efeitos: Efeitos { aloca: false, lanca: false, chama_dart: false },
    },
    Extern {
        decl: "declare void @dartforge_arc_retain(i64) nounwind",
        efeitos: Efeitos { aloca: false, lanca: false, chama_dart: false },
    },
    Extern {
        decl: "declare void @dartforge_arc_release(i64) nounwind",
        efeitos: Efeitos { aloca: false, lanca: false, chama_dart: false },
    },
    Extern {
        decl: "declare void @dartforge_arc_collect() nounwind",
        efeitos: Efeitos { aloca: true, lanca: false, chama_dart: false },
    },
    Extern {
        decl: "declare i8 @dartforge_arc_verificar_abi(i64) nounwind",
        efeitos: Efeitos { aloca: false, lanca: false, chama_dart: false },
    },
    Extern {
        decl: "declare void @dartforge_object_set(i64, i64, i64, i8)",
        // Lê/grava um campo: não aloca (o emissor a expande em linha com
        // índice constante).
        efeitos: Efeitos { aloca: false, lanca: false, chama_dart: false },
    },
    Extern {
        decl: "declare i8 @dartforge_late_field_initialized(i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_late_field_mark_initialized(i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_tearoff(i64)",
        efeitos: ALOCA_SEM_LANCAR,
    },
    Extern {
        decl: "declare void @dartforge_gc_global_root(i64, i64)",
        efeitos: ALOCA_SEM_LANCAR,
    },
    Extern {
        decl: "declare void @dartforge_marcar_constante(i64, i64)",
        efeitos: ALOCA_SEM_LANCAR,
    },
    Extern {
        decl: "declare i8 @dartforge_exception_capturavel()",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_registrar_isolados(ptr, ptr)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_ffi_registrar_tipo(i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_ffi_registrar_trampolim(ptr, i64, ptr)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_ffi_endereco_da_closure(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_ffi_endereco_do_ponteiro(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_ffi_ponteiro_de_retorno(i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_typed_novo(i64, i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_typed_novo_t(i64, i64, i64, ptr)",
        efeitos: CONSERVADOR,
    },
    // O caminho rápido das listas tipadas (`lower/tipados.rs`): funções
    // puras do handle. O comprimento, o endereço dos elementos, o tipo e a
    // imutabilidade de uma lista tipada não mudam enquanto ela vive, e um
    // handle só é reusado depois que o objeto morre — quando o código não
    // o usa mais. Então `memory(none)`: o LLVM as tira dos laços apesar das
    // chamadas do registro de raízes. `speculatable`: sobre um handle
    // qualquer (até um que não é lista tipada) respondem 0, sem efeito.
    // O contexto da thread (`runtime/src/heap.rs`, `Contexto`): o mesmo
    // endereço enquanto a thread vive — pura para o código de uma ativação.
    Extern {
        decl: "declare ptr @dartforge_contexto() memory(none) nounwind willreturn speculatable",
        efeitos: Efeitos { aloca: false, lanca: false, chama_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_typed_len(i64, i64, i64) memory(none) nounwind willreturn speculatable",
        efeitos: Efeitos { aloca: false, lanca: false, chama_dart: false },
    },
    // `fillRange` de lista tipada (`lower/tipados.rs`, `preencher_tipada`):
    // 1 se preencheu, 0 para o caminho do SDK.
    Extern {
        decl: "declare i64 @dartforge_typed_fill_int(i64, i64, i64, i64, i64) nounwind",
        efeitos: Efeitos { aloca: false, lanca: false, chama_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_typed_fill_double(i64, i64, i64, i64, double) nounwind",
        efeitos: Efeitos { aloca: false, lanca: false, chama_dart: false },
    },
    // A interpolação com partes `int` sem caixa: pares (espécie, bits).
    Extern {
        decl: "declare i64 @dartforge_string_juntar_tipado(ptr, i64) nounwind",
        efeitos: ALOCA_SEM_LANCAR,
    },
    // A ABI de quem chama conferida contra a da closure (`closures.rs`).
    Extern {
        decl: "declare i64 @dartforge_closure_tipada(i64, i64) memory(inaccessiblemem: read) nounwind willreturn",
        efeitos: Efeitos { aloca: false, lanca: false, chama_dart: false },
    },
    // Os campos dos objetos em linha (`GetField`/`SetField`): o endereço
    // vem do vetor de campos no heap do runtime, que só muda de tamanho por
    // chamadas sem atributo; os campos são memória que o módulo acessa.
    Extern {
        decl: "declare i64 @dartforge_object_campos(i64) memory(inaccessiblemem: read) nounwind willreturn speculatable",
        efeitos: Efeitos { aloca: false, lanca: false, chama_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_view_nova(i64, i64, i64, i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_view_nova_t(i64, i64, i64, i64, i64, ptr)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_unbox_int(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare double @dartforge_unbox_double(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i8 @dartforge_unbox_bool(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_exception_peek_ref()",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_exception_throw(i64, i8)",
        efeitos: CONSERVADOR,
    },
    Extern {
        // O pedido de interrupção do isolado (J01): só lê um byte.
        decl: "declare i8 @dartforge_interrupcao_pendente()",
        efeitos: Efeitos { aloca: false, lanca: false, chama_dart: false },
    },
    Extern {
        // O ponto seguro de laço: atende o controle; um `kill` imediato
        // deixa a exceção pendente (não capturável).
        decl: "declare void @dartforge_ponto_seguro()",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i8 @dartforge_exception_pending()",
        // Só lê ou limpa a pendência: não aloca.
        efeitos: Efeitos { aloca: false, lanca: false, chama_dart: false },
    },
    Extern {
        decl: "declare void @dartforge_exception_clear()",
        // Só lê ou limpa a pendência: não aloca.
        efeitos: Efeitos { aloca: false, lanca: false, chama_dart: false },
    },
    Extern {
        decl: "declare void @dartforge_register_subclass(i64, i64)",
        efeitos: CONSERVADOR,
    },
    // O teste `v is C` pela classe: só lê o grafo de classes (e grava os
    // caches dele, `nucleo.rs`); não aloca no heap nem lança. Sem os efeitos
    // conservadores, as referências vivas não vão ao quadro de raízes a
    // cada teste, e o LLVM junta os testes iguais.
    Extern {
        decl: "declare i8 @dartforge_is_subclass(i64, i64) memory(read) nounwind willreturn",
        efeitos: Efeitos { aloca: false, lanca: false, chama_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_stack_trace_get()",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_throw_with_stack_trace(i64, i8, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_value_class(i64)",
        // A classe do valor: só lê (docs/NATIVO-ESPACO-UNIFICADO.md §3.5).
        efeitos: Efeitos { aloca: false, lanca: false, chama_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_to_string_i64(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_to_string_f64(double)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_to_string_bool(i8)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_to_string_handle(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_record_new(ptr, i64)",
        efeitos: ALOCA_SEM_LANCAR,
    },
    Extern {
        decl: "declare i64 @dartforge_collection_mark_unmodifiable(i64)",
        // Devolve uma `_ImmutableList` nova (§2.16): aloca, não lança.
        efeitos: ALOCA_SEM_LANCAR,
    },
    Extern {
        decl: "declare i64 @dartforge_exception_new(i64, i8)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_state_error_new(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_argument_error_new(i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_argument_error_value(i64, i8, i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_argument_error_not_null(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_range_error_range(i64, i64, i64, i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_unsupported_error_new(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_unimplemented_error_new(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_assertion_error_new(i64, i8)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_type_error_new()",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_null_check_error_new()",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_late_error_new(i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_stack_overflow_error_new()",
        efeitos: CONSERVADOR,
    },
    // O prólogo achou a pilha no limite (`Contexto::limite_da_pilha`).
    Extern {
        decl: "declare void @dartforge_estouro_de_pilha()",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i8 @dartforge_late_field_initializing(i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_late_field_set_initializing(i64, i64, i8)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_no_such_method_error_new(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_closure_entry(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i8 @dartforge_args_casam(ptr, ptr)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_arg_indice(ptr, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_nsm_chamada()",
        efeitos: CONSERVADOR,
    },
    // --- P2 (α): operadores sobre dynamic/num (tapa-buraco até P5) ---
    Extern {
        decl: "declare i64 @dartforge_dyn_op(i64, i64, i64)",
        efeitos: CONSERVADOR,
    },
    // --- P3 (α): iteração de lista ou conjunto ---
    Extern {
        decl: "declare i64 @dartforge_iteravel_get_ref(i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_iteravel_get_bits(i64, i64)",
        efeitos: CONSERVADOR,
    },
    // --- P5c (δ): SDK da fonte — seletores, tabelas de métodos, recusas ---
    Extern {
        decl: "declare ptr @dartforge_seletor(ptr, i64, i64, ptr, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_registrar_metodos(i64, ptr, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_membro_recusado(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_registrar_cids(ptr, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_registrar_ajudante(ptr, i64, ptr)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_registrar_nome_de_argumento(ptr, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_encaminhar_nsm(i64, i64, i64, i64, i64, i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_registrar_versao_do_sdk(ptr, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_preparar_embedder()",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_argumentos_do_main()",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare ptr @dartforge_area_de_globais(ptr)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare ptr @dartforge_area_de_globais_id(ptr, ptr)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i32 @dartforge_iniciar(ptr, ptr)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_rastro_entrada(ptr, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_rastro_saida()",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_object_new_t(i64, i64, ptr)",
        efeitos: ALOCA_SEM_LANCAR,
    },
    Extern {
        decl: "declare void @dartforge_registrar_tabela(i64, ptr)",
        efeitos: CONSERVADOR,
    },
    // --- P6: laço de eventos e os natives da sobreposição (`eventos.rs`) ---
    Extern {
        // O único ponto em que o runtime chama código Dart (G8).
        decl: "declare void @dartforge_laco_de_eventos(ptr)",
        efeitos: Efeitos { aloca: true, lanca: true, chama_dart: true },
    },
    Extern {
        decl: "declare void @dartforge_nativo_DartForge_scheduleImmediate(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_nativo_DartForge_Timer_novo(i64, i64, i8)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_nativo_DartForge_Timer_cancelar(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_nativo_Error_throwWithStackTrace(i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_nativo_Error_trySetStackTrace(i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_rti_iniciar_tabela(ptr, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_lista_de_tabela(ptr, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_lista_de_tabela_g(ptr, i64, ptr)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_ffi_registrar_composto(i64, i64, i64, i64, i64, i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_ffi_composto(i64, i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_ffi_ponteiro_novo(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_ffi_endereco_do_composto(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_ffi_composto_novo(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_ffi_handles_abrir()",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_ffi_handles_fechar(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_ffi_handle_novo(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_ffi_objeto_do_handle(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_ffi_composto_copia(i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_ffi_copiar_composto(ptr, i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_ffi_registrar_callback(ptr, i64, ptr)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_ffi_callback_entrar(ptr)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i8 @dartforge_ffi_callback_sair(ptr, ptr)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_ffi_callback_postar(ptr, ptr, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_ffi_callback_closure(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_ffi_callback_ponteiro(i64, i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_ffi_simbolo_nativo(ptr, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_typed_externo(i64, i64, i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_typed_externo_t(i64, i64, i64, i64, ptr)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_rti_receita(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_rti_avaliar(i64, i64, i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_rti_avaliar_cache(ptr, i64, i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_rti_tupla_juntar(i64, i64, i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_rti_definir(i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_rti_registro_nomeado(i64, i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i8 @dartforge_rti_e(i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_rti_como_em(i64, i64, i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_rti_texto(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_rti_objeto_tipo(i64)",
        efeitos: CONSERVADOR,
    },
    // ─── Espaço unificado (docs/NATIVO-ESPACO-UNIFICADO.md §3.6) ───────────
    Extern {
        // (cid, palavras, flags): a alocação lenta de `@df.alocar` (P0).
        decl: "declare i64 @dartforge_alocar(i64, i64, i64) nounwind",
        efeitos: ALOCA_SEM_LANCAR,
    },
    Extern {
        // (len, dois): o caminho lento de `@df.texto_alocar` (P1).
        decl: "declare i64 @dartforge_texto_novo(i64, i64) nounwind",
        efeitos: ALOCA_SEM_LANCAR,
    },
    Extern {
        // A string de uma constante vira o literal canônico (`constantes.rs`).
        decl: "declare i64 @dartforge_constante_canonica(i64) nounwind",
        efeitos: ALOCA_SEM_LANCAR,
    },
    Extern {
        // Calcula e grava o hash no cabeçalho da string (P1).
        decl: "declare i64 @dartforge_texto_hash(i64) nounwind willreturn",
        efeitos: Efeitos { aloca: false, lanca: false, chama_dart: false },
    },
    Extern {
        // As mesmas unidades? O caminho lento de `@df.texto_igual` (P1).
        decl: "declare i8 @dartforge_texto_iguais(i64, i64) memory(read) nounwind willreturn",
        efeitos: Efeitos { aloca: false, lanca: false, chama_dart: false },
    },
    Extern {
        // (palavras, n, cid, forma): a lista literal (P3).
        decl: "declare i64 @dartforge_lista_nova(ptr, i64, i64, i64) nounwind",
        efeitos: ALOCA_SEM_LANCAR,
    },
    Extern {
        // (lista, Ref): o `add` que cresce o armazenamento (P3).
        decl: "declare i64 @dartforge_lista_acrescentar(i64, i64) nounwind",
        efeitos: ALOCA_SEM_LANCAR,
    },
    Extern {
        // (início, fim): a seção dos objetos estáticos de uma imagem (P0).
        decl: "declare void @dartforge_registrar_imagem(ptr, ptr) nounwind",
        efeitos: Efeitos { aloca: false, lanca: false, chama_dart: false },
    },
    Extern {
        // (Refs, n): o record posicional (P2).
        decl: "declare i64 @dartforge_record_novo(ptr, i64) nounwind",
        efeitos: ALOCA_SEM_LANCAR,
    },
];

/// Os dados do runtime que o código gerado referencia (não são funções),
/// `(símbolo, declaração)`: nenhum. As caixas estáticas de `bool` são lidas do
/// `Contexto` da thread (`layout::contexto::VERDADEIRO`/`FALSO`), porque entre o
/// executável e a DLL do SDK cada imagem tem a sua cópia dos dados do runtime
/// (docs/NATIVO-ESPACO-UNIFICADO.md §4.10, item 9).
pub const GLOBAIS: &[(&str, &str)] = &[];

/// Efeitos da extern `nome`; desconhecida é conservadora.
pub fn efeitos_de(nome: &str) -> Efeitos {
    // Os ajudantes `@df.*` de cada pacote do espaço unificado chamados por
    // `CallRuntime` (docs/NATIVO-ESPACO-UNIFICADO.md §3.5).
    for tabela in [
        super::textos_ir::EFEITOS_DOS_AJUDANTES,
        super::caixas_ir::EFEITOS_DOS_AJUDANTES,
        super::listas_ir::EFEITOS_DOS_AJUDANTES,
        super::tipados_ir::EFEITOS_DOS_AJUDANTES,
    ] {
        if let Some(&(_, aloca, lanca)) = tabela.iter().find(|(n, _, _)| *n == nome) {
            return Efeitos { aloca, lanca, chama_dart: false };
        }
    }
    static TABELA: std::sync::OnceLock<std::collections::HashMap<&'static str, Efeitos>> = std::sync::OnceLock::new();
    // A fonte é a tabela do runtime (`crates/runtime/efeitos.tsv`,
    // docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13.8), com uma linha por
    // extern; as marcas de [`EXTERNS`] são a mesma informação ao lado da
    // declaração, e o teste `tabela_do_runtime_concorda_com_as_declaracoes`
    // as mantém iguais.
    TABELA
        .get_or_init(|| {
            dartforge_runtime::efeitos::EFEITOS
                .iter()
                .map(|&(n, aloca, lanca, chama_dart)| (n, Efeitos { aloca, lanca, chama_dart }))
                .collect()
        })
        .get(nome)
        .copied()
        .unwrap_or(CONSERVADOR)
}

/// A compilação confere a tabela de efeitos (`DARTFORGE_EFEITOS=conferir`):
/// cada chamada a uma extern marcada `coleta = 0` ou `lanca = 0` sai entre
/// `dartforge_efeitos_antes` e `dartforge_efeitos_depois`, e o runtime
/// encerra o processo, dizendo o nome, se a extern coletar ou deixar uma
/// exceção pendente (`runtime/src/efeitos_conferir.rs`). Só para os testes:
/// é assim que uma marca passa de 1 para 0.
pub fn conferir_efeitos() -> bool {
    static LIGADO: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *LIGADO.get_or_init(|| std::env::var("DARTFORGE_EFEITOS").is_ok_and(|v| v == "conferir"))
}

/// As marcas que a conferência de `nome` leva ao runtime: bit 0, não coleta;
/// bit 1, não lança. 0 = nada a conferir (ou não é uma extern da tabela).
pub fn marcas_a_conferir(nome: &str) -> i64 {
    if !dartforge_runtime::efeitos::EFEITOS.iter().any(|(n, ..)| *n == nome)
        || nome == "dartforge_efeitos_antes"
        || nome == "dartforge_efeitos_depois"
        || nome == "dartforge_efeitos_nivel"
        || nome == "dartforge_efeitos_restaurar"
    {
        return 0;
    }
    let e = efeitos_de(nome);
    // A folha sabotada (`folha:<nome>`, o D7 do §7.3) é conferida como
    // folha: a coleta dentro dela encerra o processo, dizendo o nome.
    let aloca = e.aloca && !crate::alvo::folha_sabotada(nome);
    i64::from(!aloca && !e.chama_dart) | (i64::from(!e.lanca && !e.chama_dart) << 1)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn nomes_unicos_e_bem_formados() {
        let mut nomes: Vec<&str> = EXTERNS.iter().map(Extern::nome).collect();
        assert!(
            nomes.iter().all(|n| n.starts_with("dartforge_")),
            "{nomes:?}"
        );
        let total = nomes.len();
        nomes.sort_unstable();
        nomes.dedup();
        assert_eq!(nomes.len(), total, "extern declarada duas vezes");
    }

    #[test]
    fn tabela_do_runtime_concorda_com_as_declaracoes() {
        for e in EXTERNS {
            let t = efeitos_de(e.nome());
            assert_eq!(
                (t.aloca, t.lanca),
                (e.efeitos.aloca, e.efeitos.lanca),
                "{}: a marca ao lado da declaração difere da de crates/runtime/efeitos.tsv",
                e.nome()
            );
            assert!(!e.efeitos.chama_dart || t.chama_dart, "{}: roda Dart na declaração e não na tabela", e.nome());
        }
    }

    #[test]
    fn efeitos_conservadores_por_padrao() {
        assert_eq!(efeitos_de("dartforge_list_push"), CONSERVADOR);
        assert_eq!(efeitos_de("nao_existe"), CONSERVADOR);
    }
}

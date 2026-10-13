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
pub struct Effects {
    /// Pode alocar no heap (logo, coletar): é ponto de coleta.
    pub allocates: bool,
    /// Pode deixar uma exceção pendente.
    pub throws: bool,
    /// Pode chamar código Dart gerado (que, por sua vez, aloca e lança).
    pub calls_dart: bool,
}

/// Aloca (ponto de coleta), mas não lança nem chama código Dart: as
/// alocações e leituras simples do runtime (conferidas no código de cada
/// uma). `optimize/effects.rs` conta com isso para saber quem não lança.
pub const ALLOCATES_WITHOUT_THROWING: Effects = Effects {
    allocates: true,
    throws: false,
    calls_dart: false,
};

/// Marca conservadora: aloca e lança.
pub const CONSERVATIVE: Effects = Effects {
    allocates: true,
    throws: true,
    calls_dart: false,
};

/// Uma extern do runtime: a declaração LLVM e os efeitos.
pub struct Extern {
    pub decl: &'static str,
    pub effects: Effects,
}

impl Extern {
    /// O nome do símbolo (`dartforge_…`), entre o `@` e o `(`.
    pub fn nome(&self) -> &'static str {
        let ini = self.decl.find('@').map_or(0, |i| i + 1);
        let end = self.decl[ini..]
            .find('(')
            .map_or(self.decl.len(), |i| ini + i);
        &self.decl[ini..end]
    }
}

/// Todas as externs que o código gerado pode chamar.
pub const EXTERNS: &[Extern] = &[
    Extern {
        decl: "declare i64 @dartforge_arc_closure_owned_v1(i64, i64, i64, i64)",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        decl: "declare i64 @dartforge_string_new(ptr, i64)",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        decl: "declare i64 @dartforge_string_concat(i64, i64)",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        decl: "declare i8 @dartforge_equal(i64, i64)",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare void @dartforge_register_class_name(i64, ptr, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_object_new(i64, i64)",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        decl: "declare i64 @dartforge_object_get(i64, i64)",
        // Lê/grava um campo: não aloca (o emissor a expande em linha com
        // índice constante).
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        // `_StackTrace.toString` (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md
        // §13.14): o texto guardado, ou os endereços do `throw`
        // simbolizados agora (aloca o texto).
        decl: "declare i64 @dartforge_rastro_texto(i64)",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        // O registro da tabela do rastro da imagem (§13.14): não aloca nem
        // lança.
        decl: "declare void @dartforge_registrar_rastro(ptr, ptr)",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        // A posição de um campo do `dart:async` que o rastro percorre
        // (§13.14): não aloca no heap do coletor nem lança.
        decl: "declare void @dartforge_registrar_campo_do_rastro(i64, ptr, i64, i64)",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        // As entradas de tear-off que os ramos de stream do rastro
        // reconhecem (§13.14). Não aloca no heap do coletor nem lança.
        decl: "declare void @dartforge_registrar_tearoff_do_rastro(ptr, i64)",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        // A pilha do rastro (§13.14): o quadro de um corpo `async` ou o
        // ouvinte de `handleValue`, empurrado na entrada; devolve a
        // profundidade. Não aloca no heap do coletor nem lança.
        decl: "declare i64 @dartforge_rastro_entrar(i64, i64)",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        // A saída: corta a pilha do rastro na profundidade da entrada.
        decl: "declare void @dartforge_rastro_sair(i64)",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        // A barreira de escrita (`llvm/mod.rs`, `emit_barrier`): lembra o
        // objeto velho; não aloca nem lança.
        decl: "declare void @dartforge_lembrar(i64) nounwind",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        // A memória ARC (docs/ARC-IMPLEMENTACAO.md): liga o ARC na entrada
        // (uma coleta completa por rastreamento e o registro dos vivos).
        decl: "declare void @dartforge_memoria_arc_v1()",
        effects: Effects { allocates: true, throws: false, calls_dart: false },
    },
    Extern {
        // A gravação contada de um `Ref` numa palavra de corpo `REFS` (o
        // elemento de lista no ARC): não aloca nem lança.
        decl: "declare void @dartforge_arc_gravar_ref(i64, i64, i64) nounwind",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        // ARC puro: as referências gravadas em linha num objeto recém
        // alocado passam a contar. Não aloca nem lança.
        decl: "declare void @dartforge_arc_inicial(i64) nounwind",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare void @dartforge_arc_global_receber_v1(i64, i64) nounwind",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_box_double_owned_v1(double) nounwind",
        effects: Effects { allocates: true, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_ler_campo_escalar_v1(i64, i64) nounwind",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_registrar_retorno_owned_v1(i64, i64) nounwind",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_registrar_uniforme_owned_v1(i64) nounwind",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_registrar_tipada_v1(i64, i64) nounwind",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare void @dartforge_arc_retirar_tipada_v1(i64, i64, i64) nounwind",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_alvo_tipada_v1(i64, i64) nounwind",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_invocar_uniforme_v1(i64, i64, i64, i64) nounwind",
        effects: Effects { allocates: true, throws: true, calls_dart: true },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_invocar_closure_uniforme_v1(i64, i64, i64) nounwind",
        effects: Effects { allocates: true, throws: true, calls_dart: true },
    },
    Extern {
        decl: "declare void @dartforge_arc_retirar_retorno_owned_v1(i64, i64) nounwind",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_escopo_nativo_abrir_v1() nounwind",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare void @dartforge_arc_escopo_nativo_fechar_v1(i64) nounwind",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_env_ref_v1(i64, i64) nounwind",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_env_escalar_v1(i64, i64) nounwind",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_contexto_closure_v1(i64) nounwind",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_socket_recvfrom_owned_v1(i64)",
        effects: Effects { allocates: true, throws: true, calls_dart: true },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_ler_campo_ref_v1(i64, i64) nounwind",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_ler_campo_late_ref_v1(i64, i64) nounwind",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_lista_armazenamento_ref_v1(i64) nounwind",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_lista_forma_v1(i64) nounwind",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_lista_capacidade_v1(i64) nounwind",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare void @dartforge_arc_lista_gravar_len_v1(i64, i64) nounwind",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare void @dartforge_arc_lista_gravar_bits_v1(i64, i64, i64, i64) nounwind",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare void @dartforge_arc_lista_gravar_ref_v1(i64, i64, i64) nounwind",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_lista_gravar_dados_v1(i64, i64) nounwind",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_lista_len_v1(i64) nounwind",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_lista_ler_bits_v1(i64, i64) nounwind",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_lista_ref_owned_v1(i64, i64) nounwind",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_lista_ref_v1(i64, i64) nounwind",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_argumento_owned_v1(ptr, ptr, i64) nounwind",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_box_int_owned_v1(i64) nounwind",
        effects: Effects { allocates: true, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_box_simd_owned_v1(i64, i64, i64) nounwind",
        effects: Effects { allocates: true, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_objeto_owned_v1(i64, i64) nounwind",
        effects: Effects { allocates: true, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_objeto_owned_t_v1(i64, i64, ptr) nounwind",
        effects: Effects { allocates: true, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare void @dartforge_arc_gravar_campo_escalar_v1(i64, i64, i64) nounwind",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare void @dartforge_arc_gravar_campo_escalar_conferido_v1(i64, i64, i64, ptr, i64, i64) nounwind",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_ler_campo_escalar_conferido_v1(i64, i64, ptr, i64, i64) nounwind",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare void @dartforge_arc_gravar_campo_ref_conferido_v1(i64, i64, i64, ptr, i64) nounwind",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare void @dartforge_arc_gravar_campo_ref_v1(i64, i64, i64) nounwind",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_excecao_owned_v1() nounwind",
        effects: Effects { allocates: true, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_rastro_owned_v1() nounwind",
        effects: Effects { allocates: true, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare void @dartforge_arc_lancar_com_rastro_ref_v1(i64, i64)",
        effects: Effects { allocates: true, throws: true, calls_dart: false },
    },
    Extern {
        decl: "declare void @dartforge_arc_lancar_ref_v1(i64)",
        effects: Effects { allocates: true, throws: true, calls_dart: false },
    },
    Extern {
        decl: "declare i8 @dartforge_arc_observar_heap_v1(i64) nounwind",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_quadro_abrir_v1(i64) nounwind",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_quadro_carregar_v1(i64, i64) nounwind",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare void @dartforge_arc_quadro_receber_v1(i64, i64, i64) nounwind",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare void @dartforge_arc_quadro_copiar_v1(i64, i64, i64) nounwind",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare void @dartforge_arc_quadro_mover_v1(i64, i64, i64, i64) nounwind",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare void @dartforge_arc_quadro_fechar_v1(i64) nounwind",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare void @dartforge_arc_retain(i64) nounwind",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare void @dartforge_arc_release(i64) nounwind",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare void @dartforge_arc_collect() nounwind",
        effects: Effects { allocates: true, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare i8 @dartforge_arc_verificar_abi(i64) nounwind",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare void @dartforge_object_set(i64, i64, i64, i8)",
        // Lê/grava um campo: não aloca (o emissor a expande em linha com
        // índice constante).
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare i8 @dartforge_late_field_initialized(i64, i64)",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare void @dartforge_late_field_mark_initialized(i64, i64)",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_tearoff(i64)",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        decl: "declare void @dartforge_gc_global_root(i64, i64)",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        decl: "declare void @dartforge_marcar_constante(i64, i64)",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        decl: "declare i8 @dartforge_exception_capturavel()",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare void @dartforge_registrar_isolados(ptr, ptr)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare void @dartforge_ffi_registrar_tipo(i64, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare void @dartforge_ffi_registrar_trampolim(ptr, i64, ptr)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_ffi_endereco_da_closure(i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_ffi_endereco_do_ponteiro(i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_ffi_ponteiro_de_retorno(i64, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        // A mesma entrada sob ARC, com owner do código do resultado.
        decl: "declare i64 @dartforge_arc_ffi_ponteiro_de_retorno_owned_v1(i64, i64)",
        effects: Effects { allocates: true, throws: true, calls_dart: true },
    },
    Extern {
        decl: "declare i64 @dartforge_typed_novo(i64, i64, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_typed_novo_t(i64, i64, i64, ptr)",
        effects: CONSERVATIVE,
    },
    // O caminho rápido das listas tipadas (`lower/typed_items.rs`): funções
    // puras do handle. O comprimento, o endereço dos elementos, o tipo e a
    // imutabilidade de uma lista tipada não mudam enquanto ela vive, e um
    // handle só é reusado depois que o objeto morre — quando o código não
    // o usa mais. Então `memory(none)`: o LLVM as tira dos laços apesar das
    // chamadas do registro de raízes. `speculatable`: sobre um handle
    // qualquer (até um que não é lista tipada) respondem 0, sem efeito.
    // O contexto da thread (`runtime/src/heap.rs`, `Context`): o mesmo
    // endereço enquanto a thread vive — pura para o código de uma ativação.
    Extern {
        decl: "declare ptr @dartforge_contexto() memory(none) nounwind willreturn speculatable",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_typed_len(i64, i64, i64) memory(none) nounwind willreturn speculatable",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    // `fillRange` de lista tipada (`lower/typed_items.rs`, `fill_typed`):
    // 1 se preencheu, 0 para o caminho do SDK.
    Extern {
        decl: "declare i64 @dartforge_typed_fill_int(i64, i64, i64, i64, i64) nounwind",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_typed_fill_double(i64, i64, i64, i64, double) nounwind",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    // A interpolação com partes `int` sem caixa: pares (espécie, bits).
    Extern {
        decl: "declare i64 @dartforge_string_juntar_tipado(ptr, i64) nounwind",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        // A mesma interpolação sob ARC: enraíza as partes e entrega owner.
        decl: "declare i64 @dartforge_arc_string_juntar_owned_v1(ptr, i64) nounwind",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    // A ABI de quem chama conferida contra a da closure (`closures.rs`).
    Extern {
        decl: "declare i64 @dartforge_closure_tipada(i64, i64) memory(inaccessiblemem: read) nounwind willreturn",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    // Os campos dos objetos em linha (`GetField`/`SetField`): o endereço
    // vem do vetor de campos no heap do runtime, que só muda de tamanho por
    // chamadas sem atributo; os campos são memória que o módulo acessa.
    Extern {
        decl: "declare i64 @dartforge_object_campos(i64) memory(inaccessiblemem: read) nounwind willreturn speculatable",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_view_nova(i64, i64, i64, i64, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_view_nova_t(i64, i64, i64, i64, i64, ptr)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_unbox_int(i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare double @dartforge_unbox_double(i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i8 @dartforge_unbox_bool(i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_exception_peek_ref()",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare void @dartforge_exception_throw(i64, i8)",
        effects: CONSERVATIVE,
    },
    Extern {
        // O pedido de interrupção do isolado (J01): só lê um byte.
        decl: "declare i8 @dartforge_interrupcao_pendente()",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        // O ponto seguro de laço: atende o controle; um `kill` imediato
        // deixa a exceção pendente (não capturável).
        decl: "declare void @dartforge_ponto_seguro()",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i8 @dartforge_exception_pending()",
        // Só lê ou limpa a pendência: não aloca.
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare void @dartforge_exception_clear()",
        // Só lê ou limpa a pendência: não aloca.
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare void @dartforge_register_subclass(i64, i64)",
        effects: CONSERVATIVE,
    },
    // O teste `v is C` pela classe: só lê o grafo de classes (e grava os
    // caches dele, `core.rs`); não aloca no heap nem lança. Sem os efeitos
    // conservadores, as referências vivas não vão ao quadro de raízes a
    // cada teste, e o LLVM junta os testes iguais.
    Extern {
        decl: "declare i8 @dartforge_is_subclass(i64, i64) memory(read) nounwind willreturn",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_stack_trace_get()",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare void @dartforge_throw_with_stack_trace(i64, i8, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_value_class(i64)",
        // A classe do valor: só lê (docs/NATIVO-ESPACO-UNIFICADO.md §3.5).
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_to_string_i64(i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_to_string_f64(double)",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        decl: "declare i64 @dartforge_to_string_bool(i8)",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        decl: "declare i64 @dartforge_to_string_handle(i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_record_new(ptr, i64)",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        decl: "declare i64 @dartforge_collection_mark_unmodifiable(i64)",
        // Devolve uma `_ImmutableList` nova (§2.16): aloca, não lança.
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        decl: "declare i64 @dartforge_exception_new(i64, i8)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_state_error_new(i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_argument_error_new(i64, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_argument_error_value(i64, i8, i64, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_argument_error_not_null(i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_range_error_range(i64, i64, i64, i64, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_unsupported_error_new(i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_unimplemented_error_new(i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_assertion_error_new(i64, i8)",
        effects: CONSERVATIVE,
    },
    // Os construtores de erro sob ARC (`excecoes.rs`): o objeto novo com
    // owner do código; rodam Dart num escopo nativo.
    Extern {
        decl: "declare i64 @dartforge_arc_exception_new_owned_v1(i64)",
        effects: Effects { allocates: true, throws: true, calls_dart: true },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_assertion_error_new_owned_v1(i64)",
        effects: Effects { allocates: true, throws: true, calls_dart: true },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_state_error_new_owned_v1(i64)",
        effects: Effects { allocates: true, throws: true, calls_dart: true },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_unsupported_error_new_owned_v1(i64)",
        effects: Effects { allocates: true, throws: true, calls_dart: true },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_unimplemented_error_new_owned_v1(i64)",
        effects: Effects { allocates: true, throws: true, calls_dart: true },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_argument_error_new_owned_v1(i64, i64)",
        effects: Effects { allocates: true, throws: true, calls_dart: true },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_argument_error_value_owned_v1(i64, i64, i64)",
        effects: Effects { allocates: true, throws: true, calls_dart: true },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_argument_error_not_null_owned_v1(i64)",
        effects: Effects { allocates: true, throws: true, calls_dart: true },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_range_error_range_owned_v1(i64, i64, i64, i64, i64)",
        effects: Effects { allocates: true, throws: true, calls_dart: true },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_type_error_new_owned_v1()",
        effects: Effects { allocates: true, throws: true, calls_dart: true },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_null_check_error_new_owned_v1()",
        effects: Effects { allocates: true, throws: true, calls_dart: true },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_late_error_new_owned_v1(i64, i64)",
        effects: Effects { allocates: true, throws: true, calls_dart: true },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_stack_overflow_error_new_owned_v1()",
        effects: Effects { allocates: true, throws: true, calls_dart: true },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_no_such_method_error_new_owned_v1(i64)",
        effects: Effects { allocates: true, throws: true, calls_dart: true },
    },
    Extern {
        decl: "declare i64 @dartforge_type_error_new()",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_null_check_error_new()",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_late_error_new(i64, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_stack_overflow_error_new()",
        effects: CONSERVATIVE,
    },
    // O prólogo achou a pilha no limite (`Context::limite_da_pilha`).
    Extern {
        decl: "declare void @dartforge_estouro_de_pilha()",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i8 @dartforge_late_field_initializing(i64, i64)",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare void @dartforge_late_field_set_initializing(i64, i64, i8)",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_no_such_method_error_new(i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_closure_entry(i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i8 @dartforge_args_casam(ptr, ptr)",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_arg_indice(ptr, i64)",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare void @dartforge_nsm_chamada()",
        effects: CONSERVATIVE,
    },
    // --- P2 (α): operadores sobre dynamic/num (tapa-buraco até P5) ---
    Extern {
        decl: "declare i64 @dartforge_dyn_op(i64, i64, i64)",
        effects: CONSERVATIVE,
    },
    // --- P3 (α): iteração de lista ou conjunto ---
    Extern {
        decl: "declare i64 @dartforge_iteravel_get_ref(i64, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_iteravel_get_bits(i64, i64)",
        effects: CONSERVATIVE,
    },
    // --- P5c (δ): SDK da fonte — seletores, tabelas de métodos, recusas ---
    Extern {
        decl: "declare ptr @dartforge_seletor(ptr, i64, i64, ptr, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare void @dartforge_registrar_metodos(i64, ptr, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare void @dartforge_membro_recusado(i64)",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare void @dartforge_registrar_cids(ptr, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare void @dartforge_registrar_ajudante(ptr, i64, ptr)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare void @dartforge_registrar_nome_de_argumento(ptr, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_encaminhar_nsm(i64, i64, i64, i64, i64, i64, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare void @dartforge_registrar_versao_do_sdk(ptr, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare void @dartforge_preparar_embedder()",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_argumentos_do_main()",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare ptr @dartforge_area_de_globais(ptr)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare ptr @dartforge_area_de_globais_id(ptr, ptr)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i32 @dartforge_iniciar(ptr, ptr)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare void @dartforge_rastro_entrada(ptr, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare void @dartforge_rastro_saida()",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_object_new_t(i64, i64, ptr)",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        decl: "declare void @dartforge_registrar_tabela(i64, ptr)",
        effects: CONSERVATIVE,
    },
    // --- P6: laço de eventos e os natives da sobreposição (`events.rs`) ---
    Extern {
        // O único ponto em que o runtime chama código Dart (G8).
        decl: "declare void @dartforge_laco_de_eventos(ptr)",
        effects: Effects { allocates: true, throws: true, calls_dart: true },
    },
    Extern {
        decl: "declare void @dartforge_nativo_DartForge_scheduleImmediate(i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_nativo_DartForge_Timer_novo(i64, i64, i8)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare void @dartforge_nativo_DartForge_Timer_cancelar(i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_nativo_Error_throwWithStackTrace(i64, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare void @dartforge_nativo_Error_trySetStackTrace(i64, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        // Folha pura (universo RTI, sem heap): só aborta com tabela malformada.
        decl: "declare void @dartforge_rti_iniciar_tabela(ptr, i64)",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_lista_de_tabela(ptr, i64)",
        effects: Effects { allocates: true, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_lista_de_tabela_g(ptr, i64, ptr)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare void @dartforge_ffi_registrar_composto(i64, i64, i64, i64, i64, i64, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_ffi_composto(i64, i64, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_ffi_ponteiro_novo(i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_ffi_endereco_do_composto(i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_ffi_composto_novo(i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_ffi_handles_abrir()",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare void @dartforge_ffi_handles_fechar(i64)",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_ffi_handle_novo(i64)",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_ffi_objeto_do_handle(i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_ffi_handles_fechar_com_objeto(i64, i64)",
        effects: Effects { allocates: true, throws: true, calls_dart: true },
    },
    Extern {
        // As mesmas leituras sob ARC, com owner do código do objeto.
        decl: "declare i64 @dartforge_arc_ffi_handles_fechar_com_objeto_owned_v1(i64, i64)",
        effects: Effects { allocates: true, throws: true, calls_dart: true },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_ffi_objeto_do_handle_owned_v1(i64)",
        effects: Effects { allocates: true, throws: true, calls_dart: true },
    },
    Extern {
        decl: "declare i64 @dartforge_ffi_composto_copia(i64, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare void @dartforge_ffi_copiar_composto(ptr, i64, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare void @dartforge_ffi_registrar_callback(ptr, i64, ptr)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_ffi_callback_entrar(ptr)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i8 @dartforge_ffi_callback_sair(ptr, ptr)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare void @dartforge_ffi_callback_postar(ptr, ptr, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_ffi_callback_closure(i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_ffi_callback_ponteiro(i64, i64, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        // As mesmas entradas sob ARC, com owner do código do resultado.
        decl: "declare i64 @dartforge_arc_ffi_callback_closure_owned_v1(i64) nounwind",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        decl: "declare i64 @dartforge_arc_ffi_callback_ponteiro_owned_v1(i64, i64, i64)",
        effects: Effects { allocates: true, throws: true, calls_dart: true },
    },
    Extern {
        decl: "declare i64 @dartforge_ffi_simbolo_nativo(ptr, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_typed_externo(i64, i64, i64, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_typed_externo_t(i64, i64, i64, i64, ptr)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_rti_receita(i64)",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_rti_avaliar(i64, i64, i64, i64)",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare i64 @dartforge_rti_avaliar_cache(ptr, i64, i64, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_rti_tupla_juntar(i64, i64, i64, i64)",
        effects: Effects { allocates: true, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare void @dartforge_rti_definir(i64, i64)",
        // Interna IDs e pode reformatar a lista com caixas/GC, sem lançar
        // exceção Dart nem executar getters/callbacks Dart.
        effects: Effects { allocates: true, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare void @dartforge_rti_registro_nomeado(i64, i64, i64)",
        effects: Effects { allocates: true, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare i8 @dartforge_rti_e(i64, i64)",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        decl: "declare void @dartforge_rti_como_em(i64, i64, i64, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_rti_texto(i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        // O mesmo texto sob ARC, com owner do código do resultado.
        decl: "declare i64 @dartforge_arc_rti_texto_owned_v1(i64) nounwind",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        decl: "declare i64 @dartforge_arc_rti_objeto_tipo_owned_v1(i64) nounwind",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        decl: "declare i64 @dartforge_rti_objeto_tipo(i64)",
        effects: CONSERVATIVE,
    },
    // ─── Espaço unificado (docs/NATIVO-ESPACO-UNIFICADO.md §3.6) ───────────
    Extern {
        // (cid, palavras, flags): a alocação lenta de `@df.allocate` (P0).
        decl: "declare i64 @dartforge_alocar(i64, i64, i64) nounwind",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        // (len, dois): o caminho lento de `@df.texto_alocar` (P1).
        decl: "declare i64 @dartforge_texto_novo(i64, i64) nounwind",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        // `@df.texto_alocar` sob ARC: a string nova com owner do código.
        decl: "declare i64 @dartforge_arc_texto_novo_owned_v1(i64, i64) nounwind",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        // `TearOff` sob ARC: o tear-off canônico com owner do código.
        decl: "declare i64 @dartforge_arc_tearoff_owned_v1(i64) nounwind",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        // A string de uma constante vira o literal canônico (`constants.rs`).
        decl: "declare i64 @dartforge_constante_canonica(i64) nounwind",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        // Calcula e grava o hash no cabeçalho da string (P1).
        decl: "declare i64 @dartforge_texto_hash(i64) nounwind willreturn",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        // As mesmas unidades? O caminho lento de `@df.texto_igual` (P1).
        decl: "declare i8 @dartforge_texto_iguais(i64, i64) memory(read) nounwind willreturn",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        // (palavras, n, cid, forma): a lista literal (P3).
        decl: "declare i64 @dartforge_lista_nova(ptr, i64, i64, i64) nounwind",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        decl: "declare i64 @dartforge_arc_lista_nova_owned_v1(ptr, i64, i64, i64) nounwind",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        // O record literal sob ARC (`AllocRecord`), com owner do código.
        decl: "declare i64 @dartforge_arc_record_novo_owned_v1(ptr, i64) nounwind",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        // (lista, Ref): o `add` que cresce o armazenamento (P3).
        decl: "declare i64 @dartforge_lista_acrescentar(i64, i64) nounwind",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        // (início, fim): a seção dos objetos estáticos de uma imagem (P0).
        decl: "declare void @dartforge_registrar_imagem(ptr, ptr) nounwind",
        effects: Effects { allocates: false, throws: false, calls_dart: false },
    },
    Extern {
        // (Refs, n): o record posicional (P2).
        decl: "declare i64 @dartforge_record_novo(ptr, i64) nounwind",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
];

/// Os dados do runtime que o código gerado referencia (não são funções),
/// `(símbolo, declaresção)`: nenhum. As caixas estáticas de `bool` são lidas do
/// `Context` da thread (`layout::context::VERDADEIRO`/`FALSO`), porque entre o
/// executável e a DLL do SDK cada imagem tem a sua cópia dos dados do runtime
/// (docs/NATIVO-ESPACO-UNIFICADO.md §4.10, item 9).
pub const GLOBALS: &[(&str, &str)] = &[];

/// Efeitos da extern `name`; desconhecida é conservadora.
pub fn effects_of(name: &str) -> Effects {
    // O thunk Owned genérico tem os efeitos da base, mais o retain.
    if let Some(base) = name.strip_prefix(crate::optimize::arc::GENERIC_OWNED_PREFIX) {
        return Effects { allocates: true, ..effects_of(base) };
    }
    // O thunk de escopo tem os efeitos da base (o escopo não aloca).
    if let Some(base) = name.strip_prefix(crate::optimize::arc::NATIVE_SCOPE_PREFIX) {
        return effects_of(base);
    }
    // Os ajudantes `@df.*` de cada pacote do espaço unificado chamados por
    // `CallRuntime` (docs/NATIVO-ESPACO-UNIFICADO.md §3.5).
    for table in [
        super::ir_strings::HELPER_EFFECTS,
        super::ir_boxes::HELPER_EFFECTS,
        super::ir_lists::HELPER_EFFECTS,
        super::ir_typed::HELPER_EFFECTS,
    ] {
        if let Some(&(_, allocates, throws)) = table.iter().find(|(n, _, _)| *n == name) {
            return Effects { allocates, throws, calls_dart: false };
        }
    }
    static TABLE: std::sync::OnceLock<std::collections::HashMap<&'static str, Effects>> = std::sync::OnceLock::new();
    // A fonte é a tabela do runtime (`crates/runtime/effects.tsv`,
    // docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13.8), com uma linha por
    // extern; as marcas de [`EXTERNS`] são a mesma informação ao lado da
    // declaração, e o teste `runtime_table_agrees_with_declarations`
    // as mantém iguais.
    TABLE
        .get_or_init(|| {
            dartforge_runtime::efeitos::EFEITOS
                .iter()
                .map(|&(n, allocates, throws, calls_dart)| (n, Effects { allocates, throws, calls_dart }))
                .collect()
        })
        .get(name)
        .copied()
        .unwrap_or(CONSERVATIVE)
}

/// A compilação confere a tabela de efeitos (`DARTFORGE_EFEITOS=check`):
/// cada chamada a uma extern marcada `collection = 0` ou `throws = 0` sai entre
/// `dartforge_efeitos_antes` e `dartforge_efeitos_depois`, e o runtime
/// encerra o processo, dizendo o nome, se a extern coletar ou deixar uma
/// exceção pendente (`runtime/src/efeitos_conferir.rs`). Só para os testes:
/// é assim que uma marca passa de 1 para 0.
pub fn check_effects() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var("DARTFORGE_EFEITOS").is_ok_and(|v| v == "conferir"))
}

/// As marcas que a conferência de `name` leva ao runtime: bit 0, não coleta;
/// bit 1, não lança. 0 = nada a conferir (ou não é uma extern da tabela).
pub fn marks_to_check(name: &str) -> i64 {
    if !dartforge_runtime::efeitos::EFEITOS.iter().any(|(n, ..)| *n == name)
        || name == "dartforge_efeitos_antes"
        || name == "dartforge_efeitos_depois"
        || name == "dartforge_efeitos_nivel"
        || name == "dartforge_efeitos_restaurar"
    {
        return 0;
    }
    let e = effects_of(name);
    // A folha sabotada (`leaf:<name>`, o D7 do §7.3) é conferida como
    // folha: a coleta dentro dela encerra o processo, dizendo o nome.
    let allocates = e.allocates && !crate::alvo::sabotaged_leaf(name);
    i64::from(!allocates && !e.calls_dart) | (i64::from(!e.throws && !e.calls_dart) << 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unique_well_formed_names() {
        let mut names: Vec<&str> = EXTERNS.iter().map(Extern::nome).collect();
        assert!(
            names.iter().all(|n| n.starts_with("dartforge_")),
            "{names:?}"
        );
        let total = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), total, "extern declarada duas vezes");
    }

    #[test]
    fn runtime_table_agrees_with_declarations() {
        for e in EXTERNS {
            let t = effects_of(e.nome());
            assert_eq!(
                (t.allocates, t.throws),
                (e.effects.allocates, e.effects.throws),
                "{}: a marca ao lado da declaração difere da de crates/runtime/efeitos.tsv",
                e.nome()
            );
            assert!(!e.effects.calls_dart || t.calls_dart, "{}: roda Dart na declaração e não na tabela", e.nome());
        }
    }

    #[test]
    fn conservative_effects_by_default() {
        assert_eq!(effects_of("dartforge_list_push"), CONSERVATIVE);
        assert_eq!(effects_of("nao_existe"), CONSERVATIVE);
    }
}

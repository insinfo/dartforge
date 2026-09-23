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
        decl: "declare void @dartforge_print_i64(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_print_f64(double)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_print_bool(i8)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_print_null()",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_print_string(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_print_list(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_print_map(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_print_set(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_print_handle(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_string_new(ptr, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_string_concat(i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i8 @dartforge_string_equal(i64, i64)",
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
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_object_get(i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_object_set(i64, i64, i64, i8)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_object_class(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_list_new(ptr, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_list_len(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_list_get_bits(i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i8 @dartforge_list_get_tag(i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_list_set(i64, i64, i64, i8)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_list_push(i64, i64, i8)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_map_new(ptr, ptr, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_map_len(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_map_get_bits(i64, i64, i8)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i8 @dartforge_map_get_tag(i64, i64, i8)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_map_set(i64, i64, i8, i64, i8)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i8 @dartforge_map_contains(i64, i64, i8)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_set_new(ptr, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_set_len(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i8 @dartforge_set_contains(i64, i64, i8)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i8 @dartforge_set_add(i64, i64, i8)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_cell_new(i64, i8)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_cell_get_bits(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i8 @dartforge_cell_get_tag(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_cell_set(i64, i64, i8)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_env_new(ptr, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_env_get(i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_closure_new(i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_closure_code(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_closure_env(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_tearoff(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_gc_push_frame(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_gc_set_root(i64, i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_gc_root(i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_gc_pop_frame(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_gc_collect()",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_gc_global_root(i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_box_int(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_box_double(double)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_box_bool(i8)",
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
        decl: "declare i8 @dartforge_identical(i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_list_get_ref(i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_list_first_ref(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_list_last_ref(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_list_single_ref(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_map_get_ref(i64, i64, i8)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_exception_peek_ref()",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_null_assert_fail() noreturn",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_exception_throw(i64, i8)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i8 @dartforge_exception_pending()",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_exception_take_bits()",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i8 @dartforge_exception_take_tag()",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_exception_peek_bits()",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i8 @dartforge_exception_peek_tag()",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_exception_clear()",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_register_subclass(i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i8 @dartforge_is_subclass(i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_stack_trace_get()",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_stack_trace_empty()",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_stack_trace_from_string(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_throw_with_stack_trace(i64, i8, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_list_first(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_list_last(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_value_class(i64)",
        efeitos: CONSERVADOR,
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
        decl: "declare i64 @dartforge_string_len(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_generic_len(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_string_code_unit_at(i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_string_code_units(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_string_runes(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_string_to_upper(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_string_repeat(i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_list_join(i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_list_new_empty()",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_map_get_to_string(i64, i64, i8)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_record_new(ptr, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_int_to_radix_string(i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_int_parse(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_int_try_parse(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare double @dartforge_double_parse(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_string_substring(i64, i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_string_from_char_code(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_string_from_char_codes(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_string_index_of(i64, i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_string_last_index_of(i64, i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_string_split(i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i8 @dartforge_string_contains(i64, i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_string_replace_all(i64, i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_string_pad_left(i64, i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_string_pad_right(i64, i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_string_trim(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_string_trim_left(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_string_trim_right(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i8 @dartforge_string_starts_with(i64, i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i8 @dartforge_string_ends_with(i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_string_to_lower(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_string_compare_to(i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_string_replace_first(i64, i64, i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_string_replace_range(i64, i64, i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_list_reversed(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_string_buffer_new()",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_string_buffer_write(i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_regexp_new(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_string_split_map_pieces(i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_collection_mark_unmodifiable(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i8 @dartforge_collection_is_unmodifiable(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_exception_new(i64, i8)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_format_exception_new(i64, i64, i64)",
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
        decl: "declare i64 @dartforge_range_error_new(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_range_error_value(i64, i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_range_error_range(i64, i64, i64, i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_range_error_index(i64, i64, i64, i64)",
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
        decl: "declare i64 @dartforge_concurrent_modification_error_new(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_type_error_new()",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_no_such_method_error_new(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_error_get_message(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_error_get_name(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_error_get_invalid_value(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_error_get_start(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_error_get_end(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_error_get_source(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_error_get_offset(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_error_get_stack_trace(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_list_single(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_list_sublist(i64, i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_list_remove_at(i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_list_filled(i64, i64, i8)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_map_remove(i64, i64, i8)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_map_keys(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_iteration_begin(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_iteration_end(i64)",
        efeitos: CONSERVADOR,
    },
    // --- P1 (closures, α): convenção uniforme e leituras com representação ---
    Extern {
        decl: "declare i64 @dartforge_cell_get_ref(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_env_get_ref(i64, i64)",
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
    Extern {
        decl: "declare i64 @dartforge_dyn_unario(i64, i64)",
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
    // --- P3 (α): records ---
    Extern {
        decl: "declare i64 @dartforge_record_len(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_record_get_ref(i64, i64)",
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
        decl: "declare void @dartforge_nativo_Error_trySetStackTrace(i64, i64)",
        efeitos: CONSERVADOR,
    },
    // --- RTI: tipos em tempo de execução (`tipos.rs`) ---
    Extern {
        decl: "declare void @dartforge_rti_classe_nome(i64, i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_rti_regra(i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_rti_classe_do_runtime(i64, i64)",
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
        decl: "declare void @dartforge_rti_definir(i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_rti_do_valor(i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i8 @dartforge_rti_e(i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i8 @dartforge_rti_subtipo(i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare void @dartforge_rti_como(i64, i64)",
        efeitos: CONSERVADOR,
    },
    Extern {
        decl: "declare i64 @dartforge_rti_texto(i64)",
        efeitos: CONSERVADOR,
    },
];

/// Efeitos da extern `nome`; desconhecida é conservadora.
pub fn efeitos_de(nome: &str) -> Efeitos {
    EXTERNS
        .iter()
        .find(|e| e.nome() == nome)
        .map_or(CONSERVADOR, |e| e.efeitos)
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
    fn efeitos_conservadores_por_padrao() {
        assert_eq!(efeitos_de("dartforge_list_push"), CONSERVADOR);
        assert_eq!(efeitos_de("nao_existe"), CONSERVADOR);
    }
}

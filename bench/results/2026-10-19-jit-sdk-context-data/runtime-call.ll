; DartForge contract provenance 5cdf547e6e73b46258e1970be102262594fb7ad24ee807a6cfce474afc2e6ee3
; df.universal-parent 0
; df.cone-class 0
; df.cone-class 1073741569
; df.cone-parent 1073741569 621
; Declarations of the native Rust runtime (table in llvm/externs.rs)
declare i64 @dartforge_arc_closure_owned_v1(i64, i64, i64, i64)
declare i64 @dartforge_string_new(ptr, i64)
declare i64 @dartforge_string_concat(i64, i64)
declare i8 @dartforge_equal(i64, i64)
declare void @dartforge_register_class_name(i64, ptr, i64)
declare i64 @dartforge_object_new(i64, i64)
declare i64 @dartforge_object_get(i64, i64)
declare i64 @dartforge_stack_trace_string(i64)
declare void @dartforge_register_stack_trace(ptr, ptr)
declare void @dartforge_register_stack_trace_field(i64, ptr, i64, i64)
declare void @dartforge_register_stack_trace_tearoff(ptr, i64)
declare i64 @dartforge_stack_trace_enter(i64, i64)
declare void @dartforge_stack_trace_exit(i64)
declare void @dartforge_remember(i64) nounwind
declare void @dartforge_memory_arc_v1()
declare void @dartforge_arc_write_ref(i64, i64, i64) nounwind
declare void @dartforge_arc_initial(i64) nounwind
declare void @dartforge_arc_global_receive_v1(i64, i64) nounwind
declare i64 @dartforge_arc_box_double_owned_v1(double) nounwind
declare i64 @dartforge_arc_read_field_scalar_v1(i64, i64) nounwind
declare i64 @dartforge_arc_register_return_owned_v1(i64, i64) nounwind
declare i64 @dartforge_arc_register_uniform_owned_v1(i64) nounwind
declare i64 @dartforge_arc_register_typed_v1(i64, i64) nounwind
declare void @dartforge_arc_take_typed_v1(i64, i64, i64) nounwind
declare i64 @dartforge_arc_typed_target_v1(i64, i64) nounwind
declare i64 @dartforge_arc_invoke_uniform_v1(i64, i64, i64, i64) nounwind
declare i64 @dartforge_arc_invoke_uniform_closure_v1(i64, i64, i64) nounwind
declare void @dartforge_arc_take_return_owned_v1(i64, i64) nounwind
declare i64 @dartforge_arc_native_scope_open_v1() nounwind
declare void @dartforge_arc_native_scope_close_v1(i64) nounwind
declare i64 @dartforge_arc_env_ref_v1(i64, i64) nounwind
declare i64 @dartforge_arc_scalar_env_v1(i64, i64) nounwind
declare i64 @dartforge_arc_closure_context_v1(i64) nounwind
declare i64 @dartforge_arc_socket_recvfrom_owned_v1(i64)
declare i64 @dartforge_arc_read_field_ref_v1(i64, i64) nounwind
declare i64 @dartforge_arc_read_field_late_ref_v1(i64, i64) nounwind
declare i64 @dartforge_arc_list_storage_ref_v1(i64) nounwind
declare i64 @dartforge_arc_list_shape_v1(i64) nounwind
declare i64 @dartforge_arc_list_capacity_v1(i64) nounwind
declare void @dartforge_arc_list_write_len_v1(i64, i64) nounwind
declare void @dartforge_arc_list_write_bits_v1(i64, i64, i64, i64) nounwind
declare void @dartforge_arc_list_write_ref_v1(i64, i64, i64) nounwind
declare i64 @dartforge_arc_list_write_data_v1(i64, i64) nounwind
declare i64 @dartforge_arc_list_len_v1(i64) nounwind
declare i64 @dartforge_arc_list_read_bits_v1(i64, i64) nounwind
declare i64 @dartforge_arc_list_ref_owned_v1(i64, i64) nounwind
declare i64 @dartforge_arc_list_ref_v1(i64, i64) nounwind
declare i64 @dartforge_arc_argument_owned_v1(ptr, ptr, i64) nounwind
declare i64 @dartforge_arc_box_int_owned_v1(i64) nounwind
declare i64 @dartforge_arc_box_simd_owned_v1(i64, i64, i64) nounwind
declare i64 @dartforge_arc_object_owned_v1(i64, i64) nounwind
declare i64 @dartforge_arc_object_owned_t_v1(i64, i64, ptr) nounwind
declare void @dartforge_arc_write_field_scalar_v1(i64, i64, i64) nounwind
declare void @dartforge_arc_write_field_scalar_checked_v1(i64, i64, i64, ptr, i64, i64) nounwind
declare i64 @dartforge_arc_read_field_scalar_checked_v1(i64, i64, ptr, i64, i64) nounwind
declare void @dartforge_arc_write_field_ref_checked_v1(i64, i64, i64, ptr, i64) nounwind
declare void @dartforge_arc_write_field_ref_v1(i64, i64, i64) nounwind
declare i64 @dartforge_arc_exception_owned_v1() nounwind
declare i64 @dartforge_arc_stack_trace_owned_v1() nounwind
declare void @dartforge_arc_throw_with_stack_trace_ref_v1(i64, i64)
declare void @dartforge_arc_throw_ref_v1(i64)
declare i8 @dartforge_arc_observe_heap_v1(i64) nounwind
declare i64 @dartforge_arc_frame_open_v1(i64) nounwind
declare i64 @dartforge_arc_frame_load_v1(i64, i64) nounwind
declare void @dartforge_arc_frame_receive_v1(i64, i64, i64) nounwind
declare void @dartforge_arc_frame_copy_v1(i64, i64, i64) nounwind
declare void @dartforge_arc_frame_move_v1(i64, i64, i64, i64) nounwind
declare void @dartforge_arc_frame_close_v1(i64) nounwind
declare void @dartforge_arc_retain(i64) nounwind
declare void @dartforge_arc_release(i64) nounwind
declare void @dartforge_arc_collect() nounwind
declare i8 @dartforge_arc_verify_abi(i64) nounwind
declare void @dartforge_object_set(i64, i64, i64, i8)
declare i8 @dartforge_late_field_initialized(i64, i64)
declare void @dartforge_late_field_mark_initialized(i64, i64)
declare i64 @dartforge_tearoff(i64)
declare void @dartforge_gc_global_root(i64, i64)
declare void @dartforge_mark_constant(i64, i64)
declare i8 @dartforge_exception_catchable()
declare void @dartforge_register_isolates(ptr, ptr)
declare void @dartforge_ffi_register_type(i64, i64)
declare void @dartforge_ffi_register_trampoline(ptr, i64, ptr)
declare i64 @dartforge_ffi_closure_address(i64)
declare i64 @dartforge_ffi_pointer_address(i64)
declare i64 @dartforge_ffi_return_pointer(i64, i64)
declare i64 @dartforge_arc_ffi_return_pointer_owned_v1(i64, i64)
declare i64 @dartforge_typed_new(i64, i64, i64)
declare i64 @dartforge_typed_new_t(i64, i64, i64, ptr)
declare ptr @dartforge_context() memory(none) nounwind willreturn speculatable
declare i64 @dartforge_typed_len(i64, i64, i64) memory(none) nounwind willreturn speculatable
declare i64 @dartforge_typed_fill_int(i64, i64, i64, i64, i64) nounwind
declare i64 @dartforge_typed_fill_double(i64, i64, i64, i64, double) nounwind
declare i64 @dartforge_string_join_typed(ptr, i64) nounwind
declare i64 @dartforge_arc_string_join_owned_v1(ptr, i64) nounwind
declare i64 @dartforge_closure_typed(i64, i64) memory(inaccessiblemem: read) nounwind willreturn
declare i64 @dartforge_object_fields(i64) memory(inaccessiblemem: read) nounwind willreturn speculatable
declare i64 @dartforge_view_new(i64, i64, i64, i64, i64)
declare i64 @dartforge_view_new_t(i64, i64, i64, i64, i64, ptr)
declare i64 @dartforge_unbox_int(i64)
declare double @dartforge_unbox_double(i64)
declare i8 @dartforge_unbox_bool(i64)
declare i64 @dartforge_exception_peek_ref()
declare void @dartforge_exception_throw(i64, i8)
declare i8 @dartforge_pending_interrupt()
declare void @dartforge_safepoint()
declare i8 @dartforge_exception_pending()
declare void @dartforge_exception_clear()
declare void @dartforge_register_subclass(i64, i64)
declare i8 @dartforge_is_subclass(i64, i64) memory(read) nounwind willreturn
declare i64 @dartforge_stack_trace_get()
declare void @dartforge_throw_with_stack_trace(i64, i8, i64)
declare i64 @dartforge_value_class(i64)
declare i64 @dartforge_to_string_i64(i64)
declare i64 @dartforge_to_string_f64(double)
declare i64 @dartforge_to_string_bool(i8)
declare i64 @dartforge_to_string_handle(i64)
declare i64 @dartforge_record_new(ptr, i64)
declare i64 @dartforge_collection_mark_unmodifiable(i64)
declare i64 @dartforge_exception_new(i64, i8)
declare i64 @dartforge_state_error_new(i64)
declare i64 @dartforge_argument_error_new(i64, i64)
declare i64 @dartforge_argument_error_value(i64, i8, i64, i64)
declare i64 @dartforge_argument_error_not_null(i64)
declare i64 @dartforge_range_error_range(i64, i64, i64, i64, i64)
declare i64 @dartforge_unsupported_error_new(i64)
declare i64 @dartforge_unimplemented_error_new(i64)
declare i64 @dartforge_assertion_error_new(i64, i8)
declare i64 @dartforge_arc_exception_new_owned_v1(i64)
declare i64 @dartforge_arc_assertion_error_new_owned_v1(i64)
declare i64 @dartforge_arc_state_error_new_owned_v1(i64)
declare i64 @dartforge_arc_unsupported_error_new_owned_v1(i64)
declare i64 @dartforge_arc_unimplemented_error_new_owned_v1(i64)
declare i64 @dartforge_arc_argument_error_new_owned_v1(i64, i64)
declare i64 @dartforge_arc_argument_error_value_owned_v1(i64, i64, i64)
declare i64 @dartforge_arc_argument_error_not_null_owned_v1(i64)
declare i64 @dartforge_arc_range_error_range_owned_v1(i64, i64, i64, i64, i64)
declare i64 @dartforge_arc_type_error_new_owned_v1()
declare i64 @dartforge_arc_null_check_error_new_owned_v1()
declare i64 @dartforge_arc_late_error_new_owned_v1(i64, i64)
declare i64 @dartforge_arc_stack_overflow_error_new_owned_v1()
declare i64 @dartforge_arc_no_such_method_error_new_owned_v1(i64)
declare i64 @dartforge_type_error_new()
declare i64 @dartforge_null_check_error_new()
declare i64 @dartforge_late_error_new(i64, i64)
declare i64 @dartforge_stack_overflow_error_new()
declare void @dartforge_stack_overflow()
declare i8 @dartforge_late_field_initializing(i64, i64)
declare void @dartforge_late_field_set_initializing(i64, i64, i8)
declare i64 @dartforge_no_such_method_error_new(i64)
declare i64 @dartforge_closure_entry(i64)
declare i8 @dartforge_args_match(ptr, ptr)
declare i64 @dartforge_arg_index(ptr, i64)
declare void @dartforge_nsm_call()
declare i64 @dartforge_dyn_op(i64, i64, i64)
declare i64 @dartforge_iteravel_get_ref(i64, i64)
declare i64 @dartforge_iteravel_get_bits(i64, i64)
declare ptr @dartforge_selector(ptr, i64, i64, ptr, i64)
declare void @dartforge_register_methods(i64, ptr, i64)
declare void @dartforge_member_rejected(i64)
declare void @dartforge_register_cids(ptr, i64)
declare void @dartforge_register_helper(ptr, i64, ptr)
declare void @dartforge_register_argument_name(ptr, i64)
declare i64 @dartforge_forward_nsm(i64, i64, i64, i64, i64, i64, i64)
declare void @dartforge_register_sdk_version(ptr, i64)
declare void @dartforge_prepare_embedder()
declare i64 @dartforge_main_arguments()
declare ptr @dartforge_globals_area(ptr)
declare ptr @dartforge_globals_area_id(ptr, ptr)
declare i32 @dartforge_init(ptr, ptr)
declare void @dartforge_call_trace_entry(ptr, i64)
declare void @dartforge_call_trace_exit()
declare i64 @dartforge_object_new_t(i64, i64, ptr)
declare i64 @dartforge_object_new_m(i64, i64, i64)
declare i64 @dartforge_object_new_mt(i64, i64, i64, ptr)
declare void @dartforge_register_table(i64, ptr)
declare void @dartforge_event_loop(ptr)
declare void @dartforge_native_DartForge_scheduleImmediate(i64)
declare i64 @dartforge_native_DartForge_Timer_new(i64, i64, i8)
declare void @dartforge_native_DartForge_Timer_cancel(i64)
declare i64 @dartforge_native_DartForge_Timer_tick(i64)
declare i8 @dartforge_native_DartForge_native_pump_supported()
declare i64 @dartforge_native_DartForge_native_pump_enable()
declare i64 @dartforge_native_DartForge_native_pump_disable()
declare i8 @dartforge_native_DartForge_native_pump_dispatcher_supported()
declare i64 @dartforge_native_DartForge_native_pump_set_dispatcher(i64)
declare i64 @dartforge_native_DartForge_native_pump_clear_dispatcher()
declare i64 @dartforge_native_Error_throwWithStackTrace(i64, i64)
declare void @dartforge_native_Error_trySetStackTrace(i64, i64)
declare void @dartforge_rti_init_table(ptr, i64)
declare i64 @dartforge_list_from_table(ptr, i64)
declare i64 @dartforge_list_from_table_g(ptr, i64, ptr)
declare void @dartforge_ffi_register_compound(i64, i64, i64, i64, i64, i64, i64)
declare i64 @dartforge_ffi_compound(i64, i64, i64)
declare i64 @dartforge_ffi_pointer_new(i64)
declare i64 @dartforge_ffi_compound_address(i64)
declare i64 @dartforge_ffi_compound_new(i64)
declare i64 @dartforge_ffi_handles_open()
declare void @dartforge_ffi_handles_close(i64)
declare i64 @dartforge_ffi_handle_new(i64)
declare i64 @dartforge_ffi_handle_object(i64)
declare i64 @dartforge_ffi_handles_close_with_object(i64, i64)
declare i64 @dartforge_arc_ffi_handles_close_with_object_owned_v1(i64, i64)
declare i64 @dartforge_arc_ffi_handle_object_owned_v1(i64)
declare i64 @dartforge_ffi_compound_copy(i64, i64)
declare void @dartforge_ffi_copy_compound(ptr, i64, i64)
declare void @dartforge_ffi_register_callback(ptr, i64, ptr)
declare i64 @dartforge_ffi_callback_enter(ptr)
declare i64 @dartforge_ffi_callback_late_return(ptr) nounwind
declare i64 @dartforge_ffi_callback_late_calls() nounwind
declare i8 @dartforge_ffi_callback_exit(ptr, ptr)
declare void @dartforge_ffi_callback_post(ptr, ptr, i64)
declare i64 @dartforge_ffi_callback_sync(ptr, ptr, i64)
declare i64 @dartforge_ffi_callback_closure(i64)
declare i64 @dartforge_ffi_callback_pointer(i64, i64, i64)
declare i64 @dartforge_arc_ffi_callback_closure_owned_v1(i64) nounwind
declare i64 @dartforge_arc_ffi_callback_pointer_owned_v1(i64, i64, i64)
declare i64 @dartforge_ffi_native_symbol(ptr, i64)
declare i64 @dartforge_typed_external(i64, i64, i64, i64)
declare i64 @dartforge_typed_external_t(i64, i64, i64, i64, ptr)
declare i64 @dartforge_rti_recipe(i64)
declare i64 @dartforge_rti_eval(i64, i64, i64, i64)
declare i64 @dartforge_rti_eval_cache(ptr, i64, i64, i64)
declare i64 @dartforge_rti_tuple_join(i64, i64, i64, i64)
declare void @dartforge_rti_set(i64, i64)
declare void @dartforge_rti_named_record(i64, i64, i64)
declare i8 @dartforge_rti_e(i64, i64)
declare i8 @dartforge_rti_e_cache(ptr, i64, i64)
declare void @dartforge_rti_as_with_context(i64, i64, i64, i64)
declare i64 @dartforge_rti_string(i64)
declare i64 @dartforge_arc_rti_string_owned_v1(i64) nounwind
declare i64 @dartforge_arc_rti_object_type_owned_v1(i64) nounwind
declare i64 @dartforge_rti_object_type(i64)
declare i64 @dartforge_allocate(i64, i64, i64) nounwind
declare i64 @dartforge_string_allocate(i64, i64) nounwind
declare i64 @dartforge_arc_string_allocate_owned_v1(i64, i64) nounwind
declare i64 @dartforge_arc_tearoff_owned_v1(i64) nounwind
declare i64 @dartforge_canonical_constant(i64) nounwind
declare i64 @dartforge_string_hash(i64) nounwind willreturn
declare i8 @dartforge_string_equals(i64, i64) memory(read) nounwind willreturn
declare i64 @dartforge_list_new(ptr, i64, i64, i64) nounwind
declare i64 @dartforge_arc_list_new_owned_v1(ptr, i64, i64, i64) nounwind
declare i64 @dartforge_arc_record_from_refs_owned_v1(ptr, i64) nounwind
declare i64 @dartforge_list_append(i64, i64) nounwind
declare void @dartforge_register_image(ptr, ptr) nounwind
declare i64 @dartforge_record_from_refs(ptr, i64) nounwind
declare <4 x float> @llvm.fabs.v4f32(<4 x float>)
declare <2 x double> @llvm.fabs.v2f64(<2 x double>)
declare <4 x float> @llvm.sqrt.v4f32(<4 x float>)
declare <2 x double> @llvm.sqrt.v2f64(<2 x double>)
declare <4 x i32> @llvm.abs.v4i32(<4 x i32>, i1)
declare i8 @llvm.expect.i8(i8, i8)
declare i1 @llvm.expect.i1(i1, i1)
declare i64 @llvm.read_register.i64(metadata)
define internal i64 @df.string_len(i64 %s) alwaysinline nounwind {
  %p = inttoptr i64 %s to ptr
  %q = getelementptr inbounds i8, ptr %p, i64 14
  %n = load i64, ptr %q, align 8
  ret i64 %n
}
define internal i64 @df.string_code_unit(i64 %s, i64 %i) alwaysinline nounwind {
entrada:
  %p = inttoptr i64 %s to ptr
  %pc = getelementptr inbounds i8, ptr %p, i64 2
  %c = load i32, ptr %pc, align 4, !invariant.load !{}
  %u = getelementptr inbounds i8, ptr %p, i64 22
  %um = icmp eq i32 %c, 6
  br i1 %um, label %um1, label %dois
um1:
  %a1 = getelementptr inbounds i8, ptr %u, i64 %i
  %b1 = load i8, ptr %a1, align 1
  %r1 = zext i8 %b1 to i64
  ret i64 %r1
dois:
  %a2 = getelementptr inbounds i16, ptr %u, i64 %i
  %b2 = load i16, ptr %a2, align 2
  %r2 = zext i16 %b2 to i64
  ret i64 %r2
}
define internal i64 @df.string_allocate(i64 %len, i1 %dois) alwaysinline nounwind {
entrada:
  %d = zext i1 %dois to i64
  %bytes = shl i64 %len, %d
  %b7 = add i64 %bytes, 7
  %pw = lshr i64 %b7, 3
  %w = add i64 %pw, 1
  %grande = icmp ugt i64 %w, 16
  br i1 %grande, label %lento, label %rapido
rapido:
  %c = add i64 %d, 6
  %cs = shl i64 %c, 32
  %ws = shl i64 %w, 16
  %c0 = or i64 %cs, %ws
  %cab = or i64 %c0, 513
  %h = call i64 @df.allocate(i64 %cab, i64 %w)
  %p = inttoptr i64 %h to ptr
  %q = getelementptr inbounds i8, ptr %p, i64 14
  store i64 %len, ptr %q, align 8
  ret i64 %h
lento:
  %hl = call i64 @dartforge_string_allocate(i64 %len, i64 %d)
  ret i64 %hl
}
define internal void @df.string_write(i64 %s, i64 %i, i64 %x) alwaysinline nounwind {
entrada:
  %p = inttoptr i64 %s to ptr
  %pc = getelementptr inbounds i8, ptr %p, i64 2
  %c = load i32, ptr %pc, align 4, !invariant.load !{}
  %u = getelementptr inbounds i8, ptr %p, i64 22
  %um = icmp eq i32 %c, 6
  br i1 %um, label %um1, label %dois
um1:
  %a1 = getelementptr inbounds i8, ptr %u, i64 %i
  %t1 = trunc i64 %x to i8
  store i8 %t1, ptr %a1, align 1
  ret void
dois:
  %a2 = getelementptr inbounds i16, ptr %u, i64 %i
  %t2 = trunc i64 %x to i16
  store i16 %t2, ptr %a2, align 2
  ret void
}
define internal i64 @df.string_hash(i64 %s) alwaysinline nounwind {
entrada:
  %p = inttoptr i64 %s to ptr
  %q = getelementptr inbounds i8, ptr %p, i64 6
  %x = load i32, ptr %q, align 4
  %z = icmp eq i32 %x, 0
  %ze = call i1 @llvm.expect.i1(i1 %z, i1 false)
  br i1 %ze, label %lento, label %rapido
rapido:
  %r = zext i32 %x to i64
  ret i64 %r
lento:
  %l = call i64 @dartforge_string_hash(i64 %s)
  ret i64 %l
}
define internal i1 @df.string_equals(i64 %a, i64 %b) alwaysinline nounwind {
entrada:
  %id = icmp eq i64 %a, %b
  br i1 %id, label %sim, label %compr
compr:
  %pa = inttoptr i64 %a to ptr
  %pb = inttoptr i64 %b to ptr
  %qa = getelementptr inbounds i8, ptr %pa, i64 14
  %qb = getelementptr inbounds i8, ptr %pb, i64 14
  %la = load i64, ptr %qa, align 8
  %lb = load i64, ptr %qb, align 8
  %ml = icmp eq i64 %la, %lb
  br i1 %ml, label %hashes, label %nao
hashes:
  %ra = getelementptr inbounds i8, ptr %pa, i64 6
  %rb = getelementptr inbounds i8, ptr %pb, i64 6
  %ha = load i32, ptr %ra, align 4
  %hb = load i32, ptr %rb, align 4
  %az = icmp eq i32 %ha, 0
  %bz = icmp eq i32 %hb, 0
  %mh = icmp eq i32 %ha, %hb
  %algum = or i1 %az, %bz
  %talvez = or i1 %algum, %mh
  br i1 %talvez, label %lento, label %nao
lento:
  %r = call i8 @dartforge_string_equals(i64 %a, i64 %b)
  %rb1 = icmp ne i8 %r, 0
  ret i1 %rb1
sim:
  ret i1 true
nao:
  ret i1 false
}
define internal i1 @df.string_equals_object(i64 %a, i64 %b) alwaysinline nounwind {
entrada:
  %id = icmp eq i64 %a, %b
  br i1 %id, label %sim, label %objeto
objeto:
  %t = and i64 %b, -9223372036854775801
  %e = icmp eq i64 %t, 2
  br i1 %e, label %classe, label %nao
classe:
  %pb = inttoptr i64 %b to ptr
  %pc = getelementptr inbounds i8, ptr %pb, i64 2
  %c = load i32, ptr %pc, align 4, !invariant.load !{}
  %c6 = sub i32 %c, 6
  %texto = icmp ult i32 %c6, 2
  br i1 %texto, label %comparar, label %nao
comparar:
  %r = call i1 @df.string_equals(i64 %a, i64 %b)
  ret i1 %r
sim:
  ret i1 true
nao:
  ret i1 false
}
define internal i64 @df.box_int(i64 %v) alwaysinline nounwind {
%a = add i64 %v, 4611686018427387904
%ok = icmp ult i64 %a, -9223372036854775808
br i1 %ok, label %smi, label %heap
smi:
%s = shl i64 %v, 1
%r = or i64 %s, 1
ret i64 %r
heap:
%h = call i64 @df.allocate(i64 12884967937, i64 1)
%p = inttoptr i64 %h to ptr
%q = getelementptr inbounds i8, ptr %p, i64 14
store i64 %v, ptr %q, align 8
ret i64 %h
}
define internal i64 @df.unbox_int(i64 %r) alwaysinline nounwind {
%b = and i64 %r, 1
%e = icmp ne i64 %b, 0
br i1 %e, label %smi, label %obj
smi:
%v = ashr i64 %r, 1
ret i64 %v
obj:
%m = and i64 %r, -9223372036854775801
%o = icmp eq i64 %m, 2
br i1 %o, label %cls, label %lento
cls:
%p = inttoptr i64 %r to ptr
%cp = getelementptr inbounds i8, ptr %p, i64 2
%c = load i32, ptr %cp, align 4, !invariant.load !{}
%mint = icmp eq i32 %c, 3
br i1 %mint, label %ler, label %lento
ler:
%vp = getelementptr inbounds i8, ptr %p, i64 14
%x = load i64, ptr %vp, align 8
ret i64 %x
lento:
%h = call i64 @dartforge_unbox_int(i64 %r)
ret i64 %h
}
define internal i64 @df.box_double(double %d) alwaysinline nounwind {
%h = call i64 @df.allocate(i64 17179935233, i64 1)
%p = inttoptr i64 %h to ptr
%q = getelementptr inbounds i8, ptr %p, i64 14
store double %d, ptr %q, align 8
ret i64 %h
}
define internal double @df.unbox_double(i64 %r) alwaysinline nounwind {
%m = and i64 %r, -9223372036854775801
%o = icmp eq i64 %m, 2
br i1 %o, label %cls, label %lento
cls:
%p = inttoptr i64 %r to ptr
%cp = getelementptr inbounds i8, ptr %p, i64 2
%c = load i32, ptr %cp, align 4, !invariant.load !{}
%dbl = icmp eq i32 %c, 4
br i1 %dbl, label %ler, label %lento
ler:
%vp = getelementptr inbounds i8, ptr %p, i64 14
%x = load double, ptr %vp, align 8
ret double %x
lento:
%h = call double @dartforge_unbox_double(i64 %r)
ret double %h
}
define internal double @df.num_to_double(i64 %r) alwaysinline nounwind {
%b = and i64 %r, 1
%e = icmp ne i64 %b, 0
br i1 %e, label %smi, label %obj
smi:
%v = ashr i64 %r, 1
%dv = sitofp i64 %v to double
ret double %dv
obj:
%m = and i64 %r, -9223372036854775801
%o = icmp eq i64 %m, 2
br i1 %o, label %cls, label %dbl
cls:
%p = inttoptr i64 %r to ptr
%cp = getelementptr inbounds i8, ptr %p, i64 2
%c = load i32, ptr %cp, align 4, !invariant.load !{}
%mint = icmp eq i32 %c, 3
br i1 %mint, label %ler, label %dbl
ler:
%vp = getelementptr inbounds i8, ptr %p, i64 14
%x = load i64, ptr %vp, align 8
%dx = sitofp i64 %x to double
ret double %dx
dbl:
%d = call double @df.unbox_double(i64 %r)
ret double %d
}
define internal i64 @df.box_bool(i1 %b) alwaysinline nounwind {
%ctx = call ptr @df.context()
%o = select i1 %b, i64 360, i64 368
%p = getelementptr inbounds i8, ptr %ctx, i64 %o
%r = load i64, ptr %p, align 8, !invariant.load !{}
ret i64 %r
}
define internal i1 @df.unbox_bool(i64 %r) alwaysinline nounwind {
%ctx = call ptr @df.context()
%vp = getelementptr inbounds i8, ptr %ctx, i64 360
%vh = load i64, ptr %vp, align 8, !invariant.load !{}
%v = icmp eq i64 %r, %vh
br i1 %v, label %sim, label %t
sim:
ret i1 true
t:
%fp = getelementptr inbounds i8, ptr %ctx, i64 368
%fh = load i64, ptr %fp, align 8, !invariant.load !{}
%f = icmp eq i64 %r, %fh
br i1 %f, label %nao, label %lento
nao:
ret i1 false
lento:
%u = call i8 @dartforge_unbox_bool(i64 %r)
%x = icmp ne i8 %u, 0
ret i1 %x
}
define internal i64 @df.bit_smi(i64 %x) alwaysinline nounwind {
%b = and i64 %x, 1
ret i64 %b
}
define internal i64 @df.smi_value(i64 %x) alwaysinline nounwind {
%v = ashr i64 %x, 1
ret i64 %v
}
define internal i1 @df.identical(i64 %a, i64 %b) alwaysinline nounwind {
%eq = icmp eq i64 %a, %b
br i1 %eq, label %sim, label %t1
t1:
%ma = and i64 %a, -9223372036854775801
%oa = icmp eq i64 %ma, 2
%mb = and i64 %b, -9223372036854775801
%ob = icmp eq i64 %mb, 2
%ambos = and i1 %oa, %ob
br i1 %ambos, label %t2, label %nao
t2:
%pa = inttoptr i64 %a to ptr
%pb = inttoptr i64 %b to ptr
%cpa = getelementptr inbounds i8, ptr %pa, i64 2
%cpb = getelementptr inbounds i8, ptr %pb, i64 2
%ca = load i32, ptr %cpa, align 4, !invariant.load !{}
%cb = load i32, ptr %cpb, align 4, !invariant.load !{}
%mesma = icmp eq i32 %ca, %cb
%c3 = sub i32 %ca, 3
%num = icmp ult i32 %c3, 2
%cmp = and i1 %mesma, %num
br i1 %cmp, label %t3, label %nao
t3:
%va = getelementptr inbounds i8, ptr %pa, i64 14
%vb = getelementptr inbounds i8, ptr %pb, i64 14
%xa = load i64, ptr %va, align 8
%xb = load i64, ptr %vb, align 8
%r = icmp eq i64 %xa, %xb
ret i1 %r
sim:
ret i1 true
nao:
ret i1 false
}
define internal i64 @df.list_len(i64 %l) alwaysinline nounwind {
%p = inttoptr i64 %l to ptr
%q = getelementptr inbounds i8, ptr %p, i64 14
%n = load i64, ptr %q, align 8
ret i64 %n
}
define internal i64 @df.list_storage(i64 %l) alwaysinline nounwind {
entrada:
%p = inttoptr i64 %l to ptr
%cp = getelementptr inbounds i8, ptr %p, i64 2
%c = load i32, ptr %cp, align 4, !invariant.load !{}
%e = icmp eq i32 %c, 10
br i1 %e, label %exp, label %fim
exp:
%dp = getelementptr inbounds i8, ptr %p, i64 22
%d = load i64, ptr %dp, align 8
br label %fim
fim:
%r = phi i64 [ %d, %exp ], [ %l, %entrada ]
ret i64 %r
}
define internal ptr @df.list_element(i64 %a, i64 %i) alwaysinline nounwind {
%p = inttoptr i64 %a to ptr
%b = getelementptr inbounds i8, ptr %p, i64 22
%e = getelementptr inbounds i64, ptr %b, i64 %i
ret ptr %e
}
define internal i8 @df.list_shape(i64 %a) alwaysinline nounwind {
%p = inttoptr i64 %a to ptr
%fp = getelementptr inbounds i8, ptr %p, i64 -1
%f = load i8, ptr %fp, align 1
%r = and i8 %f, 102
ret i8 %r
}
define internal i64 @df.list_capacity(i64 %l) alwaysinline nounwind {
%a = call i64 @df.list_storage(i64 %l)
%n = call i64 @df.list_len(i64 %a)
ret i64 %n
}
define internal i64 @df.list_read(i64 %a, i64 %i) alwaysinline nounwind {
%e = call ptr @df.list_element(i64 %a, i64 %i)
%v = load i64, ptr %e, align 8
ret i64 %v
}
define internal void @df.list_write_ref(i64 %a, i64 %i, i64 %v) alwaysinline nounwind {
%e = call ptr @df.list_element(i64 %a, i64 %i)
store i64 %v, ptr %e, align 8
call void @df.element_barrier(i64 %a, i64 %i, i64 %v)
ret void
}
define internal void @df.list_write_len(i64 %l, i64 %n) alwaysinline nounwind {
%p = inttoptr i64 %l to ptr
%q = getelementptr inbounds i8, ptr %p, i64 14
store i64 %n, ptr %q, align 8
ret void
}
define internal i64 @df.list_write_data(i64 %l, i64 %d) alwaysinline nounwind {
entrada:
%a = call i64 @df.list_storage(i64 %l)
%fa = call i8 @df.list_shape(i64 %a)
%fd = call i8 @df.list_shape(i64 %d)
%igual = icmp eq i8 %fa, %fd
br i1 %igual, label %grava, label %fim
grava:
%p = inttoptr i64 %l to ptr
%q = getelementptr inbounds i8, ptr %p, i64 22
store i64 %d, ptr %q, align 8
call void @df.barrier(i64 %l, i64 %d)
br label %fim
fim:
%r = phi i64 [ 1, %grava ], [ 0, %entrada ]
ret i64 %r
}
define internal i64 @df.typed_len(i64 %t) alwaysinline nounwind {
  %a = add i64 %t, 14
  %p = inttoptr i64 %a to ptr
  %n = load i64, ptr %p, align 8, !invariant.load !{}
  ret i64 %n
}
define internal i64 @df.typed_data(i64 %t) alwaysinline nounwind {
  %a = add i64 %t, 22
  %p = inttoptr i64 %a to ptr
  %d = load i64, ptr %p, align 8, !invariant.load !{}
  ret i64 %d
}
define internal i64 @df.typed_cid(i64 %t) alwaysinline nounwind {
  %a = add i64 %t, 2
  %p = inttoptr i64 %a to ptr
  %c = load i32, ptr %p, align 4, !invariant.load !{}
  %r = sext i32 %c to i64
  ret i64 %r
}
define internal i64 @df.typed_len_writable(i64 %t, i64 %imutavel) alwaysinline nounwind {
  %c = call i64 @df.typed_cid(i64 %t)
  %n = call i64 @df.typed_len(i64 %t)
  %i = icmp eq i64 %c, %imutavel
  %r = select i1 %i, i64 0, i64 %n
  ret i64 %r
}
define internal i64 @df.typed_bytes(i64 %t) alwaysinline nounwind {
  %c = call i64 @df.typed_cid(i64 %t)
  %n = call i64 @df.typed_len(i64 %t)
  %d = sub i64 %c, 22
  %interna = icmp ult i64 %d, 14
  %v = sub i64 %c, 36
  %vt = urem i64 %v, 14
  %bd = icmp sge i64 %c, 64
  %tv = select i1 %bd, i64 14, i64 %vt
  %tipo = select i1 %interna, i64 %d, i64 %tv
  %s = mul i64 %tipo, 4
  %tab = lshr i64 19214116860268544, %s
  %lg = and i64 %tab, 15
  %r = shl i64 %n, %lg
  ret i64 %r
}
define internal i64 @df.typed_base(i64 %t) alwaysinline nounwind {
  %a = add i64 %t, 30
  %p = inttoptr i64 %a to ptr
  %b = load i64, ptr %p, align 8, !invariant.load !{}
  ret i64 %b
}
define internal i64 @df.typed_offset(i64 %t) alwaysinline nounwind {
  %a = add i64 %t, 38
  %p = inttoptr i64 %a to ptr
  %d = load i64, ptr %p, align 8, !invariant.load !{}
  ret i64 %d
}
define internal void @df.keep_alive(i64 %l) alwaysinline nounwind {
  ret void
}
define internal i64 @df.core_len(i64 %l) alwaysinline nounwind {
entrada:
  %c = call i64 @df.class(i64 %l)
  %d = sub i64 %c, 8
  %e = icmp ult i64 %d, 3
  br i1 %e, label %lista, label %fim
lista:
  %a = add i64 %l, 14
  %p = inttoptr i64 %a to ptr
  %n = load i64, ptr %p, align 8
  br label %fim
fim:
  %r = phi i64 [ %n, %lista ], [ 0, %entrada ]
  ret i64 %r
}
define internal i64 @df.core_storage(i64 %l) alwaysinline nounwind {
entrada:
  %c = call i64 @df.class(i64 %l)
  %e = icmp eq i64 %c, 10
  br i1 %e, label %expansivel, label %fim
expansivel:
  %a = add i64 %l, 22
  %p = inttoptr i64 %a to ptr
  %s = load i64, ptr %p, align 8
  br label %fim
fim:
  %r = phi i64 [ %s, %expansivel ], [ %l, %entrada ]
  ret i64 %r
}
define internal i64 @df.core_shape(i64 %a) alwaysinline nounwind {
  %f = add i64 %a, -1
  %p = inttoptr i64 %f to ptr
  %b = load i8, ptr %p, align 1
  %m = and i8 %b, 102
  %r = zext i8 %m to i64
  ret i64 %r
}
define internal i64 @df.core_len_writable(i64 %l, i64 %forma) alwaysinline nounwind {
entrada:
  %c = call i64 @df.class(i64 %l)
  %fixa = icmp eq i64 %c, 8
  %exp = icmp eq i64 %c, 10
  %mod = or i1 %fixa, %exp
  br i1 %mod, label %forma.conf, label %fim
forma.conf:
  %a = call i64 @df.core_storage(i64 %l)
  %f = call i64 @df.core_shape(i64 %a)
  %ok = icmp eq i64 %f, %forma
  %la = add i64 %l, 14
  %lp = inttoptr i64 %la to ptr
  %n = load i64, ptr %lp, align 8
  %g = select i1 %ok, i64 %n, i64 0
  br label %fim
fim:
  %r = phi i64 [ %g, %forma.conf ], [ 0, %entrada ]
  ret i64 %r
}
define internal i64 @df.word_ref(i64 %e, i64 %i) alwaysinline nounwind {
  %o = shl i64 %i, 3
  %a = add i64 %e, %o
  %p = inttoptr i64 %a to ptr
  %r = load i64, ptr %p, align 8
  ret i64 %r
}
define internal i64 @df.simd_box(<2 x i64> %v, i64 %cid) alwaysinline nounwind {
  %c = shl i64 %cid, 32
  %cab = or i64 %c, 131585
  %h = call i64 @df.allocate(i64 %cab, i64 2)
  %a = add i64 %h, 14
  %p = inttoptr i64 %a to ptr
  store <2 x i64> %v, ptr %p, align 8
  ret i64 %h
}
define internal i64 @df.class(i64 %h) alwaysinline nounwind {
  %z = icmp eq i64 %h, 0
br i1 %z, label %nulo, label %s0
nulo:
ret i64 1
s0:
%b = and i64 %h, 1
%i = icmp ne i64 %b, 0
br i1 %i, label %smi, label %s1
smi:
ret i64 2
s1:
  %m = and i64 %h, -9223372036854775801
%o = icmp eq i64 %m, 2
br i1 %o, label %obj, label %rt
obj:
%p = inttoptr i64 %h to ptr
%cp = getelementptr inbounds i8, ptr %p, i64 2
%c = load i32, ptr %cp, align 4, !invariant.load !{}
%r = sext i32 %c to i64
ret i64 %r
rt:
%x = call i64 @dartforge_value_class(i64 %h)
ret i64 %x
}
define internal ptr @df.selector(ptr %c, i64 %r, i64 %h, ptr %n, i64 %l) alwaysinline nounwind {
%cid = call i64 @df.class(i64 %r)
%k = add i64 %cid, 1
%c0 = load i64, ptr %c, align 8
%sim = icmp eq i64 %c0, %k
br i1 %sim, label %acerto, label %falha
acerto:
%ep = getelementptr inbounds i64, ptr %c, i64 1
%e = load ptr, ptr %ep, align 8
ret ptr %e
falha:
%f = call ptr @dartforge_selector(ptr %c, i64 %r, i64 %h, ptr %n, i64 %l)
ret ptr %f
}
define internal i8 @df.subclass(i64 %cid, i64 %alvo) alwaysinline nounwind {
%ctx = call ptr @df.context()
%lp = getelementptr inbounds i8, ptr %ctx, i64 352
%l = load i64, ptr %lp, align 8
%d1 = icmp ult i64 %cid, %l
br i1 %d1, label %t1, label %lento
t1:
%np = getelementptr inbounds i8, ptr %ctx, i64 344
%n = load i64, ptr %np, align 8
%d2 = icmp ult i64 %alvo, %n
br i1 %d2, label %t2, label %lento
t2:
%tp = getelementptr inbounds i8, ptr %ctx, i64 336
%t = load ptr, ptr %tp, align 8
%mp = getelementptr inbounds ptr, ptr %t, i64 %alvo
%mapa = load ptr, ptr %mp, align 8
%tem = icmp ne ptr %mapa, null
br i1 %tem, label %t3, label %lento
t3:
%w = lshr i64 %cid, 6
%wp = getelementptr inbounds i64, ptr %mapa, i64 %w
%pal = load i64, ptr %wp, align 8
%s = and i64 %cid, 63
%x = lshr i64 %pal, %s
%x8 = trunc i64 %x to i8
%b = and i8 %x8, 1
ret i8 %b
lento:
%y = call i8 @dartforge_is_subclass(i64 %cid, i64 %alvo)
ret i8 %y
}
define internal i1 @df.is_object(i64 %h) alwaysinline nounwind {
%m = and i64 %h, -9223372036854775801
%r = icmp eq i64 %m, 2
ret i1 %r
}
define internal i1 @df.young_child(i64 %v) alwaysinline nounwind {
%b = and i64 %v, 1
%imediato = icmp ne i64 %b, 0
%nulo = icmp eq i64 %v, 0
%nada = or i1 %imediato, %nulo
br i1 %nada, label %nao, label %s1
s1:
%m = and i64 %v, -9223372036854775801
%o = icmp eq i64 %m, 2
br i1 %o, label %obj, label %nao
obj:
%p = inttoptr i64 %v to ptr
%ep = getelementptr inbounds i8, ptr %p, i64 -2
%e = load i8, ptr %ep, align 8
%ey = sub i8 %e, 1
%j1 = icmp ule i8 %ey, 1
%j2 = icmp eq i8 %e, 6
%j = or i1 %j1, %j2
ret i1 %j
nao:
ret i1 false
}
define internal void @df.barrier(i64 %o, i64 %v) alwaysinline nounwind {
%p = inttoptr i64 %o to ptr
%ep = getelementptr inbounds i8, ptr %p, i64 -2
%e = load i8, ptr %ep, align 8
%velho = icmp eq i8 %e, 3
%x = call i1 @llvm.expect.i1(i1 %velho, i1 false)
br i1 %x, label %filho, label %fim
filho:
%j = call i1 @df.young_child(i64 %v)
br i1 %j, label %lembrar, label %fim
lembrar:
call void @dartforge_remember(i64 %o)
br label %fim
fim:
ret void
}
define internal void @df.element_barrier(i64 %o, i64 %i, i64 %v) alwaysinline nounwind {
%p = inttoptr i64 %o to ptr
%ep = getelementptr inbounds i8, ptr %p, i64 -2
%e = load i8, ptr %ep, align 8
%e3 = sub i8 %e, 3
%velho = icmp ult i8 %e3, 2
%x = call i1 @llvm.expect.i1(i1 %velho, i1 false)
br i1 %x, label %filho, label %fim
filho:
%j = call i1 @df.young_child(i64 %v)
br i1 %j, label %lento, label %fim
lento:
%fp = getelementptr inbounds i8, ptr %p, i64 -1
%f = load i8, ptr %fp, align 1
%fc = and i8 %f, 8
%temc = icmp ne i8 %fc, 0
br i1 %temc, label %cartao, label %lembrar
cartao:
%lp = getelementptr inbounds i8, ptr %p, i64 14
%len = load i64, ptr %lp, align 8
%w = lshr i64 %i, 11
%k = add i64 %len, %w
%kb = shl i64 %k, 3
%kd = add i64 %kb, 22
%cp = getelementptr inbounds i8, ptr %p, i64 %kd
%cv = load i64, ptr %cp, align 8
%s = lshr i64 %i, 5
%s63 = and i64 %s, 63
%bit = shl i64 1, %s63
%nv = or i64 %cv, %bit
store i64 %nv, ptr %cp, align 8
br label %lembrar
lembrar:
%ev = icmp eq i8 %e, 3
br i1 %ev, label %chamar, label %fim
chamar:
call void @dartforge_remember(i64 %o)
br label %fim
fim:
ret void
}
define internal i64 @df.allocate(i64 %cab, i64 %w) alwaysinline nounwind {
%pequeno = icmp ule i64 %w, 16
br i1 %pequeno, label %tlab, label %lento
tlab:
%ctx = call ptr @df.context()
%o16 = shl i64 %w, 4
%oc = add i64 %o16, 64
%ta = getelementptr inbounds i8, ptr %ctx, i64 %oc
%tb = load ptr, ptr %ta, align 8
%of = add i64 %oc, 8
%tfa = getelementptr inbounds i8, ptr %ctx, i64 %of
%tf = load ptr, ptr %tfa, align 8
%w8 = shl i64 %w, 3
%tam = add i64 %w8, 16
%tnx = getelementptr i8, ptr %tb, i64 %tam
%esgotada = icmp ugt ptr %tnx, %tf
%x = call i1 @llvm.expect.i1(i1 %esgotada, i1 false)
br i1 %x, label %lento, label %rapido
rapido:
store ptr %tnx, ptr %ta, align 8
store i64 %cab, ptr %tb, align 8
%hb = ptrtoint ptr %tb to i64
%h = add i64 %hb, 2
ret i64 %h
lento:
%cid = lshr i64 %cab, 32
%fl = lshr i64 %cab, 8
%fn = and i64 %fl, 16777215
%r = call i64 @dartforge_allocate(i64 %cid, i64 %w, i64 %fn)
ret i64 %r
}
define internal ptr @df.body(i64 %o, ptr %ctx) nounwind {
  %k = and i64 %o, -9223372036854775801
  %e = icmp eq i64 %k, 2
  %a = add i64 %o, -2
  %q = inttoptr i64 %a to ptr
  %vp = getelementptr inbounds i8, ptr %ctx, i64 40
  %v = load ptr, ptr %vp, align 8
  %h = select i1 %e, ptr %q, ptr %v
  ret ptr %h
}

; Constantes de string WTF-8
@.str.0 = private unnamed_addr constant [17 x i8] c"F\3C0\3BC674\3B1\3BC674\3B\3E"
@.str.1 = private unnamed_addr constant [7 x i8] c"L\3CC674\3E"
@.str.2 = private unnamed_addr constant [6 x i8] c"Object"
@.str.3 = private unnamed_addr constant [5 x i8] c"_Type"

; VTable of class Object (id 0)
; VTable of class _Type (id 1073741569)
; Closures: invalid entry and constant vectors
define internal i64 @df_clo_invalido(i64 %c, ptr %a, ptr %d) nounwind {
b0:
  ret i64 0
}
@df.const.i64.2268.45aa9b8ae90a7e326e08e11079e24fa60f9b25ce32d38d12abd8bead13467dd2 = linkonce_odr hidden unnamed_addr constant [2268 x i64] [i64 308367281505280, i64 -5252192380344477064, i64 -8678401560663039826, i64 4115361295180275170, i64 -3751705452839592546, i64 -7020068229338503334, i64 6863142145987595053, i64 -7572050702025882305, i64 5791067287842830943, i64 -7218364637318575872, i64 -4845314922291241377, i64 -1011359385498297415, i64 -3907052041379469384, i64 -1602221965896222850, i64 -434322789991964741, i64 -99567725278044688, i64 -9176038587670054173, i64 8953136210018123519, i64 935914361228815735, i64 -8254445358006282919, i64 5361505271691897746, i64 -306262513963528329, i64 5006510924171987905, i64 -6918834560521062450, i64 -5508026108914072375, i64 -821314721485405708, i64 9087734334566811249, i64 7058823535279992575, i64 -2822184944647289857, i64 -134307187690752933, i64 -1226206159909160325, i64 -6927665071914350278, i64 -7303902549039738225, i64 -4668747757088615142, i64 4070020928556134193, i64 -7247527705907258805, i64 -8242694126043594842, i64 8083924785312083328, i64 -1157351286424858072, i64 919556471190026774, i64 -7151927729769625323, i64 6228554593626813302, i64 3703933987832817412, i64 -3782411561400554260, i64 3222363791025813538, i64 -565145011596604542, i64 -1786214639503695876, i64 -4530473391080110670, i64 1322444898221310044, i64 -7223302958526363448, i64 -7116563069584016405, i64 -7355249702158705523, i64 8752044001483301264, i64 3264367412966061907, i64 -4322168804351437106, i64 -8648160760384710983, i64 -4697896456218013765, i64 4978646851636163523, i64 125591990676616405, i64 -5406718805786649994, i64 -1612995409919021573, i64 1427542865244750308, i64 -612151820108529664, i64 6700335570502782967, i64 -2450618076810457667, i64 1491662876743491021, i64 173127451919504215, i64 -2618050482743436682, i64 2142547210556759401, i64 -6640851386109226368, i64 6407299449531884287, i64 7394545601903963484, i64 -2473484220771953150, i64 500896115257417324, i64 2614115069832058964, i64 -2708271332807099807, i64 156347280588379346, i64 -8757027820173982815, i64 6017630026109371154, i64 -144316117733407686, i64 8486239250262952112, i64 -5560930496328608210, i64 -2807984528577319623, i64 979850202846926684, i64 3284876418222542081, i64 -4725224824978396261, i64 -3185385259487365429, i64 -4146310488098606012, i64 8832358940839416402, i64 -782465109141264083, i64 621044321649381981, i64 5661640271662322280, i64 -3674012322697929641, i64 -5295543349093395633, i64 3284956847455812322, i64 -9018049039685199512, i64 2395929473028468118, i64 821204810877507203, i64 -3886349957016560852, i64 -253484225177003242, i64 -6288614373843223976, i64 4631707020194122661, i64 -3925719056308824248, i64 6445060850378293316, i64 6712689624093485439, i64 -5302761413131011350, i64 -3288985462926845146, i64 578976260343801266, i64 261494280563671257, i64 1627711597154935377, i64 -3790614214251010343, i64 3202690165769823341, i64 -3254311231165376931, i64 -6508588876966318747, i64 2766549950495481291, i64 -7545573667106240096, i64 1879639884853754031, i64 6549486637621700793, i64 5002571570972308849, i64 -8401339771595237685, i64 1158234759501843862, i64 7831950599978815546, i64 2324883493358506341, i64 -5414204756008985701, i64 -2528444398612680131, i64 5354162660947764388, i64 -5056888795566188742, i64 37710861909166505, i64 6678438169194638994, i64 2528968253828602481, i64 1869672367890054221, i64 5719330265780326294, i64 -3489645868157044426, i64 89059122208406704, i64 -8919624144629159896, i64 -1599877712573331367, i64 7716750329209508040, i64 4345195026450975143, i64 -5492795894800333397, i64 271811612525902938, i64 -8406399025736702717, i64 8169018526145231719, i64 7582340505197097530, i64 -8524624675639052726, i64 -3933123942703223336, i64 -7843923012321385444, i64 4833203072852779556, i64 4633240987471385091, i64 -5290858855993080813, i64 7964701383386456457, i64 1198603283261213844, i64 658872663231505564, i64 -9089631019529499183, i64 4836266104821678544, i64 -7821144447107185755, i64 -8682618355507504093, i64 7274563474985655811, i64 658812406266695195, i64 -7781977418516063172, i64 3748166154136383524, i64 1846589511129028673, i64 7045878859056084735, i64 1726205614426482687, i64 -2181513662908011884, i64 -5063486643304810060, i64 6770937064561685729, i64 1989214703206546359, i64 -4976554875794109042, i64 1074236646538465681, i64 7074142693578487202, i64 -7188670123040397783, i64 4375066978845211830, i64 3935081984836810396, i64 -4338264141534253000, i64 7088099249629554097, i64 -3141599004844158159, i64 -5977860022029031093, i64 1300879563360244237, i64 983774894255902119, i64 5481458707676911378, i64 -8039926848871722110, i64 -3681594190290113352, i64 -7578832162309892286, i64 5631429533946129971, i64 2799917522135951067, i64 3796014259270769582, i64 -4261696328159608705, i64 -340741716415713871, i64 -6347388656454423037, i64 789147254941988414, i64 2998093509738537092, i64 -7224291004979242146, i64 6785125554654502441, i64 -7016449871304185445, i64 -2055962673455098675, i64 -5987004186680605788, i64 -8272751572977658988, i64 7553405795705606934, i64 3806410908707961649, i64 -5823968208394665681, i64 -2263141922332921503, i64 6037034946460748557, i64 -4805250595310048821, i64 3268442948046807765, i64 -4793880116088967330, i64 796844538218668757, i64 -1050783126566915291, i64 402685906786204502, i64 3727417637206781301, i64 -4793876266282089618, i64 3367522141325353707, i64 -2920378821109687362, i64 6897173869503645564, i64 5688457402714353468, i64 -809083771572842757, i64 2380043545453345917, i64 8699388080519107845, i64 6766530514353580477, i64 -728640051203162753, i64 -2427355532275414106, i64 -6866900839070502540, i64 5951959527514768798, i64 7279684608941519058, i64 -3921474816940377230, i64 8971231470839563245, i64 -2454281695789636016, i64 994337690973499904, i64 -7889115650749859263, i64 -2068143281086884346, i64 5883026137773194073, i64 998193081443752523, i64 -4749263846903570652, i64 3789606778421790747, i64 5931389571297372010, i64 -3943470595274141770, i64 -241828433834266982, i64 3743216837552545548, i64 5733776046721749628, i64 1999063846744018890, i64 7485144222843616844, i64 -919223489678395784, i64 5916457567446058099, i64 -2176481484530209634, i64 -1186929027802631501, i64 -5063751436260634586, i64 -4352962968420631012, i64 1962571314924547815, i64 -4815115491216933318, i64 9071680400051519858, i64 -2164097959921409288, i64 5871700193887318667, i64 -2243372075090737186, i64 -4328235665470175759, i64 -3070318416555190861, i64 -4595223873770849220, i64 -1802797178559605727, i64 -106239865135465940, i64 -2100355406289353083, i64 -2735405859669353975, i64 9022753285317265890, i64 3278968365283882626, i64 1424738529814589598, i64 6039754726139618008, i64 -8301437531942885879, i64 6575742745573129023, i64 -6826230799308996284, i64 4763578345835283547, i64 -5753930145882563775, i64 3421405351266227269, i64 -7228188243766661759, i64 -2411961813928857379, i64 -6754315642780419240, i64 8852399540312594815, i64 -2735238733891970550, i64 -4432991144646997382, i64 -2972191390425450460, i64 377576732831166547, i64 273066293421317814, i64 1837922342908849672, i64 -5295258582561196417, i64 4804978524844098222, i64 -2419696687883571529, i64 6941690590943176783, i64 9148671309032467179, i64 -5196965208297874686, i64 -3410074844569631002, i64 -2774320182504771299, i64 -2735098009280128495, i64 791184425250261754, i64 2478284893244291805, i64 -1194648660843661920, i64 6992677177434419240, i64 -1776806202468450717, i64 -6708153441865649533, i64 -3177240656186844801, i64 -9021634456090337103, i64 6602645912513355070, i64 800864502330376550, i64 -7277079266295661956, i64 -8942258727741980314, i64 3539214117388191726, i64 -4825346692106796133, i64 6574077152335238579, i64 7963891290925308833, i64 4633299417560843695, i64 6303721210481871083, i64 1965921940383673729, i64 2250318916131308019, i64 -208503895151157868, i64 9002486057418125003, i64 -858140538456261371, i64 -6711195793115031602, i64 -2281127155313444341, i64 -2821481012917129315, i64 8367465716494658571, i64 867205178566632511, i64 3186149478469968397, i64 3981959939113410024, i64 865497117955621637, i64 1285356054838523770, i64 1946345547922545596, i64 6381255772227862921, i64 -2903262727921112453, i64 7633812269154836502, i64 695746941895463234, i64 2927779681110653253, i64 4642795316529342111, i64 2343813870629200518, i64 6962215506163447741, i64 4964832925193220819, i64 1191383332544429724, i64 6096823811924628739, i64 5716365647026963535, i64 2595468982481874628, i64 -1543735487279416367, i64 8609556220140997641, i64 -4694702182795371219, i64 3769933247778973324, i64 -5055514231069526817, i64 -8279877173112356730, i64 -3876367970802139406, i64 9185450485578828795, i64 1887997212431955153, i64 -4122834644374520347, i64 -6564051594837269404, i64 4308628263103551638, i64 -6697150889179847205, i64 6236425325797039701, i64 -1212926730605328519, i64 5065264003092503088, i64 -2803359121237885781, i64 -5015189991820850096, i64 -3823386278981710037, i64 863890403960265835, i64 -4852832841815674385, i64 1783912643322767559, i64 -1413041304184622435, i64 7594410907610145542, i64 1542636988041607085, i64 -2222928451958398910, i64 1961014628593441158, i64 6659878543869748480, i64 -6797220087624149159, i64 4829767533341323297, i64 4028125544583901295, i64 -6999198009045151501, i64 -365074316230362370, i64 -1748751279633139227, i64 8891165873849400895, i64 8295575357962902894, i64 -3781136509144750639, i64 -981710341432730579, i64 -7706659408957093834, i64 -4764483722873930055, i64 -8292493569116982503, i64 -146252974207156202, i64 5616625484849776568, i64 1252959678495015458, i64 7074077791796900017, i64 4942788588661838177, i64 5502317696377038333, i64 6954221746247790457, i64 -3999649042148270535, i64 8430607537305685549, i64 3573429618444107703, i64 6101361795839521263, i64 -7096511759334619635, i64 -8495789790273105856, i64 -7718553126565738228, i64 3661460517933184145, i64 -7434637493438881485, i64 2109532992426989815, i64 1592623640288443811, i64 -6760971467375464239, i64 5155007881713427345, i64 6617277018848055836, i64 -6408092269233857466, i64 5604579358470444944, i64 -8591820259480572653, i64 1201360988992164734, i64 9143917534856210859, i64 8095815072464771958, i64 1233593374204941023, i64 4223003799804091090, i64 -9220158168567308739, i64 3735822496327601496, i64 -6503742354921246495, i64 3057387026648343813, i64 7673272816001559502, i64 -4491723952038728501, i64 -1991673429591719408, i64 727863648444815113, i64 1803205422129465612, i64 -8360100877729010815, i64 7506528588595460835, i64 -7120882153694609758, i64 1596106899171685399, i64 7506459365098959156, i64 -3795662869873213086, i64 -8366472863264519814, i64 2737461023074457589, i64 -8089840833822261579, i64 -4964797087209311536, i64 -2479301971142467837, i64 -2942451189920566275, i64 -7138356099329525873, i64 -6349043961364131671, i64 -5008882865570478820, i64 -640324110608339747, i64 -5059988112548310717, i64 2249520115504320474, i64 4207465121616521997, i64 -912759179504478382, i64 -2874757698772183766, i64 4230302442138024947, i64 907865820902095709, i64 -8649264212822313493, i64 6544520694273539581, i64 -367005603383366960, i64 -1578906493796944388, i64 -2619511861155470340, i64 -7292978920054216080, i64 -2188796229595296117, i64 -7485503722100503294, i64 -1781739456150646557, i64 6039289569744319633, i64 4241251122957860423, i64 -3514519389465648403, i64 2748852287092278362, i64 -7510904789447268744, i64 6137940845270401987, i64 4353763618801829546, i64 3432060154064870123, i64 2004761658867663355, i64 -3828117435327001395, i64 7851694944818976539, i64 633630123902794540, i64 -1881603397112408399, i64 -6985986031211878917, i64 -4969002510749265395, i64 -5203984691442093370, i64 -2743719446442489419, i64 87352329269737004, i64 -4814131480114819874, i64 6946809850075061458, i64 368547612563878915, i64 2173950233511318019, i64 -7265710313045558662, i64 5850494354512037568, i64 7289330710047367984, i64 1861536560728612132, i64 3100207471201699997, i64 2714548952176126956, i64 -5006037259506225999, i64 -8058406992719781675, i64 -2598986665459024434, i64 7067705197212832858, i64 697407601726374720, i64 -3833502230594436407, i64 913967870491959440, i64 -5575557033437513398, i64 7009054091542636844, i64 2283061415520255833, i64 7032375387647213370, i64 -1807231905681689646, i64 -6895780168214057907, i64 6864085960747372080, i64 -7666437436419718582, i64 -6721716405409273493, i64 4971818029674966506, i64 3811106931755532245, i64 9183092399008165080, i64 1148555703565303735, i64 637184241134580495, i64 3690567590841611989, i64 -594505907925255236, i64 5876490953209222087, i64 -290868122769235199, i64 1269232676532582415, i64 269196298084919462, i64 -3915134558738987579, i64 -4497152728920591931, i64 -6587688275558146893, i64 -3162891398293901682, i64 -5675923215282493999, i64 -3797977224922872636, i64 4742850511017509979, i64 -3711640834401895868, i64 -8028539582649517392, i64 6558075216550084526, i64 3201259255145002367, i64 5006208616715984721, i64 -8131438242866041739, i64 -4314641517885298354, i64 -4557685179870480983, i64 -6103898886476270248, i64 6867390660768029677, i64 -156467035397182894, i64 -6377935093847897454, i64 -2750766444411623811, i64 -331219898998072898, i64 -1988850420861929882, i64 -587479963465766715, i64 -3322011117248801277, i64 -5749989525484052874, i64 -2898898816648792221, i64 4784951698601408462, i64 -1300454030774234802, i64 5836733545471865073, i64 3403173950067875447, i64 1030688811741800004, i64 -3172256046366853718, i64 -3184070258902961693, i64 911911807895312535, i64 -2351066607186725977, i64 -6013141704761172241, i64 -8614627189336618315, i64 -8714482947320506016, i64 -5967825398055816237, i64 330415461539752528, i64 -6121189629802749515, i64 -7648269309649246642, i64 2046455403974618059, i64 6641204720897258812, i64 -8484395156877983984, i64 -597569887296420203, i64 -5155220253289572347, i64 -4679249496152552387, i64 -504544475583283553, i64 -4144645710397112601, i64 -3793573128513627499, i64 2958958043654687107, i64 925023743806353534, i64 -7800319218218371842, i64 -6317236241748748836, i64 5494373811142110338, i64 7880401492147633663, i64 4674113932306065254, i64 7981211701965205026, i64 -6068261979441373542, i64 6067787961386479635, i64 6801728370996360603, i64 7480627008480241769, i64 -4171804859786073783, i64 -2799799990340336244, i64 -7164531609006245707, i64 -4183819897534916647, i64 3006513519099529804, i64 -5924063276908899189, i64 -7725508033817871719, i64 3629621334451520891, i64 2960075679088186597, i64 -8073540139304151871, i64 -5866600775214522465, i64 3845725966891817172, i64 5571044186097145324, i64 7982813931155675537, i64 -7485953092630908343, i64 695874150979223384, i64 4112918175142990519, i64 895111631446136937, i64 5382587186283809702, i64 -8793000673603822676, i64 -3049503690211525206, i64 8494822234150945944, i64 2592771498783115036, i64 6080808722945332589, i64 3851713635592464576, i64 -4841797609746880611, i64 -7292383617557449784, i64 7823752565415208587, i64 6108270969633535190, i64 5224920651155854529, i64 3914092459192061425, i64 -8230055960981877022, i64 8770591757611414968, i64 57674743275737140, i64 2941948884334581602, i64 6173071574026557103, i64 -3221157029987649129, i64 -951503262159733971, i64 -4633733217058094468, i64 -5954181512957596132, i64 -4896356244279243848, i64 -7017389425921871106, i64 -980887415684686274, i64 3079834370732322154, i64 -5391549375842676264, i64 -1846713557971676171, i64 -4725413199397808315, i64 7574296624964680516, i64 -1845560385800992013, i64 4359132294523255751, i64 -6033454871405811426, i64 -2486093268613801856, i64 -6499488825765661027, i64 -8268131966345723212, i64 1723401740744376708, i64 3193243625405721628, i64 -6321548975787691194, i64 -6711793549448148181, i64 442718209522105072, i64 -786170720945850187, i64 3024963565999762188, i64 1659806318993243836, i64 5536791866703459967, i64 2511660742253688289, i64 8753506855298349076, i64 -1533127286076444027, i64 -4561271715078880935, i64 -6871736416595731193, i64 -1513014388166207455, i64 -2784696883795529560, i64 5284488550307685724, i64 697095807162651122, i64 -3621256904483955018, i64 343000097482084850, i64 -6650815502575633977, i64 7665705360219468616, i64 -3698604953113862162, i64 -607849088434917299, i64 -7272465240828824186, i64 3706840971129918856, i64 -503338019398919146, i64 -6975281963974634901, i64 -537642967359761103, i64 -1861332961072703565, i64 -9201060165722659275, i64 523779770273004175, i64 3993022078416063140, i64 2571642459897725055, i64 7094816755796408151, i64 -5758598867935986868, i64 -311827257778802374, i64 -5584079929453056129, i64 3353923927327757589, i64 -8554415602464303967, i64 2812547849157085329, i64 6912722512492826446, i64 3464702788667571632, i64 -8398948390293208774, i64 -822986332148377835, i64 -8222436235767910390, i64 -3675621157769865995, i64 5420452853760428740, i64 2066888512109563308, i64 -2177497019318304068, i64 3824782368978876510, i64 394178482557056342, i64 -7389533373726543079, i64 -473311243226353364, i64 -304980407835214362, i64 -3555543408424936205, i64 -234681708791665936, i64 5630375135022500918, i64 5864386608936066053, i64 3971061798112159626, i64 -4485638177689179378, i64 766356147556882229, i64 -7177919219179520699, i64 229838042240619904, i64 -642840968654135038, i64 4003872672190259574, i64 8656279261701658196, i64 7443361347710262182, i64 1547255209377510793, i64 -6710826447935054696, i64 1107349210437229240, i64 -5591839859883332442, i64 796203701062949773, i64 -2630112492999679189, i64 -3060828141636430381, i64 4772899050787282521, i64 1405003612365118553, i64 5799042567624590001, i64 1512863340032680600, i64 8967405535329425062, i64 -6523338029308763495, i64 -4125623857257658142, i64 1978660610819741553, i64 318591633987522621, i64 -5956431230703072357, i64 5291974475638025024, i64 4603758623655278901, i64 -1009091108469604738, i64 6537883885593133832, i64 -2190299344461571170, i64 -2339707993236471971, i64 -126596944861348978, i64 5946199794810100203, i64 -865063444494817891, i64 -6027299513913180785, i64 -3033830385235877094, i64 -889812034493705366, i64 -5823894610161342236, i64 -4354161049709339817, i64 8851094259069241788, i64 4808357662718421221, i64 -1489080850395122461, i64 -833497245822553387, i64 -6182972497685405167, i64 -8853144002613863485, i64 -5233132537815891743, i64 -5630452902390939558, i64 4806034381080596084, i64 9116174404617451559, i64 2993901466162518055, i64 4100242120701639289, i64 -5423542002935114932, i64 -3614229443096201044, i64 -2481551041090034699, i64 -5222110545614940439, i64 1187209791675519354, i64 -5922611845867972481, i64 6615191732802150870, i64 -946130029575159016, i64 1149929028438505386, i64 3558085997630221615, i64 -6388952627393405623, i64 -2469597139142099390, i64 -1622501965984460534, i64 -2497090432853821345, i64 -8383989641114167944, i64 4208058669062515027, i64 5047256376236738490, i64 7177986111772908422, i64 -360712668459859787, i64 536311248009026416, i64 -2519328875982835047, i64 -2314982500099944960, i64 -7728268723933130769, i64 -2373464011424282796, i64 2283247802864451565, i64 -8955445545246165129, i64 6212364430920111941, i64 7235610381334313077, i64 3884890964933571041, i64 6526356128064859483, i64 -202089717294716342, i64 -5222796839166549311, i64 8528680010538466120, i64 6914260990922726517, i64 791555086451931201, i64 -1002700781603228750, i64 -2911182155617022989, i64 8520245225273626511, i64 1538221142288240478, i64 6094822876166766768, i64 -5080498579897529558, i64 -4330881599371751764, i64 8607504649997236462, i64 7726789487616953184, i64 2022850796901689996, i64 -5484826858371618744, i64 4267758382976537067, i64 1767790840976770062, i64 -5127956681404384898, i64 -5212045257968043336, i64 -5997131689846270507, i64 -513504037758619270, i64 -2581830445082988078, i64 -5037808704428552012, i64 267337207577240452, i64 1399871104879663731, i64 4941901720153141014, i64 6449997206771293610, i64 -8751171173422247910, i64 -2299310566294052396, i64 6007101442139207505, i64 8096572882222794577, i64 -8895814107813755813, i64 8668030801324258266, i64 6081655580117754762, i64 -4943656083440771665, i64 -4063019893191594601, i64 -417832517295371761, i64 -8221435711509248225, i64 2115430377984365897, i64 398801758566817670, i64 3938699592637958841, i64 8574409746160632218, i64 1836704734287740152, i64 5283152633288917716, i64 -2482642515585848664, i64 -3462702338927326336, i64 560645370596494411, i64 -4559134651121849828, i64 -1306199918410726513, i64 -8312140441189353489, i64 -544397282855831467, i64 -3539870553855480506, i64 -5645517178017037250, i64 -8909121702753417538, i64 -1599429951278055639, i64 -6067721451940564717, i64 -652092041998025589, i64 5842780330110715897, i64 2112897836736196148, i64 -8391010457588018617, i64 2764595933347353454, i64 -6711224550189242705, i64 2933880010184413780, i64 -1933538112828709867, i64 -6006023637728245180, i64 -6666671900248806204, i64 -222933148590657881, i64 7775609841777021184, i64 927343258037709476, i64 1003957404084787887, i64 5889963603761444587, i64 1705549721061875373, i64 -774153127550039213, i64 1059084045955977321, i64 9165126664114883208, i64 -6053156914331959491, i64 -7175919933405463395, i64 5217996915839279850, i64 5262518909874087026, i64 -7358260238175960563, i64 3214149026843700409, i64 8566706564663563412, i64 3180013884616370344, i64 -8259476554126775701, i64 -6442079603030048573, i64 -7819307467364658643, i64 454604950208706682, i64 -7159312564192205492, i64 5233927416952617288, i64 -5505300058664375371, i64 2352907876034133634, i64 5974036950506919833, i64 334234261408490352, i64 6073725616376793655, i64 -7795108269374749533, i64 -4738842480989711822, i64 7582402353719008316, i64 -6655083748232381171, i64 6363659788670159075, i64 6380888470698658625, i64 -9043878906055047651, i64 -7290683036571059375, i64 6107257996503743855, i64 6401489833614975542, i64 6460819365109197016, i64 2742492388883035453, i64 -4726165503426230363, i64 -3663344482035747676, i64 -3147178701876930409, i64 7566667246051486018, i64 -7307917019722934629, i64 1534792429340349570, i64 7322072868410520221, i64 5678692909449715690, i64 1606483432871228335, i64 -7527791904920524967, i64 -3207134163253873841, i64 -9174322415967514710, i64 7342183363258227500, i64 3476698216325094685, i64 5580242899125786258, i64 -2693613699293072437, i64 8412261972198663147, i64 -2293614895167005095, i64 2136226003349539210, i64 -3632129038794215887, i64 2348384651849541273, i64 7365475337455615174, i64 -3642326715081935813, i64 3482520176988082664, i64 8256301241782776773, i64 -4937001626296068830, i64 6219804973369973172, i64 1938651235511700641, i64 -2452828356989581100, i64 -5416703088769761833, i64 6726943023928021710, i64 7149175011735793729, i64 -5796904244637569593, i64 1415050253679100992, i64 -6839028770901418747, i64 1687658696778270142, i64 5354804034917669475, i64 8239914113530107035, i64 6471343495949593881, i64 5283223012568452054, i64 -4348310516646878418, i64 948424196704366019, i64 -6821682138471328283, i64 1735862753224970941, i64 -7101760822641761292, i64 3496801732699139009, i64 -1851075371816907719, i64 726252766767737021, i64 996149158664747462, i64 -5032387160457564554, i64 8411625083504573112, i64 -2635354156256479453, i64 7415480461667474471, i64 3841987599691737411, i64 7650294008261635397, i64 -5011358074823201481, i64 -6250234686646349708, i64 5131911992259386160, i64 -1016689835291063012, i64 -2895749459181707825, i64 -4488326305686396853, i64 4163038367117934827, i64 -1219826344245711213, i64 -6532987371138690949, i64 3280552038436784284, i64 -4954591344381042310, i64 5985391422977151150, i64 8074114096254780227, i64 -7923958380861644583, i64 -4665767413109395297, i64 8837023425603084252, i64 2019368063565999403, i64 -3198375012819665699, i64 3049408180708690183, i64 8904006044853415072, i64 -2142375479851466610, i64 154044794362992168, i64 4018615550405378723, i64 8889155349896242102, i64 -6294432286181853555, i64 -5753870617083195070, i64 7096930108808176111, i64 3101616224499932003, i64 -8931719650055632364, i64 3793313400657510820, i64 5385361794670592366, i64 5358524709716268325, i64 -6504548082798912219, i64 -6538854112879884132, i64 -3419241425064867678, i64 4866321368690523688, i64 -6085594148360811268, i64 -777064855388040985, i64 3671734979161027057, i64 -8926092249888288025, i64 1747747573858796134, i64 -6504486828059652638, i64 3050603749772164168, i64 -6830066566865594028, i64 -5644623431859775660, i64 -7140920870102626425, i64 8258644328333837782, i64 -1522726681343468443, i64 -4299079776165010554, i64 7252487346665636389, i64 -2063178534255715317, i64 -3579889899598846674, i64 -1285346010406562271, i64 -7981747319347059900, i64 8705901174249517138, i64 -2170238081064778573, i64 -5274269989806957234, i64 -2191285727388489008, i64 -6117022856549546380, i64 8230120325406111965, i64 -2118550778738789185, i64 -7982265951823041115, i64 -1090255408168429990, i64 8054394389122523639, i64 734055383172001929, i64 -613214960652161484, i64 2097786480927267270, i64 3352721248069318842, i64 -7736263691179711104, i64 -7753011454394347102, i64 5075088498575946722, i64 -2885204979754453207, i64 7420907268968654787, i64 -1753219290421032523, i64 -4333750151367673816, i64 7351821291864299633, i64 -9011014942569935867, i64 5036287072891286706, i64 2003637950589870012, i64 6355328058623877933, i64 4009366886300162487, i64 -7189642476216087707, i64 -813765243066747297, i64 491791673485729998, i64 -3884373808346492459, i64 -7638643281141482896, i64 -2009985513205928387, i64 -4237242287908861218, i64 7370670793015136723, i64 5439099799289926277, i64 4595992142148771225, i64 -2696364303513144022, i64 -470893310457949211, i64 7454909560693719792, i64 4357612921433602709, i64 5957594612781852996, i64 -8709587109823293819, i64 -8393883160626851344, i64 -6363216515488078595, i64 5517653992874427652, i64 3174787274618257077, i64 5874230024292153649, i64 -4829121456931498563, i64 5709558071237421965, i64 -5730986690754210158, i64 -2288566010434764254, i64 -1793959201267219607, i64 -7175836806184057180, i64 -7950763990103680365, i64 2929961357061575245, i64 5884301762612319827, i64 -3622019217042905137, i64 -1389923396665537873, i64 -6140571595015803772, i64 -6115542306353663469, i64 -6015425151705102258, i64 1950818825544892730, i64 -5374885956538846027, i64 -5209955815437035898, i64 -7111783515992488541, i64 3291606537855874734, i64 2062747822950491984, i64 5921450308544883006, i64 7205944515065887680, i64 -2460015041399666621, i64 7297992132207288513, i64 1353591817732289889, i64 -3203415666773570389, i64 8821126351495475610, i64 2097499187858135310, i64 1405790853974057115, i64 452225638650660412, i64 7906202735406484373, i64 4420947354757871734, i64 8217059515950566407, i64 -9219342274516865296, i64 8013878915461435650, i64 8269414313714385016, i64 2672701295016953409, i64 -8425518400757729926, i64 2850296878066132300, i64 -4625348138098919180, i64 757694676626620396, i64 -1705706518895970555, i64 -8665289080497476579, i64 5005659930181167636, i64 -8503973895795861409, i64 2030014563213877656, i64 -8015140982042009400, i64 -5725391945361004028, i64 5845804163518900171, i64 -1554172614174187606, i64 2940912871294383322, i64 2294530343436418005, i64 1707572524427820922, i64 -3374659490668267960, i64 4925267297168069292, i64 8929164424362871475, i64 8923189393943618520, i64 5464834375665408679, i64 -5662271104248987988, i64 1134835587550780940, i64 -600482104444511014, i64 8836218507276282888, i64 6095307600893394243, i64 -1362074706556970005, i64 4868111834445247496, i64 -6156461746312348173, i64 -93778711352039171, i64 282365311576282416, i64 204545850061414111, i64 7682184728387694781, i64 7313762985657076924, i64 2579462387284406064, i64 -5549058026158727655, i64 5857482467742369118, i64 5462243224960914045, i64 -755467301357526371, i64 -8947327514426609838, i64 2642502977792895578, i64 2030874166273759112, i64 -2037884347206010628, i64 -4415120087411110018, i64 67609589167455912, i64 5850008754752977914, i64 6939625082179406909, i64 7581492477993067722, i64 5909033118097252484, i64 8288122200588696956, i64 -6250684750979615109, i64 -6298993434487906163, i64 -6111043567949325185, i64 3354877247574068751, i64 -7678922884845563306, i64 -1760398410396906993, i64 140238168895606703, i64 -3615286652607891106, i64 -6839995374569036705, i64 4399079625478599708, i64 7962858762256493782, i64 -1082063464417935377, i64 -7961224444804237243, i64 5107576770293315073, i64 -6032781944943489343, i64 -7456890239387378338, i64 -7708589962701090030, i64 -4149350669681925270, i64 -3017689076631382303, i64 6076128435938292026, i64 4189228226736039748, i64 3967787500205482397, i64 -3318819150875381948, i64 -2202447591145838058, i64 -530265921026884379, i64 -5598180330544365040, i64 -7891805413787430201, i64 1823343788338687118, i64 2211064693397310496, i64 -8103521894359896769, i64 -3045724424946759533, i64 -1842858059202550170, i64 2404377987773925122, i64 877111691131054858, i64 -1420009412724053801, i64 1048142115247789609, i64 -4058885066679396051, i64 -4057320565131656145, i64 -371795211548951893, i64 7434230330260285172, i64 -3938433823652944329, i64 -4265088790056033478, i64 6341201276952127608, i64 -7072708112558275185, i64 -4966551536283098743, i64 -6398727720138742019, i64 -4869220626567455910, i64 6071808592835200732, i64 -2377914397452748071, i64 7072204697004447103, i64 8543797038162199557, i64 4125493926498402099, i64 3103959609422370476, i64 -8340292621922805057, i64 -5661205946363494982, i64 4138571683540825736, i64 4696069407007744731, i64 -3497750735559056080, i64 2596086399219200115, i64 2529494684481264243, i64 -5825764465619429528, i64 -2038654565998175969, i64 1478991408401919739, i64 -1270255907440192908, i64 563227664406729926, i64 -1749741943129810779, i64 3626623400157534128, i64 -2796475981666739874, i64 2211075863476620467, i64 6475441458704529391, i64 -1577326822716100062, i64 -8293103036600019990, i64 -8135306370721637553, i64 -4439755775139467983, i64 -7943006537597781748, i64 -1042759959816528208, i64 1509459649007868969, i64 3660253765645431443, i64 7319811179050808881, i64 3092869674198928458, i64 -382753794015312719, i64 5329272243728851380, i64 6199389240031396461, i64 -4289039051809009674, i64 -3866985035133010261, i64 -6594587209859039699, i64 7517916641333968141, i64 4009000565994366095, i64 1402547695315668536, i64 8404480300134137004, i64 -9044814230256751868, i64 -4587792217541981564, i64 -2365740145658137839, i64 4087785310458802067, i64 -4995092126237492809, i64 -3695994064396890178, i64 646877770292491603, i64 6806286860333660642, i64 3235066016464231894, i64 -7057475235306285595, i64 3750296880259947721, i64 -6477078279644481340, i64 -688162434827439126, i64 -1176857377078802409, i64 5509463078199880380, i64 -5460681888049030386, i64 4148893745598915805, i64 2088193888963459019, i64 848720253018807125, i64 -1708038673893642483, i64 -1676469836283046751, i64 1791761387051838699, i64 -7656520424467339444, i64 -8148058755033082175, i64 8522729781927755644, i64 4314310993540754531, i64 -6905477246759257616, i64 -592490365780867891, i64 -3406620318525372978, i64 -1911669713703884992, i64 4207500760858185142, i64 -8728145610844928678, i64 4706764680951855267, i64 8725519019825787671, i64 9185398501606240261, i64 -677250374567925882, i64 -2534421246098769303, i64 -3121796410308856280, i64 1296820689855978721, i64 2024357939668999943, i64 1510453059902052732, i64 -7386056218790577046, i64 -7736441812220405446, i64 1773033558695186769, i64 -492839971576468840, i64 8715122964106821410, i64 6778753399003931329, i64 1820949693832339045, i64 -6728081502324046512, i64 -1049413338022108916, i64 -2328969033490238483, i64 -5615922829482773758, i64 910537020099609264, i64 -6709590159838429549, i64 -5224365055898378028, i64 1966239404955827647, i64 3242597942052355060, i64 -7077203753618128956, i64 1117503607911973419, i64 -4722263943541785962, i64 -4801111038107812738, i64 -2891174868088489288, i64 8498874648925497234, i64 6188155022960109734, i64 -7338367207824805777, i64 -8847048384329393875, i64 2592798002925779277, i64 -7546426782117665835, i64 -2554988183053270585, i64 -8686521176019852615, i64 6571422549236723913, i64 -7143030564486942468, i64 1702037590058249495, i64 6176646639789204353, i64 441905752587097064, i64 7679109381820614526, i64 -3794849101394756672, i64 1033980841990524837, i64 6048437926777511957, i64 -5828933975729067095, i64 1232612598018981382, i64 -3141322330261000464, i64 285505431491323678, i64 4736700752722015717, i64 5315726538741186726, i64 -569448473817995751, i64 3909015701405944078, i64 4669335480397506067, i64 3776072683648584345, i64 7263708453054619842, i64 4190693010705750625, i64 -2077980933394092893, i64 -2468398656829534699, i64 7008736407183289661, i64 371875160649798413, i64 -4946304649937866208, i64 3053760068026348869, i64 4202483088829635303, i64 -6777361172211083240, i64 240470839480912399, i64 -4467613499282340898, i64 -354757218592067364, i64 7135214393763003258, i64 5343222004730972214, i64 -2284049107091855108, i64 5018490279586489976, i64 2206034536909445359, i64 8325737780927454395, i64 1935300310292383453, i64 7977872517651082615, i64 2834390299394003324, i64 -5075682899194749255, i64 -881584028948401747, i64 9146761626686575563, i64 6624363286567579133, i64 -2385775915198716225, i64 -6353583204303397191, i64 956457967090951212, i64 5278553851614013478, i64 6564782260654123707, i64 -7377432492552982895, i64 3941661455058197187, i64 -3236974970822579387, i64 -3033701335394766057, i64 1586769773069511513, i64 1010026149930760605, i64 -992935522455908801, i64 -5974628751527221482, i64 -2506925049094890040, i64 6611649414607669422, i64 -8853389677307223627, i64 7548151218662868588, i64 5937815403606659558, i64 -8118220751639407053, i64 7497002775325719348, i64 -8974736741866065676, i64 6967807922489617804, i64 -7731162102855239838, i64 3040439296908034261, i64 -7387232868056298166, i64 -8670569271535937951, i64 -6450095717002481881, i64 6728631181450726632, i64 6447375477594744291, i64 7455027930020025446, i64 -5952188160661941145, i64 -1392626443161929850, i64 6451190182455951027, i64 714742507995140865, i64 2140841929102749043, i64 2138601298588383109, i64 8221447419331469460, i64 1238229912960463280, i64 5681927282512663273, i64 -6510258505635829235, i64 -5750751806440722103, i64 -1784296191049307732, i64 8016261762279687243, i64 -2044802508477006090, i64 -1306629363912305769, i64 -497282545310797793, i64 6010715462753423160, i64 8626595328592070787, i64 -967808928285743173, i64 8644974943609770624, i64 -5288480962471225645, i64 7916191474792834020, i64 -6214203524043129537, i64 8193005268590096129, i64 276665467192184259, i64 8968744925975935154, i64 -6178698950062293306, i64 3147243968655339265, i64 -7946108415061590295, i64 8980064267697100518, i64 8175166626748373635, i64 -6927050261115167959, i64 8100693042150010381, i64 -487526235120826894, i64 -4451014475655382974, i64 8017039441390897367, i64 -7529785293281241993, i64 -5547611810666687854, i64 -4620754175905024493, i64 3041941095730611004, i64 4039647963656114449, i64 -9058073982469999046, i64 -1049534749556088106, i64 6307969172239850662, i64 6499584704562542760, i64 -1947908307065132885, i64 8851363386731419304, i64 -7560581576998202608, i64 -8601985357136519499, i64 7643067980012037621, i64 4339516157970703728, i64 -513593401102839751, i64 7849891047048127479, i64 3605574070211392633, i64 -951140673412851786, i64 6765969748140260398, i64 -4703378133719334664, i64 -7112331804324930772, i64 -2376957764713783920, i64 -2560145840900924774, i64 -1173280284181836129, i64 3852985649295459887, i64 9044470559218420832, i64 4030493720789535783, i64 -9085543718105010392, i64 4028586979638549609, i64 2909162216054453928, i64 553005338395419314, i64 -733873821192760783, i64 5471015825553678114, i64 -1068672274447599003, i64 -6477288941185520788, i64 -3111325625829956754, i64 8798352067535098097, i64 -6804583766181603315, i64 4373925246585446620, i64 -4752288227452031751, i64 -8609925955150273932, i64 -7279077378104654410, i64 -4370361733063763158, i64 7882151276322524322, i64 445413754699089462, i64 -3881609775095345354, i64 -5470104703501439967, i64 349409449605466530, i64 -8993616615004493773, i64 -2873142874536015438, i64 6368803181588196659, i64 -1145484353822579454, i64 -7471651787240263139, i64 -5953673874788528416, i64 -1904794079136868415, i64 7199700840984879049, i64 4793926233132773124, i64 -353778008218309681, i64 -1230480511815484371, i64 -1889307214422509809, i64 6460064068916229386, i64 -4114546061268108928, i64 -7822379803217262124, i64 5065921264801120681, i64 -7546632526475237928, i64 6993136092162504619, i64 -2888423528809675308, i64 -2918090181094501468, i64 8972351148265901785, i64 -1890719618681655151, i64 5113552311878889624, i64 -4290923074208379410, i64 -9099194618401693163, i64 4760805936879574946, i64 -2220135479961107535, i64 3358647136847313589, i64 -6483196905590828048, i64 2124307211961821617, i64 -2630208632346757871, i64 -6677353054098831073, i64 -8984381254734418998, i64 8243306676371864700, i64 7897073818316387612, i64 2199947460637214206, i64 -998130958356909199, i64 8323517593949662083, i64 -1143631201178467963, i64 -7050354926974551867, i64 8478276018502177942, i64 -5930949387787876138, i64 2270567942604642578, i64 -7248892380473692479, i64 46003791595660451, i64 2477905015084542021, i64 8888375603305401356, i64 3233681622133423355, i64 -7192219100985789586, i64 -1711026644753171177, i64 7986497647124271633, i64 -9162889623727551392, i64 -2140064064486589519, i64 -72462591941618778, i64 5758989804794118256, i64 6307771306344648075, i64 3992384301585723137, i64 -4579659814254255371, i64 3191326776560555553, i64 -4955821830106777449, i64 -3577900387492227119, i64 4166753728611873902, i64 7816027843627899646, i64 -1016871682926274557, i64 -4133971194013434939, i64 -6157217186204058877, i64 7580010916216904559, i64 2563004972560429939, i64 3726246653994877828, i64 -7299366949903099212, i64 -8530428084666136122, i64 465499816220845941, i64 345593301633374614, i64 5529487392045664486, i64 4867180837457093811, i64 4068772350559667070, i64 6540552018154201167, i64 -855960095129558771, i64 -2119527268291343783, i64 -7722071188983516346, i64 6515912729355005238, i64 -8592938945906276019, i64 2285849035137277295, i64 -2040627206715053302, i64 -3659626707506111008, i64 -7546263428386013151, i64 -6953041733551484714, i64 -7858693406264787260, i64 -5247262118907848624, i64 -3848191668999025658, i64 -1475576436938814635, i64 7452404446075385407, i64 1805265112142225930, i64 -4067208480384294058, i64 -6746804343080498087, i64 4388274825138569967, i64 -2905110337171157654, i64 8139866943620997520, i64 6334062748603777535, i64 5828425224018882504, i64 2107644762752778456, i64 2657581434606941175, i64 1805225529723626893, i64 2320404269447759382, i64 -9162600211062877908, i64 5109732623733009057, i64 2137072148022321185, i64 237639104555937092, i64 -1195101974730984354, i64 8984934250717371929, i64 2929526545647411858, i64 6715972036522542850, i64 -2791762611622315645, i64 -480957445492936651, i64 -6870902801416788029, i64 -583570228636414458, i64 5804227295968216975, i64 -3747794872542026524, i64 -5388133353777823422, i64 5605645202268157916, i64 -9063070649875726065, i64 1237615066877963423, i64 -4459326268971666139, i64 -6449132595575236903, i64 -3890704427016283581, i64 -6918944650193530945, i64 3348050079832597499, i64 7951361015274329954, i64 -2812821091874941642, i64 8692252455349501942, i64 1038818314611527381, i64 -7140700513788999400, i64 -1535206495430846864, i64 6791899315106596917, i64 -9169979911963818216, i64 -8069353417707694065, i64 -8781659001547832137, i64 -8920419728108462302, i64 1880645918252833786, i64 1232870677444621200, i64 -6829467098783679977, i64 -1361202261247881964, i64 2807636373246980374, i64 5125780116609340238, i64 -2021581890363046494, i64 4359290853292764619, i64 1937435304089611254, i64 7237786738354669217, i64 -4014955296135257327, i64 -4652332642288346695, i64 -864985844858053442, i64 -6136041069870321181, i64 4579004620520441532, i64 -473016669032593885, i64 -993648544174562304, i64 -1218734390059491587, i64 -8579794159385215458, i64 612120941739186834, i64 -2022687384637565278, i64 -1897717962520012103, i64 -1533964881250629418, i64 -2859093066636462082, i64 -8678178336970417256, i64 7301995777980340602, i64 3663752046615221534, i64 -8177683136219763908, i64 1584922563774094483, i64 1249416606654527200, i64 -6457838085328556637, i64 -2597099397914049407, i64 4827172311581881067, i64 3624378858397266584, i64 4984778156015299940, i64 4731785937823424291, i64 -2720596008525920707, i64 -1992765562848489116, i64 -5526163784444893040, i64 9203806444014270935, i64 495368621624636412, i64 4988096266790931782, i64 -8955146177940060422, i64 5932430689583971759, i64 -2580711045162203357, i64 -881320842264459085, i64 1263438954056446160, i64 6464494294820396870, i64 -6276775381434719647, i64 -8844475501250508639, i64 275590997920983716, i64 7582339539297007823, i64 2011091496377405048, i64 1371837246476702001, i64 1769417401625873531, i64 732725146066822029, i64 -338490899384701827, i64 2688638027298009694, i64 8938647325484760647, i64 1926629853599110169, i64 -656386810680322503, i64 -332875018218852159, i64 -7716737321811168109, i64 -8039039636792943609, i64 3250064071395792527, i64 4213153249164667087, i64 -2932845007927407546, i64 6139486620149332523, i64 136225033110974878, i64 3202680941493996458, i64 -1896416517632244664, i64 -990914262909438321, i64 -153198080784501891, i64 -882150311892368725, i64 896838124685288808, i64 -6972205853015759801, i64 -7879288049435015706, i64 482457124204555310, i64 6087736242597191848, i64 7729609424929288835, i64 -5742820299932216691, i64 -4182627843739937611, i64 2724729020633876816, i64 -2364801813357073859, i64 -2560042046687734147, i64 782714364461450621, i64 -3347934109461429575, i64 2629523051186050642, i64 2566691031451773393, i64 -5672122455388299065, i64 -7730236517300844521, i64 -8385451434137527630, i64 6024998005513863496, i64 -3871507563043420476, i64 -5024418529462861283, i64 -874265762735551121, i64 691969877349949939, i64 -1987546594532352739, i64 -6817946249371448672, i64 -1669474166389647540, i64 7526826771237885158, i64 6839022574016607473, i64 -582719292949747146, i64 6660477880205299655, i64 7532990983597043767, i64 6515172617670652176, i64 264676661561414995, i64 -8324569709126632264, i64 7881238686339967700, i64 4689012971924454541, i64 -218994771289272251, i64 1218343010219096858, i64 2951209281750830716, i64 1425274888486101396, i64 -4418771741280194948, i64 -8979230737715337165, i64 1793164789562041190, i64 9094403857818893790, i64 4067019834735450239, i64 9023758754083358749, i64 4397538359820861197, i64 -5369916628035564988, i64 1034263827032819876, i64 3197284248146242061, i64 -613309924507601542, i64 -1364946636005498186, i64 2907845095428029830, i64 -7859405539300166470, i64 -1709245501878581072, i64 6130152665680931246, i64 879767934428193624, i64 -8624393693638975387, i64 3123166438586384613, i64 -6184301304868663965, i64 7758062729124040491, i64 7516659803419517645, i64 -4761504467850892082, i64 3287220179453433516, i64 1156239531573122214, i64 -5207978386408476820, i64 -7581926821989540088, i64 4923033773540037270, i64 -2095657962816703358, i64 2920645711231600431, i64 -5646964217664706480, i64 -5024151256098298352, i64 609168275409456322, i64 4215330154126155656, i64 1932862987246371196, i64 -1090560952292502791, i64 -951690252623019379, i64 420266537208897099, i64 4572553395073536355, i64 2144777691235019218, i64 5172617367537492150, i64 5688007021535597047, i64 -8370326167144433296, i64 801927516963056428, i64 8481978926609419555, i64 -6777295918481091566, i64 4184599428158996793, i64 -457311881863403726, i64 4328186981972606095, i64 -5971330297518823381, i64 -8779848774833965815, i64 -4033088441212714196, i64 7973857981969421217, i64 -8183418025745413476, i64 -3559008186718585718, i64 -3128666067797746880, i64 -5465940715574589496, i64 -6564723493312280025, i64 -3429554236862548667, i64 -7572114735221171964, i64 -4129512272356894299, i64 4217895350274283624, i64 5009239369767855030, i64 -5275046056372245765, i64 983396747428607271, i64 -7545034919619987526, i64 2243036802283488318, i64 -7850688083178014922, i64 -2439051842844504505, i64 9160616651104524391, i64 -3309204168096805662, i64 -3138581717595732317, i64 7614132418369609072, i64 5149235094729232107, i64 -6219755332358190064, i64 8789651178809530979, i64 -7653255678409841593, i64 -7585636299500411589, i64 -8328481329809406404, i64 -5915929198940853268, i64 5245202651468237428, i64 2454517582308045323, i64 -4992068081538587350, i64 -7893410817644537198, i64 1414898708618107936, i64 7394663319341101130, i64 3063105961525517867, i64 5081902780737728940, i64 -496252810592439865, i64 -1764619841041297284, i64 1409855492405622658, i64 2832340426706838970, i64 616740071777291166, i64 -8018486981012459505, i64 647064813510484821, i64 4117943058104321297, i64 -7007424680445687453, i64 9205207130845683202, i64 7742491132163761926, i64 -1302059819565978652, i64 -6353484488962985873, i64 1671147206735095299, i64 7639814945868606274, i64 -5984384881073105643, i64 -4438601852126681087, i64 -1590065373301833096, i64 -200752844589549943, i64 737539777525667726, i64 6483754814947979393, i64 2606732178647716360, i64 4724237645303941090, i64 1189469807200288016, i64 -703683672815927031, i64 495952942031036920, i64 -6844709501662345516, i64 9207006768607123798, i64 1922129208269836524, i64 2544649729992835833, i64 -1377046900774233629, i64 -239572794883621475, i64 -7227713540378585579, i64 881513961201218856, i64 1872335774895500032, i64 8887315332721551175, i64 6309056003164747008, i64 359977834173615126, i64 -6184400502913877538, i64 -4917746656967594095, i64 3673511893337312958, i64 -1556225212929496020, i64 -9215788716163529073, i64 -3623410197640290441, i64 3856505004211963012, i64 -3249751804001912334, i64 -7869400010860137520, i64 -303902947431352166, i64 5186326160029370363, i64 8759644273822217593, i64 -6375696842109648766, i64 7396639708009067552, i64 5190477245210799307, i64 1374112579153745357, i64 1175131360645598994, i64 -6456335477606438580, i64 7570540473737301641, i64 -5051870573873034510, i64 -9143477080982198342, i64 -5700393284387784440, i64 1451853468255019130, i64 -6295746606794518376, i64 -9074080394382815992, i64 4920766581833244954, i64 5494502495748948776, i64 -7849100405006850484, i64 -5583327837925522231, i64 473973028109747234, i64 -1030275818017298607, i64 8291805835848831653, i64 2453660699254764307, i64 7273483261824304434, i64 1919890678074370565, i64 1154931552834846393, i64 3450609168858760081, i64 2748109794215238323, i64 -554136407637177486, i64 -4755736894738449794, i64 5278850049367994653, i64 -5278772358085294600, i64 8956922375813434562, i64 -5060415934826507682, i64 1431878795857375867, i64 -2286890766005134305, i64 -7016829587386207357, i64 -1093050613646557657, i64 -8203785847251630852, i64 -1111684886663215068, i64 -4504725674785250048, i64 -4829940396336979446, i64 -2609216107154333436, i64 -5350731339202805377, i64 6782285683831118540, i64 -8595312060832091910, i64 914015436628284745, i64 -123481161442580785, i64 5848595579021704312, i64 -8126673847757025954, i64 5768839490863091500, i64 -6642281854449869250, i64 -266485040094815954, i64 7730687302523036705, i64 7502159149497951384, i64 -5361959972421360467, i64 7262926529930478114, i64 5564989841201111900, i64 4909117791384981233, i64 -8398376649897712797, i64 -2339148541934033507, i64 2383242865869866430, i64 -1830118755845134416, i64 -3518267004861128011, i64 -6001953932029801660, i64 -8237236752875722447, i64 -6839081628771674982, i64 -1186243966925436346, i64 5938337776896131801, i64 5923649932827917992, i64 -9102552966490146900, i64 90235641228402778, i64 4784600221756482035, i64 3292189087512201625, i64 -5558440522744785766, i64 4356897485386200302, i64 7360854216266217893, i64 4909691532993750331, i64 1136886513612938709, i64 -1180871347950225185, i64 -435786542515337010, i64 8714212269414170889, i64 -2747824887557309498, i64 -8142615621658847947, i64 4323025664407224295, i64 -6166708243114063833, i64 7224920234027762157, i64 -8052655780841621799, i64 6313570491176953069, i64 5544928224901657480, i64 -6435440177112283074, i64 1229564524899931613, i64 -1991288044016921114, i64 234116328419421311, i64 -9188384002847755959, i64 -4037124962385063806, i64 -5547472583637335581, i64 4355943033451868778, i64 -282211513682888399, i64 535240084430802867, i64 2704201490892975047, i64 6175365398363869039, i64 6702953072729317262, i64 -8573895973141605546, i64 -8126153792894994849, i64 -6327765789619955093, i64 762374597082734473, i64 2488028469164491859, i64 -2045477612197516688, i64 773759781442175685, i64 4606932448553331132, i64 7129868563386958877, i64 1314786700156837874, i64 -1549070317387377282, i64 2572954631416534862, i64 -644208495044146223, i64 -4974345566688540793, i64 4274540635713514654, i64 -2461217208698341152, i64 -366974484393506857, i64 -8515969751193306966, i64 3564930777076479622, i64 4415564971521390383, i64 3865928619297231310, i64 -169034592091725871, i64 2710364586008078175, i64 8828192250072346798, i64 8719872216492485923, i64 7852573981754451165, i64 -1127320708971771021, i64 1503469259204146521, i64 4727485662231371132, i64 5571370722040519308, i64 -1260708297585665772, i64 3836916836447856288, i64 7784747731608669169, i64 7353686438824759577, i64 4765801250557677893, i64 -5167640066802518519, i64 -3611304857877397060, i64 -4111319986509724519, i64 -3987917739485919537, i64 -6761539642989268283, i64 -722521515218444018, i64 6557521303462963694, i64 -5604560377596893832, i64 -5390520631354321232, i64 7659450536767587785, i64 -564225977423120159, i64 -7607956075920509704, i64 -1127382022678936978, i64 1701072224325739594, i64 -2468048668546314828, i64 -2619636897534337585, i64 6692080011683470824, i64 -6306769380109382572, i64 7690950414471441325, i64 2278811438049347209, i64 -5502978578923670471, i64 -9096458099834202476, i64 -2063277558705912498, i64 -4407591360304948316, i64 -3989814447966110522, i64 396382313383525415, i64 -7065906312603678852, i64 -3434806034431068458, i64 4314368653087326056, i64 5804921885386185633, i64 -4571721354136138367, i64 -2700480350476766301, i64 -7288575021351742848, i64 -2510882548899485625, i64 -5668566273706574380, i64 5918225616504112510, i64 -1959470540430119291, i64 4528020704581371184, i64 -869056144161503934, i64 8547272234636646375, i64 -16082375477575921, i64 -3637308508942287940, i64 8256491367544030333, i64 161266089436911892, i64 8871015283503408329, i64 1873233633861095509, i64 -7036674609743088752, i64 1554147339066013909, i64 -8487154249820166364, i64 8067511149962650278, i64 5075603036143240632, i64 7604223360618080337, i64 -2203585541017327895, i64 -2595363493564234225, i64 3076717996012112089, i64 4325576069518034348, i64 -1629242610057247679, i64 8771776176208709639, i64 -9036360518549805608, i64 3044257524326893415, i64 649591295302855196, i64 -4039617882443059158, i64 450189349703891847, i64 6328256960623539476, i64 3505380706795627760, i64 5803245757403222774, i64 -766234745906495231, i64 4198898441896201844, i64 -7617025690547555159, i64 6939137362611743872, i64 -1045648287220361672, i64 818631421479334080, i64 3360499193559259935, i64 1346255281576687726, i64 7572244335244064390, i64 -2629759913963464684, i64 -9136616088541004841, i64 -7149227336758565711, i64 -9113286788900819514, i64 3522483831586210697, i64 -7416580235476423632, i64 3253294774304389327, i64 -1096843728297820372, i64 8249360919881346071, i64 -3601228580652640431, i64 4062363032736051260, i64 -9135751321121061155, i64 1095361197253594931, i64 -883576902118161844, i64 1149743663379404377, i64 -3269359022454033200, i64 2177290942858271266, i64 1083993346533887755, i64 4346553093841524490, i64 -5288793011537459782, i64 -7318686778691033640, i64 5941576045655866164, i64 2389913317785690729, i64 4209655796525042038, i64 1663630197032007867, i64 6726855451718360886, i64 -5046613135450014900, i64 -3289355183970726765, i64 -3165057842933369036, i64 -6958834155128636938, i64 -3330835532783398853, i64 7948434369798698169, i64 -4393794123908838376, i64 3486271230737220365, i64 458107174131258200, i64 -2098767797213323869, i64 -7485074110781425866, i64 -5533843807177341963, i64 -2999258043694817154, i64 -2869391521784345553, i64 -769018838612420940, i64 4353624741890269058, i64 -3086923823895037089, i64 -7697350289907870301, i64 5362266081082321476, i64 -1374120299775355951, i64 4300463407417087404, i64 4850322401300122943, i64 -5143219569570732057, i64 -3675806554469116941, i64 -6705638032078417573, i64 274183146335957607, i64 273161936921721283, i64 850643898152430787, i64 -7825634116158126653, i64 8794752142245979617, i64 -5675527233125872324, i64 -3186388755527020770, i64 -5200263630950250592, i64 -2883817104116568107, i64 -5379454263969821809, i64 4748952631633461732, i64 8785069763882772395, i64 8841725480816732880, i64 8461292924447667294, i64 -5750149366720049746, i64 -2875074743995333013, i64 -1386161449063617691, i64 -5910011679325978214, i64 5108458522640084118, i64 8560498497296134681, i64 -1057642542159245251, i64 -5770418500653621319, i64 7354147754309855920, i64 -7360684728522105226, i64 4596711687093911445, i64 8759905780732683791, i64 -225180690272161089, i64 3088174238071424504, i64 4573829815031814445, i64 -175532659533418704, i64 -8570091009831322432, i64 4381505033514059745, i64 913063117934717098, i64 -8709637126133053930, i64 6755951781953348951, i64 1090283459177055176, i64 211122868944519918, i64 -6736987828379490396, i64 -3985476362478999098, i64 -5135716024268851529, i64 -994890377264089400, i64 -1369999434940390688, i64 -1685514040743590965, i64 8189400215094479120, i64 3324542369633192845, i64 5061513056867082592, i64 5319764248191741774, i64 -1358077801215222917, i64 -2823813532193856403, i64 -8686578464302058340, i64 -6544127667646171576, i64 -4681606399346713279, i64 3876225034367526692, i64 -6646241949832213956, i64 4387227097149411128, i64 -3958407463780120004, i64 215491347762920501, i64 -8833791435673434714, i64 1670358127892632129, i64 -2168592654566549883, i64 -1792962093259448719, i64 -2152973421237437924, i64 -3406286298585939181, i64 7499172674564395365, i64 -6381834538421304955, i64 7580305191116255052, i64 -679153008666433601, i64 4658624852677413499, i64 914231619116577945, i64 824763024087877125, i64 1523026794559838296, i64 7548478506484912499, i64 910187847226589992, i64 4847575937281246022, i64 759445962178515777, i64 -4319618088167764569, i64 6575897779812594961, i64 2507632330170263789, i64 -7996303281005248037, i64 2454115644483969261, i64 -8843649055748069469, i64 6418041547689182675, i64 4087640515653539234, i64 -4473102402046157371, i64 -5136629856250679763, i64 -4241244139838935540, i64 6027278025033829222, i64 7074551457589541507, i64 -332624048739210329, i64 -3851746793005362735, i64 917334673020125994, i64 2753685554651035150, i64 7790129306542115400, i64 -5322412870984079542, i64 -8043070170097335201, i64 -5663907282872496444, i64 4256009230404729541, i64 5379328118501106188, i64 7800661895855653912, i64 8512581412623652684, i64 -8983900279763188648, i64 -6240332858791850300, i64 8367938388403000727, i64 -7681050996640938980, i64 -3421740290102458312, i64 2514030540996828520, i64 734749442843960436, i64 8812230515084240003, i64 7596499850922227269, i64 7285235457062560148, i64 -5325143217817988392, i64 -4552338343519909324, i64 6714058610568765044, i64 -4427998656990374256, i64 -8602821109902904032, i64 -5691586022143034386, i64 4926610656292151810, i64 2022533330528623305, i64 7123298380797726530, i64 5050107675604380453, i64 7433267710473592790, i64 6683787218307829828, i64 2986738473862968970, i64 982511922969469146, i64 3336281295039152870, i64 2135654902620301164, i64 3094162207480152326, i64 1357321152843958754, i64 -9107082843830155163, i64 -1944665977816861614, i64 -7894940797977206552, i64 -215510787412973208, i64 -5688993890236274480, i64 -215507466676962674, i64 5797960663582941412, i64 -3157686388866921722, i64 -2131167620034706575, i64 8960135106972206166, i64 4199834699416259152, i64 6708474619433864099, i64 2886246158437228706, i64 830764880047647873, i64 6968629311144080513, i64 8160702337965016149, i64 4080337974851414036, i64 6524945740229710858, i64 7444161043413023117, i64 -7671452670003450120, i64 -8518957226149967418, i64 -4408266227614607723, i64 -5966572962817322315, i64 -969062292056554282, i64 8790784373795225544, i64 -4827979849963454492, i64 -8687656155280334864, i64 2916343772719758555, i64 -8952370885630027318, i64 4058412540879207888, i64 -8642036843897985532, i64 -457194079480756412, i64 -7796141008600925395, i64 1720064738856230490, i64 -4354988324337060278, i64 -6609507238503231490, i64 -5932444213066250102, i64 274590663679434005, i64 -2268751169726301393, i64 4271485817337843010, i64 2005218645781519805, i64 8637059221170539704, i64 502006076629839343, i64 -1768844966458655522, i64 -8089063288448354373, i64 -913216433755150733, i64 -937912037351915937, i64 -1131378113895597626, i64 -7315407407985664017, i64 3579911179325685982, i64 501906171962549281, i64 6796267937817927790, i64 1246657075329041272, i64 -2989418999741761740, i64 7220178595788772385, i64 -1991755638003462752, i64 -982743938300523325, i64 -4809845991064867322, i64 15084813], align 8
@df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892 = linkonce_odr hidden unnamed_addr constant [2 x i64] [i64 0, i64 0], align 8

define i64 @df.lengths$2edart..observe(i64 %v0) noinline "disable-tail-calls"="true" nounwind {
b0:
  %v4 = add i64 0, 3
  %v5 = mul i64 %v0, %v4
  %v6 = add i64 0, 7
  %v7 = xor i64 %v5, %v6
  ret i64 %v7
}

define i64 @df.lengths$2edart..stable(i64 %v0) noinline "disable-tail-calls"="true" nounwind {
b0:
  %ctx = call ptr @df.context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load i64, ptr %pilhalp, align 8
  %pilhasp = call i64 @llvm.read_register.i64(metadata !{!"sp"})
  %pilhab = icmp ult i64 %pilhasp, %pilhal
  %pilhax = call i1 @llvm.expect.i1(i1 %pilhab, i1 false)
  br i1 %pilhax, label %pilha.estouro, label %pilha.ok
pilha.estouro:
  call void @dartforge_stack_overflow()
  ret i64 0
pilha.ok:
  %v3 = add i64 0, 0
  %v6 = add i64 0, 0
  %v11 = call i64 @df.string_len(i64 %v0)
  br label %b1
b1:
  %v33 = phi i64 [ %v3, %pilha.ok ], [ %v26, %b6 ]
  %v34 = phi i64 [ %v6, %pilha.ok ], [ %v29, %b6 ]
  %v12 = icmp slt i64 %v34, %v11
  br i1 %v12, label %b2, label %b4
b2:
  %ip13 = getelementptr inbounds i8, ptr %ctx, i64 32
  %iv13 = load atomic i8, ptr %ip13 monotonic, align 8
  %v13 = call i8 @llvm.expect.i8(i8 %iv13, i8 0)
  %c0 = zext i8 %v13 to i64
  %v14 = icmp ne i64 %c0, 0
  br i1 %v14, label %b5, label %b6
b4:
  %xcp32 = load i8, ptr %ctx, align 8
  %xcn32 = icmp ne i8 %xcp32, 0
  %xce32 = call i1 @llvm.expect.i1(i1 %xcn32, i1 false)
  br i1 %xce32, label %xc32.limpar, label %xc32.fim
xc32.limpar:
  call void @dartforge_exception_clear()
  br label %xc32.fim
xc32.fim:
  ret i64 %v33
b5:
  call void @dartforge_safepoint()
  %v16 = load i8, ptr %ctx, align 8
  %c1 = zext i8 %v16 to i64
  %v17 = icmp ne i64 %c1, 0
  br i1 %v17, label %b8, label %b7
b6:
  %v20 = call i64 @df.lengths$2edart..observe(i64 %v34)
  %v25 = add i64 %v20, %v11
  %v26 = add i64 %v33, %v25
  %v29 = add i64 %v34, 1
  br label %b1
b7:
  br label %b6
b8:
  ret i64 0
}

define i64 @df.lengths$2edart..changing(i64 %v0, i64 %v3) noinline "disable-tail-calls"="true" nounwind {
b0:
  %gcq = alloca { ptr, i64, [2 x i64] }, align 8
  %ctx = call ptr @df.context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load i64, ptr %pilhalp, align 8
  %pilhasp = call i64 @llvm.read_register.i64(metadata !{!"sp"})
  %pilhab = icmp ult i64 %pilhasp, %pilhal
  %pilhax = call i1 @llvm.expect.i1(i1 %pilhab, i1 false)
  br i1 %pilhax, label %pilha.estouro, label %pilha.ok
pilha.estouro:
  call void @dartforge_stack_overflow()
  ret i64 0
pilha.ok:
  store { ptr, i64, [2 x i64] } { ptr null, i64 4611686018427387906, [2 x i64] zeroinitializer }, ptr %gcq
  %gcs0 = getelementptr inbounds { ptr, i64, [2 x i64] }, ptr %gcq, i64 0, i32 2, i64 0
  %gcs1 = getelementptr inbounds { ptr, i64, [2 x i64] }, ptr %gcq, i64 0, i32 2, i64 1
  %gcant = load ptr, ptr %ctxtopo, align 8
  store ptr %gcant, ptr %gcq, align 8
  store ptr %gcq, ptr %ctxtopo, align 8
  store i64 %v0, ptr %gcs0
  store i64 %v3, ptr %gcs1
  %v9 = add i64 0, 0
  %v12 = add i64 0, 0
  br label %b1
b1:
  %v46 = phi i64 [ %v0, %pilha.ok ], [ %v39, %b12 ]
  %v47 = phi i64 [ %v9, %pilha.ok ], [ %v42, %b12 ]
  %v48 = phi i64 [ %v12, %pilha.ok ], [ %v32, %b12 ]
  %v17 = call i64 @df.string_len(i64 %v46)
  %v18 = icmp slt i64 %v47, %v17
  br i1 %v18, label %b2, label %b3
b2:
  %ip19 = getelementptr inbounds i8, ptr %ctx, i64 32
  %iv19 = load atomic i8, ptr %ip19 monotonic, align 8
  %v19 = call i8 @llvm.expect.i8(i8 %iv19, i8 0)
  %c0 = zext i8 %v19 to i64
  %v20 = icmp ne i64 %c0, 0
  br i1 %v20, label %b4, label %b5
b3:
  %xcp45 = load i8, ptr %ctx, align 8
  %xcn45 = icmp ne i8 %xcp45, 0
  %xce45 = call i1 @llvm.expect.i1(i1 %xcn45, i1 false)
  br i1 %xce45, label %xc45.limpar, label %xc45.fim
xc45.limpar:
  call void @dartforge_exception_clear()
  br label %xc45.fim
xc45.fim:
  %gcvolta3 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta3, ptr %ctxtopo, align 8
  ret i64 %v48
b4:
  call void @dartforge_safepoint()
  %v22 = load i8, ptr %ctx, align 8
  %c1 = zext i8 %v22 to i64
  %v23 = icmp ne i64 %c1, 0
  br i1 %v23, label %b7, label %b6
b5:
  %v26 = call i64 @df.lengths$2edart..observe(i64 %v47)
  %v31 = add i64 %v26, %v17
  %v32 = add i64 %v48, %v31
  %v35 = and i64 %v47, 1
  %v36 = icmp eq i64 %v35, 0
  br i1 %v36, label %b10, label %b11
b6:
  br label %b5
b7:
  %gcvolta7 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta7, ptr %ctxtopo, align 8
  ret i64 0
b10:
  br label %b12
b11:
  br label %b12
b12:
  %v39 = phi i64 [ %v3, %b10 ], [ %v0, %b11 ]
  %v42 = add i64 %v47, 1
  br label %b1
}

define i64 @df.lengths$2edart..nullable(i64 %v0, i1 %v3) noinline "disable-tail-calls"="true" nounwind {
b0:
  %gcq = alloca { ptr, i64, [1 x i64] }, align 8
  %ctx = call ptr @df.context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load i64, ptr %pilhalp, align 8
  %pilhasp = call i64 @llvm.read_register.i64(metadata !{!"sp"})
  %pilhab = icmp ult i64 %pilhasp, %pilhal
  %pilhax = call i1 @llvm.expect.i1(i1 %pilhab, i1 false)
  br i1 %pilhax, label %pilha.estouro, label %pilha.ok
pilha.estouro:
  call void @dartforge_stack_overflow()
  ret i64 0
pilha.ok:
  store { ptr, i64, [1 x i64] } { ptr null, i64 4611686018427387905, [1 x i64] zeroinitializer }, ptr %gcq
  %gcs0 = getelementptr inbounds { ptr, i64, [1 x i64] }, ptr %gcq, i64 0, i32 2, i64 0
  %gcant = load ptr, ptr %ctxtopo, align 8
  store ptr %gcant, ptr %gcq, align 8
  store ptr %gcq, ptr %ctxtopo, align 8
  store i64 %v0, ptr %gcs0
  %v7 = xor i1 %v3, true
  br i1 %v7, label %b1, label %b3
b1:
  %v8 = add i64 0, 123
  %xcp9 = load i8, ptr %ctx, align 8
  %xcn9 = icmp ne i8 %xcp9, 0
  %xce9 = call i1 @llvm.expect.i1(i1 %xcn9, i1 false)
  br i1 %xce9, label %xc9.limpar, label %xc9.fim
xc9.limpar:
  call void @dartforge_exception_clear()
  br label %xc9.fim
xc9.fim:
  %gcvolta1 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1, ptr %ctxtopo, align 8
  ret i64 %v8
b3:
  %v11 = icmp eq i64 %v0, 0
  br i1 %v11, label %b5, label %b6
b5:
  store i64 0, ptr %gcs0
  %v12 = call i64 @dartforge_null_check_error_new()
  store i64 %v12, ptr %gcs0
  call void @dartforge_exception_throw(i64 %v12, i8 3)
  %gcvolta5 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta5, ptr %ctxtopo, align 8
  ret i64 0
b6:
  %v14 = call i64 @df.lengths$2edart..stable(i64 %v0)
  %v15 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v15 to i64
  %v16 = icmp ne i64 %c0, 0
  br i1 %v16, label %b9, label %b8
b8:
  %xcp17 = load i8, ptr %ctx, align 8
  %xcn17 = icmp ne i8 %xcp17, 0
  %xce17 = call i1 @llvm.expect.i1(i1 %xcn17, i1 false)
  br i1 %xce17, label %xc17.limpar, label %xc17.fim
xc17.limpar:
  call void @dartforge_exception_clear()
  br label %xc17.fim
xc17.fim:
  %gcvolta8 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta8, ptr %ctxtopo, align 8
  ret i64 %v14
b9:
  %gcvolta9 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta9, ptr %ctxtopo, align 8
  ret i64 0
}

define void @dart_main() "disable-tail-calls"="true" nounwind {
b0:
  %gcq = alloca { ptr, i64, [2 x i64] }, align 8
  %area = call ptr @df.get_area()
  %ctx = call ptr @df.context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load i64, ptr %pilhalp, align 8
  %pilhasp = call i64 @llvm.read_register.i64(metadata !{!"sp"})
  %pilhab = icmp ult i64 %pilhasp, %pilhal
  %pilhax = call i1 @llvm.expect.i1(i1 %pilhab, i1 false)
  br i1 %pilhax, label %pilha.estouro, label %pilha.ok
pilha.estouro:
  call void @dartforge_stack_overflow()
  ret void
pilha.ok:
  store { ptr, i64, [2 x i64] } { ptr null, i64 4611686018427387906, [2 x i64] zeroinitializer }, ptr %gcq
  %gcs0 = getelementptr inbounds { ptr, i64, [2 x i64] }, ptr %gcq, i64 0, i32 2, i64 0
  %gcs1 = getelementptr inbounds { ptr, i64, [2 x i64] }, ptr %gcq, i64 0, i32 2, i64 1
  %gcant = load ptr, ptr %ctxtopo, align 8
  store ptr %gcant, ptr %gcq, align 8
  store ptr %gcq, ptr %ctxtopo, align 8
  %v0 = add i64 0, 0
  %v3 = add i64 0, 0
  br label %b1
b1:
  %v107 = phi i64 [ %v0, %pilha.ok ], [ %v97, %b35 ]
  %v108 = phi i64 [ %v3, %pilha.ok ], [ %v100, %b35 ]
  %v7 = add i64 0, 20
  %v8 = icmp slt i64 %v108, %v7
  br i1 %v8, label %b2, label %b4
b2:
  store i64 0, ptr %gcs1
  %v10 = add i64 0, 0
  store i64 %v10, ptr %gcs0
  %v11 = call i64 @df.allocate(i64 47244902401, i64 4)
  %bp11 = inttoptr i64 %v11 to ptr
  %bg11_k = getelementptr inbounds i8, ptr %bp11, i64 14
  store i64 ptrtoint (ptr @dart_main$clo0$e3e298bca$ent to i64), ptr %bg11_k, align 8
  %bg11_e = getelementptr inbounds i8, ptr %bp11, i64 22
  store i64 %v10, ptr %bg11_e, align 8
  %bg11_t = getelementptr inbounds i8, ptr %bp11, i64 30
  store i64 ptrtoint (ptr @dart_main$clo0$e3e298bca to i64), ptr %bg11_t, align 8
  %bg11_a = getelementptr inbounds i8, ptr %bp11, i64 38
  store i64 37, ptr %bg11_a, align 8
  %bm11 = getelementptr inbounds i8, ptr %bp11, i64 6
  store i32 2, ptr %bm11, align 4
  store i64 %v11, ptr %gcs0
  %ga12 = getelementptr i64, ptr %area, i64 1
  %v12 = load i8, ptr %ga12
  %c0 = zext i8 %v12 to i64
  %v13 = icmp ne i64 %c0, 0
  br i1 %v13, label %b5, label %b6
b4:
  store i64 0, ptr %gcs1
  %v103 = call i64 @df.box_int(i64 %v107)
  store i64 %v103, ptr %gcs0
  call void @df.dart$3acore..print(i64 %v103)
  %v105 = load i8, ptr %ctx, align 8
  %c1 = zext i8 %v105 to i64
  %v106 = icmp ne i64 %c1, 0
  br i1 %v106, label %b38, label %b37
b5:
  %ga14 = getelementptr i64, ptr %area, i64 0
  %v14 = load i64, ptr %ga14
  br label %b7
b6:
  %v15 = call i64 @df.rti.0a9e544d725576ee.lengths$2edart()
  br label %b7
b7:
  %v16 = phi i64 [ %v14, %b5 ], [ %v15, %b6 ]
  call void @dartforge_rti_set(i64 %v11, i64 %v16)
  %ga19 = getelementptr i64, ptr %area, i64 3
  %v19 = load i8, ptr %ga19
  %c2 = zext i8 %v19 to i64
  %v20 = icmp ne i64 %c2, 0
  br i1 %v20, label %b8, label %b9
b8:
  %ga21 = getelementptr i64, ptr %area, i64 2
  %v21 = load i64, ptr %ga21
  br label %b10
b9:
  %v22 = call i64 @df.rti.1dee75ed5d853ef3.lengths$2edart()
  br label %b10
b10:
  %v23 = phi i64 [ %v21, %b8 ], [ %v22, %b9 ]
  %v24 = call i64 @df.dart$3acore.List.new$3agenerate(i64 %v108, i64 %v11, i1 true, i64 %v23)
  store i64 %v24, ptr %gcs0
  %v25 = load i8, ptr %ctx, align 8
  %c3 = zext i8 %v25 to i64
  %v26 = icmp ne i64 %c3, 0
  br i1 %v26, label %b12, label %b11
b11:
  %v27 = add i64 0, 0
  %v28 = call i64 @df.dart$3acore.String.new$3afromCharCodes(i64 %v24, i64 %v27, i64 0)
  store i64 %v28, ptr %gcs0
  %v29 = load i8, ptr %ctx, align 8
  %c4 = zext i8 %v29 to i64
  %v30 = icmp ne i64 %c4, 0
  br i1 %v30, label %b14, label %b13
b12:
  %gcvolta12 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta12, ptr %ctxtopo, align 8
  ret void
b13:
  %v34 = add i64 0, 1
  %v35 = add i64 %v108, %v34
  %v36 = add i64 0, 0
  store i64 %v36, ptr %gcs1
  %v37 = call i64 @df.allocate(i64 47244902401, i64 4)
  %bp37 = inttoptr i64 %v37 to ptr
  %bg37_k = getelementptr inbounds i8, ptr %bp37, i64 14
  store i64 ptrtoint (ptr @dart_main$clo1$e3e298bca$ent to i64), ptr %bg37_k, align 8
  %bg37_e = getelementptr inbounds i8, ptr %bp37, i64 22
  store i64 %v36, ptr %bg37_e, align 8
  %bg37_t = getelementptr inbounds i8, ptr %bp37, i64 30
  store i64 ptrtoint (ptr @dart_main$clo1$e3e298bca to i64), ptr %bg37_t, align 8
  %bg37_a = getelementptr inbounds i8, ptr %bp37, i64 38
  store i64 37, ptr %bg37_a, align 8
  %bm37 = getelementptr inbounds i8, ptr %bp37, i64 6
  store i32 2, ptr %bm37, align 4
  store i64 %v37, ptr %gcs1
  %ga38 = getelementptr i64, ptr %area, i64 1
  %v38 = load i8, ptr %ga38
  %c5 = zext i8 %v38 to i64
  %v39 = icmp ne i64 %c5, 0
  br i1 %v39, label %b15, label %b16
b14:
  %gcvolta14 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta14, ptr %ctxtopo, align 8
  ret void
b15:
  %ga40 = getelementptr i64, ptr %area, i64 0
  %v40 = load i64, ptr %ga40
  br label %b17
b16:
  %v41 = call i64 @df.rti.0a9e544d725576ee.lengths$2edart()
  br label %b17
b17:
  %v42 = phi i64 [ %v40, %b15 ], [ %v41, %b16 ]
  call void @dartforge_rti_set(i64 %v37, i64 %v42)
  %ga45 = getelementptr i64, ptr %area, i64 3
  %v45 = load i8, ptr %ga45
  %c6 = zext i8 %v45 to i64
  %v46 = icmp ne i64 %c6, 0
  br i1 %v46, label %b18, label %b19
b18:
  %ga47 = getelementptr i64, ptr %area, i64 2
  %v47 = load i64, ptr %ga47
  br label %b20
b19:
  %v48 = call i64 @df.rti.1dee75ed5d853ef3.lengths$2edart()
  br label %b20
b20:
  %v49 = phi i64 [ %v47, %b18 ], [ %v48, %b19 ]
  %v50 = call i64 @df.dart$3acore.List.new$3agenerate(i64 %v35, i64 %v37, i1 true, i64 %v49)
  store i64 %v50, ptr %gcs1
  %v51 = load i8, ptr %ctx, align 8
  %c7 = zext i8 %v51 to i64
  %v52 = icmp ne i64 %c7, 0
  br i1 %v52, label %b22, label %b21
b21:
  %v53 = add i64 0, 0
  %v54 = call i64 @df.dart$3acore.String.new$3afromCharCodes(i64 %v50, i64 %v53, i64 0)
  store i64 %v54, ptr %gcs1
  %v55 = load i8, ptr %ctx, align 8
  %c8 = zext i8 %v55 to i64
  %v56 = icmp ne i64 %c8, 0
  br i1 %v56, label %b24, label %b23
b22:
  %gcvolta22 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta22, ptr %ctxtopo, align 8
  ret void
b23:
  %v61 = call i64 @df.lengths$2edart..stable(i64 %v28)
  %v62 = load i8, ptr %ctx, align 8
  %c9 = zext i8 %v62 to i64
  %v63 = icmp ne i64 %c9, 0
  br i1 %v63, label %b26, label %b25
b24:
  %gcvolta24 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta24, ptr %ctxtopo, align 8
  ret void
b25:
  %v65 = call i64 @df.lengths$2edart..stable(i64 %v54)
  %v66 = load i8, ptr %ctx, align 8
  %c10 = zext i8 %v66 to i64
  %v67 = icmp ne i64 %c10, 0
  br i1 %v67, label %b28, label %b27
b26:
  %gcvolta26 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta26, ptr %ctxtopo, align 8
  ret void
b27:
  %v68 = add i64 %v61, %v65
  %v69 = add i64 %v107, %v68
  %v74 = call i64 @df.lengths$2edart..changing(i64 %v28, i64 %v54)
  %v75 = load i8, ptr %ctx, align 8
  %c11 = zext i8 %v75 to i64
  %v76 = icmp ne i64 %c11, 0
  br i1 %v76, label %b30, label %b29
b28:
  %gcvolta28 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta28, ptr %ctxtopo, align 8
  ret void
b29:
  %v79 = call i64 @df.lengths$2edart..changing(i64 %v54, i64 %v28)
  %v80 = load i8, ptr %ctx, align 8
  %c12 = zext i8 %v80 to i64
  %v81 = icmp ne i64 %c12, 0
  br i1 %v81, label %b32, label %b31
b30:
  %gcvolta30 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta30, ptr %ctxtopo, align 8
  ret void
b31:
  %v82 = add i64 %v74, %v79
  %v83 = add i64 %v69, %v82
  store i64 0, ptr %gcs0
  %v88 = call i64 @df.lengths$2edart..nullable(i64 0, i1 false)
  %v89 = load i8, ptr %ctx, align 8
  %c13 = zext i8 %v89 to i64
  %v90 = icmp ne i64 %c13, 0
  br i1 %v90, label %b34, label %b33
b32:
  %gcvolta32 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta32, ptr %ctxtopo, align 8
  ret void
b33:
  %v93 = call i64 @df.lengths$2edart..nullable(i64 %v54, i1 true)
  %v94 = load i8, ptr %ctx, align 8
  %c14 = zext i8 %v94 to i64
  %v95 = icmp ne i64 %c14, 0
  br i1 %v95, label %b36, label %b35
b34:
  %gcvolta34 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta34, ptr %ctxtopo, align 8
  ret void
b35:
  %v96 = add i64 %v88, %v93
  %v97 = add i64 %v83, %v96
  %v100 = add i64 %v108, 1
  br label %b1
b36:
  %gcvolta36 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta36, ptr %ctxtopo, align 8
  ret void
b37:
  %gcvolta37 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta37, ptr %ctxtopo, align 8
  ret void
b38:
  %gcvolta38 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta38, ptr %ctxtopo, align 8
  ret void
}

define i64 @dart_main$clo0$e3e298bca(i64 %v0, i64 %v1) "disable-tail-calls"="true" nounwind {
b0:
  %v4 = add i64 0, 65
  %v6 = add i64 %v4, %v1
  ret i64 %v6
}

define i64 @dart_main$clo0$e3e298bca$ent(i64 %v0, ptr %v1, ptr %v2) "disable-tail-calls"="true" nounwind {
b0:
  %gcq = alloca { ptr, i64, [1 x i64] }, align 8
  %ctx = call ptr @df.context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load i64, ptr %pilhalp, align 8
  %pilhasp = call i64 @llvm.read_register.i64(metadata !{!"sp"})
  %pilhab = icmp ult i64 %pilhasp, %pilhal
  %pilhax = call i1 @llvm.expect.i1(i1 %pilhab, i1 false)
  br i1 %pilhax, label %pilha.estouro, label %pilha.ok
pilha.estouro:
  call void @dartforge_stack_overflow()
  ret i64 0
pilha.ok:
  store { ptr, i64, [1 x i64] } { ptr null, i64 4611686018427387905, [1 x i64] zeroinitializer }, ptr %gcq
  %gcs0 = getelementptr inbounds { ptr, i64, [1 x i64] }, ptr %gcq, i64 0, i32 2, i64 0
  %gcant = load ptr, ptr %ctxtopo, align 8
  store ptr %gcant, ptr %gcq, align 8
  store ptr %gcq, ptr %ctxtopo, align 8
  %fcb3 = call ptr @df.body(i64 %v0, ptr %ctx)
  %fp3 = getelementptr inbounds i8, ptr %fcb3, i64 16
  %fg3 = getelementptr inbounds i64, ptr %fp3, i64 1
  %v3 = load i64, ptr %fg3, align 8
  %li4 = getelementptr i64, ptr %v2, i64 0
  %v4 = load i64, ptr %li4
  %li5 = getelementptr i64, ptr %v2, i64 1
  %v5 = load i64, ptr %li5
  %v6 = xor i64 %v4, 1
  %v7 = or i64 %v6, %v5
  %v8 = icmp eq i64 %v7, 0
  br i1 %v8, label %b1, label %b2
b1:
  %li10 = getelementptr i64, ptr %v2, i64 0
  %v10 = load i64, ptr %li10
  %li11 = getelementptr i64, ptr %v1, i64 0
  %v11 = load i64, ptr %li11
  store i64 %v11, ptr %gcs0
  %v12 = call i64 @df.unbox_int(i64 %v11)
  %v13 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v13 to i64
  %v14 = icmp ne i64 %c0, 0
  br i1 %v14, label %b4, label %b3
b2:
  call void @dartforge_nsm_call()
  %gcvolta2 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2, ptr %ctxtopo, align 8
  ret i64 0
b3:
  %v19 = add i64 0, 65
  %v20 = add i64 %v19, %v12
  store i64 0, ptr %gcs0
  %v18 = call i64 @df.box_int(i64 %v20)
  %gcvolta3 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta3, ptr %ctxtopo, align 8
  ret i64 %v18
b4:
  %gcvolta4 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta4, ptr %ctxtopo, align 8
  ret i64 0
}

define i64 @df.rti.0a9e544d725576ee.lengths$2edart() "disable-tail-calls"="true" nounwind {
b0:
  %area = call ptr @df.get_area()
  %ga0 = getelementptr i64, ptr %area, i64 1
  %v0 = load i8, ptr %ga0
  %c0 = zext i8 %v0 to i64
  %v1 = icmp ne i64 %c0, 0
  br i1 %v1, label %b1, label %b2
b1:
  %ga2 = getelementptr i64, ptr %area, i64 0
  %v2 = load i64, ptr %ga2
  ret i64 %v2
b2:
  %lsp3 = getelementptr i64, ptr %area, i64 4
  %lsv3 = load i64, ptr %lsp3, align 8
  %lsz3 = icmp eq i64 %lsv3, 0
  %lsx3 = call i1 @llvm.expect.i1(i1 %lsz3, i1 false)
  br i1 %lsx3, label %ls3.nova, label %ls3.fim
ls3.nova:
  %lsn3 = call i64 @dartforge_string_new(ptr @.str.0, i64 17)
  store i64 %lsn3, ptr %lsp3, align 8
  br label %ls3.fim
ls3.fim:
  %v3 = phi i64 [ %lsv3, %b2 ], [ %lsn3, %ls3.nova ]
  %v4 = call i64 @dartforge_rti_recipe(i64 %v3)
  %ga5 = getelementptr i64, ptr %area, i64 0
  store i64 %v4, ptr %ga5
  %ga6 = getelementptr i64, ptr %area, i64 1
  store i8 1, ptr %ga6
  ret i64 %v4
}

define i64 @df.rti.1dee75ed5d853ef3.lengths$2edart() "disable-tail-calls"="true" nounwind {
b0:
  %area = call ptr @df.get_area()
  %ga0 = getelementptr i64, ptr %area, i64 3
  %v0 = load i8, ptr %ga0
  %c0 = zext i8 %v0 to i64
  %v1 = icmp ne i64 %c0, 0
  br i1 %v1, label %b1, label %b2
b1:
  %ga2 = getelementptr i64, ptr %area, i64 2
  %v2 = load i64, ptr %ga2
  ret i64 %v2
b2:
  %lsp3 = getelementptr i64, ptr %area, i64 6
  %lsv3 = load i64, ptr %lsp3, align 8
  %lsz3 = icmp eq i64 %lsv3, 0
  %lsx3 = call i1 @llvm.expect.i1(i1 %lsz3, i1 false)
  br i1 %lsx3, label %ls3.nova, label %ls3.fim
ls3.nova:
  %lsn3 = call i64 @dartforge_string_new(ptr @.str.1, i64 7)
  store i64 %lsn3, ptr %lsp3, align 8
  br label %ls3.fim
ls3.fim:
  %v3 = phi i64 [ %lsv3, %b2 ], [ %lsn3, %ls3.nova ]
  %v4 = call i64 @dartforge_rti_recipe(i64 %v3)
  %ga5 = getelementptr i64, ptr %area, i64 2
  store i64 %v4, ptr %ga5
  %ga6 = getelementptr i64, ptr %area, i64 3
  store i8 1, ptr %ga6
  ret i64 %v4
}

define i64 @dart_main$clo1$e3e298bca(i64 %v0, i64 %v1) "disable-tail-calls"="true" nounwind {
b0:
  %v4 = add i64 0, 1024
  %v6 = add i64 %v4, %v1
  ret i64 %v6
}

define i64 @dart_main$clo1$e3e298bca$ent(i64 %v0, ptr %v1, ptr %v2) "disable-tail-calls"="true" nounwind {
b0:
  %gcq = alloca { ptr, i64, [1 x i64] }, align 8
  %ctx = call ptr @df.context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load i64, ptr %pilhalp, align 8
  %pilhasp = call i64 @llvm.read_register.i64(metadata !{!"sp"})
  %pilhab = icmp ult i64 %pilhasp, %pilhal
  %pilhax = call i1 @llvm.expect.i1(i1 %pilhab, i1 false)
  br i1 %pilhax, label %pilha.estouro, label %pilha.ok
pilha.estouro:
  call void @dartforge_stack_overflow()
  ret i64 0
pilha.ok:
  store { ptr, i64, [1 x i64] } { ptr null, i64 4611686018427387905, [1 x i64] zeroinitializer }, ptr %gcq
  %gcs0 = getelementptr inbounds { ptr, i64, [1 x i64] }, ptr %gcq, i64 0, i32 2, i64 0
  %gcant = load ptr, ptr %ctxtopo, align 8
  store ptr %gcant, ptr %gcq, align 8
  store ptr %gcq, ptr %ctxtopo, align 8
  %fcb3 = call ptr @df.body(i64 %v0, ptr %ctx)
  %fp3 = getelementptr inbounds i8, ptr %fcb3, i64 16
  %fg3 = getelementptr inbounds i64, ptr %fp3, i64 1
  %v3 = load i64, ptr %fg3, align 8
  %li4 = getelementptr i64, ptr %v2, i64 0
  %v4 = load i64, ptr %li4
  %li5 = getelementptr i64, ptr %v2, i64 1
  %v5 = load i64, ptr %li5
  %v6 = xor i64 %v4, 1
  %v7 = or i64 %v6, %v5
  %v8 = icmp eq i64 %v7, 0
  br i1 %v8, label %b1, label %b2
b1:
  %li10 = getelementptr i64, ptr %v2, i64 0
  %v10 = load i64, ptr %li10
  %li11 = getelementptr i64, ptr %v1, i64 0
  %v11 = load i64, ptr %li11
  store i64 %v11, ptr %gcs0
  %v12 = call i64 @df.unbox_int(i64 %v11)
  %v13 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v13 to i64
  %v14 = icmp ne i64 %c0, 0
  br i1 %v14, label %b4, label %b3
b2:
  call void @dartforge_nsm_call()
  %gcvolta2 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2, ptr %ctxtopo, align 8
  ret i64 0
b3:
  %v19 = add i64 0, 1024
  %v20 = add i64 %v19, %v12
  store i64 0, ptr %gcs0
  %v18 = call i64 @df.box_int(i64 %v20)
  %gcvolta3 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta3, ptr %ctxtopo, align 8
  ret i64 %v18
b4:
  %gcvolta4 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta4, ptr %ctxtopo, align 8
  ret i64 0
}

define i64 @df.$type.toString(i64 %v0) "disable-tail-calls"="true" nounwind {
b0:
  %ctx = call ptr @df.context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load i64, ptr %pilhalp, align 8
  %pilhasp = call i64 @llvm.read_register.i64(metadata !{!"sp"})
  %pilhab = icmp ult i64 %pilhasp, %pilhal
  %pilhax = call i1 @llvm.expect.i1(i1 %pilhab, i1 false)
  br i1 %pilhax, label %pilha.estouro, label %pilha.ok
pilha.estouro:
  call void @dartforge_stack_overflow()
  ret i64 0
pilha.ok:
  %fxa1 = add i64 %v0, -2
  %fcb1 = inttoptr i64 %fxa1 to ptr
  %fp1 = getelementptr inbounds i8, ptr %fcb1, i64 16
  %fg1 = getelementptr inbounds i64, ptr %fp1, i64 0
  %v1 = load i64, ptr %fg1, align 8
  %v2 = call i64 @dartforge_rti_string(i64 %v1)
  ret i64 %v2
}

define i64 @df.$type.$3d$3d$c(i64 %v0, ptr %v1, ptr %v2) "disable-tail-calls"="true" nounwind {
b0:
  %gcq = alloca { ptr, i64, [2 x i64] }, align 8
  %ctx = call ptr @df.context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load i64, ptr %pilhalp, align 8
  %pilhasp = call i64 @llvm.read_register.i64(metadata !{!"sp"})
  %pilhab = icmp ult i64 %pilhasp, %pilhal
  %pilhax = call i1 @llvm.expect.i1(i1 %pilhab, i1 false)
  br i1 %pilhax, label %pilha.estouro, label %pilha.ok
pilha.estouro:
  call void @dartforge_stack_overflow()
  ret i64 0
pilha.ok:
  store { ptr, i64, [2 x i64] } { ptr null, i64 4611686018427387906, [2 x i64] zeroinitializer }, ptr %gcq
  %gcs0 = getelementptr inbounds { ptr, i64, [2 x i64] }, ptr %gcq, i64 0, i32 2, i64 0
  %gcs1 = getelementptr inbounds { ptr, i64, [2 x i64] }, ptr %gcq, i64 0, i32 2, i64 1
  %gcant = load ptr, ptr %ctxtopo, align 8
  store ptr %gcant, ptr %gcq, align 8
  store ptr %gcq, ptr %ctxtopo, align 8
  store i64 %v0, ptr %gcs0
  %li3 = getelementptr i64, ptr %v1, i64 0
  %v3 = load i64, ptr %li3
  store i64 %v3, ptr %gcs1
  %v4 = icmp eq i64 %v0, %v3
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v5 = call i64 @df.box_bool(i1 %v4)
  %gcvolta0 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta0, ptr %ctxtopo, align 8
  ret i64 %v5
}

define i64 @df.$type.toString$c(i64 %v0, ptr %v1, ptr %v2) "disable-tail-calls"="true" nounwind {
b0:
  %ctx = call ptr @df.context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load i64, ptr %pilhalp, align 8
  %pilhasp = call i64 @llvm.read_register.i64(metadata !{!"sp"})
  %pilhab = icmp ult i64 %pilhasp, %pilhal
  %pilhax = call i1 @llvm.expect.i1(i1 %pilhab, i1 false)
  br i1 %pilhax, label %pilha.estouro, label %pilha.ok
pilha.estouro:
  call void @dartforge_stack_overflow()
  ret i64 0
pilha.ok:
  %fxa6 = add i64 %v0, -2
  %fcb6 = inttoptr i64 %fxa6 to ptr
  %fp6 = getelementptr inbounds i8, ptr %fcb6, i64 16
  %fg6 = getelementptr inbounds i64, ptr %fp6, i64 0
  %v6 = load i64, ptr %fg6, align 8
  %v7 = call i64 @dartforge_rti_string(i64 %v6)
  %v4 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v4 to i64
  %v5 = icmp ne i64 %c0, 0
  br i1 %v5, label %b2, label %b1
b1:
  ret i64 %v7
b2:
  ret i64 0
}

define void @df.prepare_rti() "disable-tail-calls"="true" nounwind {
b0:
  %v0 = getelementptr i64, ptr @df.const.i64.2268.45aa9b8ae90a7e326e08e11079e24fa60f9b25ce32d38d12abd8bead13467dd2, i64 0
  call void @dartforge_rti_init_table(ptr %v0, i64 18139)
  ret void
}

define i64 @dartforge_dispatch_toString(i64 %obj) nounwind {
b0:
  %a = alloca [1 x i64]
  store i64 0, ptr %a
  %area = call ptr @df.get_area()
  %ic = getelementptr i64, ptr %area, i64 8
  %f = call ptr @df.selector(ptr %ic, i64 %obj, i64 -3445551396850951156, ptr @df.seln.0, i64 10)
  %r = call i64 %f(i64 %obj, ptr %a, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  ret i64 %r
}

@df.mt.$type$d = private unnamed_addr constant { i64, i64, i64, ptr, i64, ptr, i64, ptr, i64, ptr, i64, ptr, i64, ptr, i64, ptr } { i64 1073741569, i64 7, i64 -9163797671095228218, ptr @df.$type.$3d$3d$c, i64 -3445551396850951156, ptr @df.$type.toString$c, i64 -1301464018035317077, ptr @df.dart$3acore.Object.noSuchMethod$tc, i64 3599887816169301665, ptr @df.dart$3acore.Object.noSuchMethod$c, i64 4031450617872922849, ptr @df.dart$3acore.Object.hashCode$g, i64 5306504273795186908, ptr @df.dart$3acore.Object._identityHashCode$g, i64 7093520068988870176, ptr @df.dart$3acore.Object.runtimeType$g }
define ptr @df.mt.$type() nounwind {
b0:
  ret ptr @df.mt.$type$d
}
define void @df.register.program() nounwind {
b0:
  call void @dartforge_register_class_name(i64 0, ptr @.str.2, i64 6)
  call void @dartforge_register_class_name(i64 1073741569, ptr @.str.3, i64 5)
  call void @dartforge_register_subclass(i64 1073741569, i64 621)
  call void @dartforge_register_table(i64 1073741569, ptr @df.mt.$type)
  ret void
}

@df.cids = private unnamed_addr constant [32 x i64] [i64 1, i64 2, i64 3, i64 4, i64 5, i64 6, i64 7, i64 10, i64 8, i64 9, i64 11, i64 12, i64 23, i64 37, i64 29, i64 19, i64 20, i64 21, i64 22, i64 24, i64 25, i64 26, i64 27, i64 28, i64 30, i64 31, i64 32, i64 34, i64 33, i64 35, i64 17, i64 18]
@df.sdk_version = private unnamed_addr constant [47 x i8] c"3.6.2 \28stable\29 \28Wed Jan 29 01:20:39 2025 \2D0800\29"
define void @df.prepare_isolate() nounwind {
  call void @dartforge_register_cids(ptr @df.cids, i64 32)
  call void @dartforge_register_sdk_version(ptr @df.sdk_version, i64 47)
  call void @dartforge_register_table(i64 1, ptr @df.mt.dart$3acore.Null)
  call void @dartforge_register_table(i64 2, ptr @df.mt.dart$3acore._Smi)
  call void @dartforge_register_table(i64 3, ptr @df.mt.dart$3acore._Mint)
  call void @dartforge_register_table(i64 4, ptr @df.mt.dart$3acore._Double)
  call void @dartforge_register_table(i64 5, ptr @df.mt.dart$3acore.bool)
  call void @dartforge_register_table(i64 6, ptr @df.mt.dart$3acore._OneByteString)
  call void @dartforge_register_table(i64 7, ptr @df.mt.dart$3acore._TwoByteString)
  call void @dartforge_register_table(i64 10, ptr @df.mt.dart$3acore._GrowableList)
  call void @dartforge_register_table(i64 8, ptr @df.mt.dart$3acore._List)
  call void @dartforge_register_table(i64 9, ptr @df.mt.dart$3acore._ImmutableList)
  call void @dartforge_register_table(i64 11, ptr @df.mt.dart$3acore._Closure)
  call void @dartforge_register_table(i64 12, ptr @df.mt.dart$3acore._Record)
  call void @dartforge_register_table(i64 23, ptr @df.mt.dart$3atyped_data._Uint8List)
  call void @dartforge_register_table(i64 37, ptr @df.mt.dart$3atyped_data._Uint8ArrayView)
  call void @dartforge_register_table(i64 29, ptr @df.mt.dart$3atyped_data._Int64List)
  call void @dartforge_register_table(i64 19, ptr @df.mt.dart$3atyped_data._Float32x4)
  call void @dartforge_register_table(i64 20, ptr @df.mt.dart$3atyped_data._Int32x4)
  call void @dartforge_register_table(i64 21, ptr @df.mt.dart$3atyped_data._Float64x2)
  call void @dartforge_register_table(i64 22, ptr @df.mt.dart$3atyped_data._Int8List)
  call void @dartforge_register_table(i64 24, ptr @df.mt.dart$3atyped_data._Uint8ClampedList)
  call void @dartforge_register_table(i64 25, ptr @df.mt.dart$3atyped_data._Int16List)
  call void @dartforge_register_table(i64 26, ptr @df.mt.dart$3atyped_data._Uint16List)
  call void @dartforge_register_table(i64 27, ptr @df.mt.dart$3atyped_data._Int32List)
  call void @dartforge_register_table(i64 28, ptr @df.mt.dart$3atyped_data._Uint32List)
  call void @dartforge_register_table(i64 30, ptr @df.mt.dart$3atyped_data._Uint64List)
  call void @dartforge_register_table(i64 31, ptr @df.mt.dart$3atyped_data._Float32List)
  call void @dartforge_register_table(i64 32, ptr @df.mt.dart$3atyped_data._Float64List)
  call void @dartforge_register_table(i64 34, ptr @df.mt.dart$3atyped_data._Int32x4List)
  call void @dartforge_register_table(i64 33, ptr @df.mt.dart$3atyped_data._Float32x4List)
  call void @dartforge_register_table(i64 35, ptr @df.mt.dart$3atyped_data._Float64x2List)
  call void @dartforge_register_table(i64 17, ptr @df.mt.dart$3aisolate._SendPort)
  call void @dartforge_register_table(i64 18, ptr @df.mt.dart$3aisolate._Capability)
  call void @df.register.dart$3a_internal()
  call void @df.register.dart$3acore()
  call void @df.register.dart$3a_compact_hash()
  call void @df.register.dart$3acollection()
  call void @df.register.dart$3amath()
  call void @df.register.dart$3aconvert()
  call void @df.register.dart$3aasync()
  call void @df.register.dart$3atyped_data()
  call void @df.register.dart$3aisolate()
  call void @df.register.dart$3adeveloper()
  call void @df.register.dart$3anativewrappers()
  call void @df.register.dart$3affi()
  call void @df.register.dart$3a_http()
  call void @df.register.dart$3aio()
  call void @df.register.dart$3amirrors()
  call void @df.register.program()
  call void @df.prepare_rti()
  call void @dartforge_ffi_register_type(i64 693, i64 98)
  call void @dartforge_ffi_register_type(i64 694, i64 97)
  call void @dartforge_ffi_register_type(i64 698, i64 100)
  call void @dartforge_ffi_register_type(i64 701, i64 102)
  call void @dartforge_ffi_register_type(i64 702, i64 72)
  call void @dartforge_ffi_register_type(i64 703, i64 105)
  call void @dartforge_ffi_register_type(i64 704, i64 115)
  call void @dartforge_ffi_register_type(i64 705, i64 105)
  call void @dartforge_ffi_register_type(i64 706, i64 108)
  call void @dartforge_ffi_register_type(i64 707, i64 97)
  call void @dartforge_ffi_register_type(i64 708, i64 108)
  call void @dartforge_ffi_register_type(i64 709, i64 108)
  call void @dartforge_ffi_register_type(i64 710, i64 108)
  call void @dartforge_ffi_register_type(i64 719, i64 112)
  call void @dartforge_ffi_register_type(i64 720, i64 115)
  call void @dartforge_ffi_register_type(i64 721, i64 97)
  call void @dartforge_ffi_register_type(i64 722, i64 109)
  call void @dartforge_ffi_register_type(i64 725, i64 116)
  call void @dartforge_ffi_register_type(i64 726, i64 106)
  call void @dartforge_ffi_register_type(i64 727, i64 109)
  call void @dartforge_ffi_register_type(i64 728, i64 104)
  call void @dartforge_ffi_register_type(i64 729, i64 109)
  call void @dartforge_ffi_register_type(i64 731, i64 104)
  call void @dartforge_ffi_register_type(i64 732, i64 106)
  call void @dartforge_ffi_register_type(i64 733, i64 109)
  call void @dartforge_ffi_register_type(i64 734, i64 109)
  call void @dartforge_ffi_register_type(i64 735, i64 116)
  call void @dartforge_ffi_register_type(i64 736, i64 42)
  call void @dartforge_ffi_register_type(i64 737, i64 118)
  call void @dartforge_ffi_register_type(i64 738, i64 105)
  call void @dartforge_prepare_embedder()
  ret void
}

define void @dartforge_entry() nounwind {
  call void @dartforge_register_isolates(ptr @df.prepare_isolate, ptr null)
  call void @df.prepare_isolate()
  call void @dart_main()
  ret void
}

define i32 @main() nounwind {
  %r = call i32 @dartforge_init(ptr @dartforge_entry, ptr @dartforge_dispatch_toString)
  ret i32 %r
}

@df.seln.0 = private unnamed_addr constant [10 x i8] c"c:toString"

@df.area = private unnamed_addr constant [12 x i64] [i64 -7318119598941579387, i64 10, i64 1286746223075843796, i64 -8374729828724864046, i64 3567685117937366277, i64 -6018209748338069523, i64 9007518357102002052, i64 9007519456613630263, i64 7748048044044893249, i64 7748046944533265038, i64 -457777237921703800, i64 -457776138410075589]
define void @df.prepare_area() nounwind {
  %a = call ptr @dartforge_globals_area(ptr @df.area)
  ret void
}
@df.area_id.program = internal global i64 0, align 8
define internal ptr @df.get_area() alwaysinline nounwind {
%ctx = call ptr @df.context()
%id = load atomic i64, ptr @df.area_id.program monotonic, align 8
%tp = getelementptr inbounds i8, ptr %ctx, i64 16
%t = load ptr, ptr %tp, align 8
%np = getelementptr inbounds i8, ptr %ctx, i64 24
%n = load i64, ptr %np, align 8
%dentro = icmp ult i64 %id, %n
br i1 %dentro, label %ler, label %lenta
ler:
%e = getelementptr ptr, ptr %t, i64 %id
%p = load ptr, ptr %e, align 8
%tem = icmp ne ptr %p, null
br i1 %tem, label %pronto, label %lenta
pronto:
ret ptr %p
lenta:
%q = call ptr @dartforge_globals_area_id(ptr @df.area, ptr @df.area_id.program)
ret ptr %q
}
; Defined in another module (source SDK) or in the runtime
declare void @df.dart$3acore..print(i64)
declare i64 @df.dart$3acore.List.new$3agenerate(i64, i64, i1, i64)
declare i64 @df.dart$3acore.Object._identityHashCode$g(i64, ptr, ptr)
declare i64 @df.dart$3acore.Object.hashCode$g(i64, ptr, ptr)
declare i64 @df.dart$3acore.Object.noSuchMethod$c(i64, ptr, ptr)
declare i64 @df.dart$3acore.Object.noSuchMethod$tc(i64, ptr, ptr)
declare i64 @df.dart$3acore.Object.runtimeType$g(i64, ptr, ptr)
declare i64 @df.dart$3acore.String.new$3afromCharCodes(i64, i64, i64)
declare ptr @df.mt.dart$3acore.Null()
declare ptr @df.mt.dart$3acore._Closure()
declare ptr @df.mt.dart$3acore._Double()
declare ptr @df.mt.dart$3acore._GrowableList()
declare ptr @df.mt.dart$3acore._ImmutableList()
declare ptr @df.mt.dart$3acore._List()
declare ptr @df.mt.dart$3acore._Mint()
declare ptr @df.mt.dart$3acore._OneByteString()
declare ptr @df.mt.dart$3acore._Record()
declare ptr @df.mt.dart$3acore._Smi()
declare ptr @df.mt.dart$3acore._TwoByteString()
declare ptr @df.mt.dart$3acore.bool()
declare ptr @df.mt.dart$3aisolate._Capability()
declare ptr @df.mt.dart$3aisolate._SendPort()
declare ptr @df.mt.dart$3atyped_data._Float32List()
declare ptr @df.mt.dart$3atyped_data._Float32x4()
declare ptr @df.mt.dart$3atyped_data._Float32x4List()
declare ptr @df.mt.dart$3atyped_data._Float64List()
declare ptr @df.mt.dart$3atyped_data._Float64x2()
declare ptr @df.mt.dart$3atyped_data._Float64x2List()
declare ptr @df.mt.dart$3atyped_data._Int16List()
declare ptr @df.mt.dart$3atyped_data._Int32List()
declare ptr @df.mt.dart$3atyped_data._Int32x4()
declare ptr @df.mt.dart$3atyped_data._Int32x4List()
declare ptr @df.mt.dart$3atyped_data._Int64List()
declare ptr @df.mt.dart$3atyped_data._Int8List()
declare ptr @df.mt.dart$3atyped_data._Uint16List()
declare ptr @df.mt.dart$3atyped_data._Uint32List()
declare ptr @df.mt.dart$3atyped_data._Uint64List()
declare ptr @df.mt.dart$3atyped_data._Uint8ArrayView()
declare ptr @df.mt.dart$3atyped_data._Uint8ClampedList()
declare ptr @df.mt.dart$3atyped_data._Uint8List()
declare void @df.register.dart$3a_compact_hash()
declare void @df.register.dart$3a_http()
declare void @df.register.dart$3a_internal()
declare void @df.register.dart$3aasync()
declare void @df.register.dart$3acollection()
declare void @df.register.dart$3aconvert()
declare void @df.register.dart$3acore()
declare void @df.register.dart$3adeveloper()
declare void @df.register.dart$3affi()
declare void @df.register.dart$3aio()
declare void @df.register.dart$3aisolate()
declare void @df.register.dart$3amath()
declare void @df.register.dart$3amirrors()
declare void @df.register.dart$3anativewrappers()
declare void @df.register.dart$3atyped_data()


@dartforge_context_slot = external global i64, align 8
define internal ptr @df.context() nounwind alwaysinline {
  %ctx = call ptr @dartforge_context()
  ret ptr %ctx
}

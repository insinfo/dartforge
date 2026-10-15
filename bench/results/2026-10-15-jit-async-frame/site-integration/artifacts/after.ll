; DartForge contract provenance 3c64efcdca32be4ecada61e129ab30dee0e57bc5acada9c42e13e9fe33b73686
; df.cone-class 0
; df.cone-parent 1073741569 621
; df.async-frame dart_main v0 dart_main$async$q0fee530a$ent 6
; df.async-frame dart_main$async$q0fee530a v52 df.main$2edart..worker$async$qead0428d$ent 8
; df.async-frame dart_main$async$q0fee530a v72 df.main$2edart..worker$async$qead0428d$ent 8
; df.async-frame df.main$2edart..worker v0 df.main$2edart..worker$async$qead0428d$ent 8
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
%ctx = call ptr @dartforge_context()
%o = select i1 %b, i64 360, i64 368
%p = getelementptr inbounds i8, ptr %ctx, i64 %o
%r = load i64, ptr %p, align 8, !invariant.load !{}
ret i64 %r
}
define internal i1 @df.unbox_bool(i64 %r) alwaysinline nounwind {
%ctx = call ptr @dartforge_context()
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
%ctx = call ptr @dartforge_context()
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
%ctx = call ptr @dartforge_context()
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
  %fp = getelementptr inbounds i8, ptr %h, i64 1
  %f = load i8, ptr %fp, align 1
  %b = and i8 %f, 1
  %x = icmp ne i8 %b, 0
  %cp = getelementptr inbounds i8, ptr %h, i64 16
  %c = load ptr, ptr %cp, align 8
  %r = select i1 %x, ptr %c, ptr %h
  ret ptr %r
}
declare void @llvm.memcpy.p0.p0.i64(ptr, ptr, i64, i1)
declare void @llvm.memset.p0.i64(ptr, i8, i64, i1)
declare ptr @llvm.stacksave.p0()
declare void @llvm.stackrestore.p0(ptr)

; Constantes de string WTF-8
@.str.0 = private unnamed_addr constant [88 x i8] c"\2FVolumes\2Fssd_dock\2FMyRustProjects\2Fdartforge\2Ftarget\2Fjit\2Dasync\2Dframe\2Dreload\2D17855\2Fsuspended"
@.str.1 = private unnamed_addr constant [9 x i8] c"suspended"
@.str.2 = private unnamed_addr constant [86 x i8] c"\2FVolumes\2Fssd_dock\2FMyRustProjects\2Fdartforge\2Ftarget\2Fjit\2Dasync\2Dframe\2Dreload\2D17855\2Frelease"
@.str.3 = private unnamed_addr constant [85 x i8] c"\2FVolumes\2Fssd_dock\2FMyRustProjects\2Fdartforge\2Ftarget\2Fjit\2Dasync\2Dframe\2Dreload\2D17855\2Fresult"
@.str.4 = private unnamed_addr constant [4 x i8] c"new\3A"
@.str.5 = private unnamed_addr constant [1 x i8] c"\3A"
@.str.6 = private unnamed_addr constant [1 x i8] c"\0A"
@.str.7 = private unnamed_addr constant [4 x i8] c"L\3CV\3E"
@.str.8 = private unnamed_addr constant [6 x i8] c"Object"
@.str.9 = private unnamed_addr constant [11 x i8] c"_AsyncFrame"
@.str.10 = private unnamed_addr constant [5 x i8] c"_Type"

; VTable of class Object (id 0)
; VTable of class _AsyncFrame (id 1073741568)
; VTable of class _Type (id 1073741569)
; Closures: invalid entry and constant vectors
define internal i64 @df_clo_invalido(i64 %c, ptr %a, ptr %d) nounwind {
b0:
  ret i64 0
}
@df.arr.0 = private unnamed_addr constant [2 x i64] [i64 0, i64 0], align 8
@df.arr.1 = private unnamed_addr constant [2 x i64] [i64 1, i64 0], align 8
@df.arr.2 = private unnamed_addr constant [3 x i64] [i64 1, i64 1, i64 954177780379921842], align 8
@df.arr.3 = private unnamed_addr constant [3056 x i64] [i64 408775026951168, i64 -5252157194898122120, i64 -1438916917502400370, i64 7832687547508459655, i64 -2791304351955686711, i64 7289843828167193563, i64 6585834216587835623, i64 4926940334982166013, i64 324832158737751190, i64 -2885796046660696017, i64 -1513845291101464621, i64 -1755079935002149, i64 -4640031075056238920, i64 -2254249706129024642, i64 -3885221654803059044, i64 -361525265852007467, i64 -4739672760348627867, i64 4605371522081886400, i64 -8025989866715087353, i64 2670043765485241559, i64 3731598369915442383, i64 -1875045859174862683, i64 5006616583730493686, i64 9205860671124155793, i64 6888549790506810614, i64 3567451942300413934, i64 9115138042434630189, i64 1678074465839078712, i64 -7929739509599635723, i64 -8789344433948960045, i64 -403283324153609040, i64 -7292082776404394417, i64 5825874973251571690, i64 3698806410037251277, i64 8826913844147203411, i64 -3831128856547796388, i64 -802234475883974697, i64 736384547489505410, i64 3471397536395870748, i64 -6753940067036821418, i64 -7219078155270966537, i64 -6295005851428118641, i64 6148990924079060326, i64 -9117011051359327565, i64 6222473520225818277, i64 -894339817233095250, i64 9007145744823276836, i64 -2975028984381440072, i64 4637177165844906771, i64 2499505199882807872, i64 -2423307104058521592, i64 -2992145882637516834, i64 -7345163103549370093, i64 7691229810311413769, i64 2821055231795725767, i64 -7867223085169396025, i64 -3612480655316859381, i64 -1594477023294129221, i64 3937459159388585713, i64 7927183743021146615, i64 -2174145758409480102, i64 6026231903561831319, i64 44048418760964614, i64 -364754779959815168, i64 7255492440565546742, i64 8320657584376902119, i64 -7828735278453329035, i64 150477512390728923, i64 7993013825538906734, i64 533383869021251290, i64 6040161890707564192, i64 7180158271321132024, i64 8784626422262231922, i64 -5060177623336296415, i64 7942283956645913309, i64 6084981987955541056, i64 -6727219619300669936, i64 2501276111868595485, i64 4741512787494189840, i64 -2109153504536810336, i64 4598756014269992660, i64 -5100971342835591523, i64 -7569573030099760851, i64 -4251083231774281929, i64 -3817905918982688869, i64 -4886626022428835764, i64 -5219523072267781739, i64 -1100856899689285168, i64 -5283731525407986493, i64 5645320293264005472, i64 -7183223261571521149, i64 5525989983525654150, i64 -6401007910106222588, i64 -7951350269478368301, i64 4628496200861804486, i64 8748444065695906105, i64 -5215859439861000565, i64 -4034334287228864152, i64 8001258133547431012, i64 4764150646369384529, i64 -290878890770492706, i64 5456503180379273295, i64 320353620784888726, i64 1752793699305748244, i64 -1491896225871428404, i64 -2556853421509456900, i64 -5602059324897153307, i64 -2778952318721626319, i64 -6907468094949714190, i64 1063991525058872184, i64 -5823750916045501148, i64 6265088643030535612, i64 -4434955249320138824, i64 -2544149187661090467, i64 -5088301326708913387, i64 2405136165056912683, i64 3150647673546848640, i64 -1561099235324320721, i64 -7173148186446916423, i64 -5378481760865264420, i64 7690491196770833852, i64 -4322127259831925895, i64 -5917354166215433202, i64 8649293544365341870, i64 -337824503692848141, i64 5750781799764186941, i64 -5159721173503755386, i64 -6945180474079969805, i64 1154240427088554392, i64 4826376180001475703, i64 -4453308180859092286, i64 1616763755513236972, i64 2375081765494431998, i64 -5512667564535793677, i64 -9217805609252694032, i64 6360793654210252428, i64 5625772718715603096, i64 8562871741629519106, i64 -7136408947749601896, i64 6471502914377076581, i64 1445485734198426358, i64 -3502220553759452012, i64 -2224362193395321227, i64 3077620672351784996, i64 -5663758304745485111, i64 2703148208293097067, i64 -8572784943670615830, i64 451826649721843397, i64 3675010774784868530, i64 7839441674929039906, i64 -7884165554906690684, i64 -9060569266734125755, i64 -5033295824855333692, i64 4255966436771934802, i64 -1346524099047705456, i64 -4288326869795638768, i64 -4578108973853707161, i64 -1232826798844685913, i64 -2510594575515942056, i64 5847677877787288529, i64 1232156871244137402, i64 4034216471159882557, i64 -63825551294001393, i64 -3275525837364316068, i64 6177873630214968764, i64 -4884339079844086729, i64 -4794117583056128171, i64 5035510967824112579, i64 1813753263058186461, i64 -6304028895849588195, i64 -5523509886275654931, i64 3850312928596014441, i64 8002674295009799951, i64 1014839397329080117, i64 -6705592363451514356, i64 -3637325755325810567, i64 -3640432888554034132, i64 -8837385353198405267, i64 6612556782072323012, i64 -4285918570762222459, i64 -5727729330268371593, i64 8955497437290754056, i64 5486204050829511601, i64 853111975876501079, i64 -3045473667987472053, i64 8957050238646205517, i64 -728039091010253353, i64 1696092032134213504, i64 1107230072408622301, i64 8833923707633937144, i64 -6893487704407926049, i64 670286517500251416, i64 5574622141515124054, i64 6199571424035558921, i64 -6769727726189239987, i64 5645053282319172774, i64 7628710589519103034, i64 7847184838725199678, i64 -5299780514407932711, i64 -7364064012135467412, i64 -236895091273168553, i64 -7676721550801321266, i64 -1198742455757740528, i64 -8300038779001386161, i64 -2485324300709089830, i64 2719457455140899178, i64 8602040033837755864, i64 -6096389142305298344, i64 8437579699768211493, i64 -1588854444581738302, i64 2726778508362329135, i64 -2485321277052068392, i64 2726777882425493877, i64 -4970641730492998024, i64 5453516263645254397, i64 -8797306075307215120, i64 -5967803925490052915, i64 5609274926048687136, i64 681412995619184791, i64 7734723853161589682, i64 1837509744923041154, i64 297856514831895957, i64 3851978049633924193, i64 -3054949995643243693, i64 -6765028358017362840, i64 786009158125336913, i64 1465799696894560336, i64 -8999435490910262367, i64 4875643037049688453, i64 -330327844685846072, i64 8271326036513545524, i64 -6400571152600191846, i64 8176454941280977245, i64 8445627616798620177, i64 609455379815699278, i64 9198555390088647993, i64 7911993637493497369, i64 7215675682545527606, i64 -2935727189623517517, i64 4242962650515260051, i64 -4553504930450279389, i64 -7763850441347847136, i64 1669209705968586913, i64 -6837424783926617257, i64 -8497789581271816808, i64 -7610374173738372519, i64 2908608630489129244, i64 -9063160389241097452, i64 8130493337867205160, i64 4882690323312856060, i64 -381869030156446413, i64 -8094317346632706808, i64 6062947690912552400, i64 6226935604048067934, i64 2786901645123131423, i64 8717765649022277256, i64 -5704896334579207297, i64 -5263505313516254958, i64 3824215765283450784, i64 -8261247078401340070, i64 5631625277400249895, i64 -768274999490823492, i64 7269505588729586313, i64 5699912938338461527, i64 9111110074705037212, i64 -5998932368805194306, i64 -6734898483186112391, i64 -8586285824719288107, i64 6170639950301156770, i64 5844028098341253252, i64 4977310428811529492, i64 926535655631260195, i64 -5317164017233652261, i64 9196986724992260779, i64 -8468079521565366051, i64 8887443177117219670, i64 2360584766316305802, i64 -6362370972067926356, i64 4977874561370090728, i64 4617764678787828347, i64 -2256249098592296972, i64 -5377071627581887697, i64 -2833972079337576277, i64 -758319238054630475, i64 4978015448889473811, i64 4564239091077647035, i64 8866556161859236945, i64 -5717052578844982429, i64 -2486089930906226419, i64 6236072983165359632, i64 -8432050722398434565, i64 2765389371577924445, i64 -1518811831408125056, i64 8181627947211721256, i64 -8201118503112524697, i64 -8387167089247280377, i64 -2975655685964728834, i64 -3357878062532433743, i64 -7283354319240116390, i64 2932360302665577561, i64 7676994603783336406, i64 -2865292763816704746, i64 1594755919116191862, i64 8602171984521472117, i64 3127598804964844214, i64 -731644045965802122, i64 -2303494277554542411, i64 3092544234635235462, i64 7456143985285195402, i64 544425270743734332, i64 -104257015526803549, i64 -4722190580797918347, i64 8542100740397194658, i64 -3337513131589620761, i64 -3878752232702362811, i64 -2790800250081880114, i64 7790725646361862795, i64 -1724637831769385925, i64 6372290152391197722, i64 -5853123780424909900, i64 1731276810399467018, i64 1214847283763900141, i64 5856259915556199346, i64 -5688733861137571054, i64 6299492978939767133, i64 3867463029914079597, i64 -3217352359388206003, i64 -5651969492254299452, i64 6740616218053260698, i64 1754205285290183809, i64 3279810871275990849, i64 -8303974197963150815, i64 2486884842424054921, i64 7571348667724205953, i64 3632455272545463602, i64 1356470353533767981, i64 -7321152142531247088, i64 8524507439899364172, i64 2632749951689541493, i64 3365043844274611706, i64 -5434325134521404598, i64 -6109579796377142812, i64 -1527394962212440897, i64 -7377253977766310609, i64 5980409990047511398, i64 -8928246252092311465, i64 -7407707587533971499, i64 -4107585832213811492, i64 -3413334992310054249, i64 -3925073549209019713, i64 -5001118303328807527, i64 -6068840065359111926, i64 3524854941375386819, i64 5847405843715899065, i64 3551877601729418004, i64 -3931173198511095815, i64 -7845642873386193741, i64 8733019322475588407, i64 -5990266473295349069, i64 7136774274033357598, i64 3721603599666865906, i64 -7944056361416547633, i64 478428789655918068, i64 5566456624393391130, i64 -3037054958657600717, i64 2886896240024091712, i64 7209093950668243267, i64 1678672552683497589, i64 9187130726816710622, i64 -1015013113474450502, i64 9142245153903345532, i64 6732598076260908088, i64 8345816003171980125, i64 4301146666751023705, i64 7522670817904854310, i64 -122817622545151978, i64 3595579534098545791, i64 531580385655053090, i64 5185859033811087438, i64 5011238804031065712, i64 -6027688930979687736, i64 -8838120019858201918, i64 9193297181236013765, i64 2396784766368446965, i64 -1056050858116899528, i64 -1428630137172647593, i64 2079783176710384391, i64 -4913508660607987985, i64 3704886008621550047, i64 4651519263710935674, i64 329901626654469328, i64 2461718168455757138, i64 -6240403734269851429, i64 8787980476334095556, i64 -6652007773054091785, i64 -5714070621891000393, i64 -8427579946116117704, i64 -3295892920381346125, i64 -2367130764528971549, i64 4104063777406903258, i64 -5892496412456207175, i64 4396010094894495128, i64 -8980006809084434438, i64 -5431969235722115368, i64 -9056484458988707917, i64 4457677672426060799, i64 -6291100672269436862, i64 169093964438437363, i64 2028680535556601376, i64 9189727445469393028, i64 2973859219725550759, i64 8716568410348268842, i64 3930629344230533606, i64 3315534123585571747, i64 146948492945490121, i64 -8996816250542456507, i64 -110820101703784312, i64 7000280959886979493, i64 -3790640638910049484, i64 3229509285379905943, i64 1754612798356285068, i64 7521880392436597453, i64 8694524829620580253, i64 -4626083575153623243, i64 -613523213894293908, i64 6967054563428227601, i64 8427923460144488563, i64 -3853947757975011912, i64 8484611690747132875, i64 -7518144299345091538, i64 -153242319431590737, i64 -7413877515591262790, i64 1139162626734498299, i64 -1428414523623098805, i64 -8132922113201399170, i64 -6164097049933470519, i64 2454018886378002419, i64 -2002233622901521535, i64 1408674206821741802, i64 -5816385291743190478, i64 -6602334990219098289, i64 -3439428171190656331, i64 1628492780485210615, i64 -6603342143203574542, i64 1518038475212667083, i64 7472838759579796937, i64 -8137825144549243268, i64 5042079236145865554, i64 -7535050637235828490, i64 -4700955107360005215, i64 -5059649478144394420, i64 1218952745812513312, i64 -801085629120938989, i64 -3518410978275431977, i64 -3007257377927632283, i64 5350572556534780164, i64 -3271835132675000738, i64 -119650724414175941, i64 -1347557485352526482, i64 -5582148631895291360, i64 -3630658173204393844, i64 3915104186033741633, i64 3054663827603877259, i64 1656755563937862380, i64 8172394011888621535, i64 -2863132459741432560, i64 8181030295535433796, i64 -6906090886922157318, i64 8148251331468989521, i64 6877490580400662292, i64 4978653015886516464, i64 -6268436697804880680, i64 -6647765826238578047, i64 1244839549334294804, i64 888486739684975719, i64 3492532126777071633, i64 8065527999559423127, i64 1246802735227170276, i64 -421694161935702911, i64 2468337393923676095, i64 9062410837975245584, i64 -5537139621141788599, i64 3621915824211772168, i64 -5239683792278116301, i64 4837305306056862898, i64 -1853194480094569119, i64 5048968376884386063, i64 7347363417563946648, i64 459996982899257075, i64 -6874489707778597808, i64 -8424033048145611475, i64 -5357512421157538859, i64 -339696477944321646, i64 961535789588468516, i64 -1252495209405709504, i64 5805068654269619951, i64 -1351186285291622709, i64 -622123381986577247, i64 4548516735446976837, i64 -5965582390073900417, i64 4942848091444984013, i64 -5370770567937453499, i64 7932286252445479353, i64 -270715463129628950, i64 3323368972727578875, i64 8102857874808099301, i64 -3180724660348128241, i64 -895161353845141999, i64 1977081483271948717, i64 -7499716207254483599, i64 -4031896844694173937, i64 7413829696199228856, i64 7942632622532127773, i64 -675681944201482789, i64 7542834408041371200, i64 7030760167264177839, i64 -5787286042502044513, i64 -4077052104647623384, i64 7745563622838171135, i64 -811668167211604884, i64 3901295008562171083, i64 -8986058531783391761, i64 4286164219056464155, i64 758457260036012354, i64 8402606610636766783, i64 6676978976760317609, i64 -2135298683693855974, i64 6366064706896845418, i64 6798472355110764158, i64 -1139609704946134330, i64 -3606729034241741058, i64 -5761673338137376803, i64 -927254880990524120, i64 2394541113862318552, i64 -3636463999446839399, i64 7374982014509354300, i64 -8093320666411700638, i64 -6219978617940585432, i64 7057436146725769741, i64 5348576551404968429, i64 -7004701752020295821, i64 -5206777287270391594, i64 -3726949357973463937, i64 -7001827027561768518, i64 5629489705041383353, i64 2722878737528407991, i64 -2988350736828330615, i64 4122937052594934745, i64 -6831102904667725582, i64 2640715662168982564, i64 8768869293579778531, i64 1179798735722608175, i64 1499994090379496666, i64 4754743750064242124, i64 -8565612724550519539, i64 -5245835898247864904, i64 5432812302483114053, i64 -3027975988448669049, i64 -4503086014230864321, i64 -7254061466486599969, i64 432806358359107649, i64 3435905978338795532, i64 -8511650855342820288, i64 5515400175691867733, i64 -1016720610290367857, i64 1315951803084769789, i64 957225684205577242, i64 3089297786118536952, i64 6926564884731029032, i64 -7488252097422781605, i64 7250905495095951128, i64 1461142242635466022, i64 6347089263590715830, i64 7155943413727400710, i64 8454074696762660328, i64 1340364856878994631, i64 -80500671600361606, i64 -1470381469532382584, i64 -4762267482069995995, i64 -1233489439168753083, i64 2723582969208751089, i64 474725197464818413, i64 -9008275668040140688, i64 8427216155638143267, i64 -1442324231112716802, i64 -2779621333995122977, i64 1564386835503803079, i64 3804504474016161215, i64 9203190452140528473, i64 -433755016813920603, i64 3722145824096089972, i64 -4596379518335887818, i64 -6145228065393499548, i64 3919632800733004622, i64 3166295460081228244, i64 943716466714921102, i64 6256714038652254768, i64 6337802017825941047, i64 -6571738471838586673, i64 -8918718621479675148, i64 -4439958131568529894, i64 -7836048553017145215, i64 7426658601114721717, i64 -5774226154572114803, i64 8895992410970809696, i64 -6262591175032577149, i64 1462617659543376385, i64 7360451174298588151, i64 -375374687545671148, i64 -5581662360286145543, i64 7471789371468489190, i64 2534469379361984965, i64 5220907498349020919, i64 -731655813747273617, i64 -7711933292301170469, i64 -6689548142380394155, i64 5992611919927757528, i64 -7387329490912266751, i64 6689402507832932168, i64 3806906715006756056, i64 2150250914895797721, i64 3411229776875702354, i64 5197131790318182527, i64 6502530540246725028, i64 -8814314939972709195, i64 8000980141940804896, i64 8236998507477036105, i64 -2014963530049900869, i64 -5328449366194425319, i64 -9132489149049047592, i64 7063638898756397834, i64 2950802464367049398, i64 -6108074781146182155, i64 219748115423019056, i64 -7030645129236422940, i64 -8027832704107608796, i64 -8430034157184458830, i64 6550711715965470267, i64 1766145976340275524, i64 2482003179789889515, i64 2460785725501549666, i64 1824930535974524615, i64 -5268546204062782055, i64 -1809266191835807189, i64 6624173932936588558, i64 3690901014798616741, i64 725588596720648420, i64 2477474002177096737, i64 -903906358181802179, i64 3174477394295196916, i64 5033097435893864717, i64 -1931687461096416279, i64 5404554313412606438, i64 -8225339343139023093, i64 -37018097788315708, i64 3960222922079194100, i64 -4595957587177129161, i64 7427042813608953439, i64 -794192643516823274, i64 1786486012780180734, i64 6230116775930916169, i64 7608753949170071816, i64 5103186496715873270, i64 5407460403222451670, i64 -7967047772278520760, i64 -4919479208273613006, i64 3513580728725103097, i64 7169846881842018205, i64 -4392708808214560645, i64 858445525801698101, i64 4580615829928426484, i64 -8774463038832417653, i64 -1827928269492777230, i64 5752810892469307481, i64 7657376467861221863, i64 -7356474268529277824, i64 1995512232683058423, i64 5193129833547637889, i64 -8415959820350466189, i64 5888243121571649652, i64 6192066974936720349, i64 1178571638036407369, i64 3129567568179736559, i64 -4483421149348493803, i64 -1085104792800029936, i64 7620188818827897352, i64 -1671857882617294323, i64 4086876002314111132, i64 5097132967689317824, i64 -1071717663412466072, i64 471047516924944349, i64 5418516615642166369, i64 4897051955724931031, i64 -9052109572461316217, i64 -1442959842818638952, i64 -3391722684623729950, i64 4833371620276653975, i64 1022533882291758580, i64 -8039508263869971364, i64 -730694893866947470, i64 -7660119107576581482, i64 -6998794875622122975, i64 -4355826388902908303, i64 892201115187199035, i64 4964967065576172607, i64 -1793754371729868686, i64 -8252721291221201166, i64 -147078546843509226, i64 -229627812988524773, i64 -5082123036497841951, i64 -3564161331044828604, i64 8266850946948895945, i64 3833253342735964635, i64 6620695212105180913, i64 -3440265936536082701, i64 -5400838375171864242, i64 6841920113949039081, i64 3927746968981317356, i64 -4868415242197971459, i64 7848104368286657030, i64 -3210839454188814617, i64 3487367018379940036, i64 3902190216284047789, i64 6537588588721884711, i64 -2621777635442692058, i64 2214708451296992532, i64 -2888562288051625336, i64 -906281170813876313, i64 1000305656137686106, i64 -4001695179803947971, i64 2416649349088644376, i64 -1754225369433991386, i64 6026325733770042812, i64 7966403395402587671, i64 7982742969931233698, i64 -3147789059397123481, i64 -8892331733464763650, i64 -6485185250725729369, i64 5651540050223024144, i64 -2177840995580996708, i64 -2176073631649098544, i64 3875899649253712893, i64 -4120967552906359415, i64 236351892589782795, i64 -8002041715135616717, i64 -8146508393189235505, i64 3985870742135537454, i64 3429949078853204752, i64 -4702351796429621860, i64 2601694835733181414, i64 3455527146999324370, i64 -7545093666084674302, i64 -8862014393314854766, i64 -3842649575232462035, i64 -637481306113099002, i64 5314187477222953050, i64 3005882183217914166, i64 4972205484799144029, i64 8390831977965382838, i64 -2396690281950016366, i64 -846102042374623105, i64 -3296381846867081059, i64 -3250213160904119167, i64 -7768830472280833529, i64 -5524233079848867308, i64 2341692708734388770, i64 840412689212600168, i64 5264188119322632057, i64 3732173685161422478, i64 -5731125145214466757, i64 6860728440940355611, i64 -1027955915828313091, i64 8995154496628026588, i64 8141511389851188343, i64 -2626880235647659061, i64 2503940070232340636, i64 92521754502364819, i64 -803927411721726186, i64 -2787870245262538117, i64 -4325026592896781339, i64 9130721685161310717, i64 -6375065858076043404, i64 -5831856158572333115, i64 -4988062252742510378, i64 231507938924751293, i64 7837523769635726750, i64 3869832751183797518, i64 -2557469052949781392, i64 7572382606684789680, i64 -1836242846119068556, i64 4891013370167724194, i64 1656417702319183895, i64 -4036458966119218781, i64 -8068029018339759557, i64 -6886435917725030738, i64 4985962461880695480, i64 -3027594394079023442, i64 8833956410137626525, i64 -3714473681715342122, i64 -7276064119785375082, i64 9129131456668447034, i64 -1285373261712363185, i64 -4014374970967423794, i64 -2716866235745493162, i64 -8625560315268389851, i64 6427136391137683143, i64 -8192177673874624583, i64 727116084250553257, i64 4202367296941417887, i64 -1153195436326987134, i64 1209060118821802451, i64 7758095779975582886, i64 -6205912678838739137, i64 6730424323737153612, i64 4519813423155658907, i64 492238289473305576, i64 7360228547901579101, i64 1426071854043993939, i64 -559454459592891163, i64 7674956024307212643, i64 1986799193050010640, i64 -7953768672149531868, i64 -3368822504495504964, i64 -8441301312349286130, i64 -8970114220871646493, i64 6267433717938608498, i64 -2227882577190747462, i64 -2608771751142292371, i64 6253336419811713987, i64 -5043041706722037590, i64 6860173724924084477, i64 -3089229023564942150, i64 -4367913712611385762, i64 2106220284894266317, i64 1706703693827397430, i64 -2755206170128180370, i64 4787727298354706957, i64 -2714573810333943940, i64 -8010907795781345208, i64 -6408754357325287349, i64 -1443552118953355157, i64 9221040762829387769, i64 -2709015587941617890, i64 8461015113765201288, i64 5212094301241123925, i64 -5236356903652747940, i64 8713335476933852181, i64 5418317782185372579, i64 -8071631928679715709, i64 559837646340215671, i64 -2742372064934267140, i64 7671493243475074494, i64 6552361834198292157, i64 -1933202640819213105, i64 4573725221129340978, i64 8221895167708853913, i64 932867832534911028, i64 -7037568456296401597, i64 -72142412348534427, i64 -7312166619987848201, i64 -6384656854881469415, i64 3839411406323846855, i64 -1010330549207709544, i64 5954180689385618416, i64 -7694517960106197242, i64 1339066715018438914, i64 -6949129014275368551, i64 1780237478161971918, i64 5484281268637282317, i64 -2697298610245393839, i64 -375646736147422318, i64 -4232935929333703484, i64 -2291422333057896710, i64 550143294033928758, i64 -4513589398216546163, i64 -4274811453938004740, i64 5872544210114169897, i64 7113151259869020086, i64 9011246274232722252, i64 2269765441073143489, i64 7117519436509403167, i64 2636760412624099124, i64 8699288220243669367, i64 -728312789924344042, i64 -2013881997820391063, i64 60670395756942490, i64 4072687779263614287, i64 4238100489621230868, i64 3766541944854727308, i64 -7243445729762790761, i64 1719345326887851277, i64 -7580265919353533462, i64 -3632967855381674673, i64 -8436593311711634108, i64 -7658255147632248116, i64 -5674945767697938960, i64 -6923032071765194006, i64 3690242331998657447, i64 -7419791471009842706, i64 6284095390144505215, i64 7286304364934689590, i64 6495306553848788939, i64 -614410576601459914, i64 3927583826801207243, i64 -6815574378613740617, i64 -5458568851850977978, i64 -678038517993989794, i64 -3725365252101902598, i64 549946045587965021, i64 -7230296440494884902, i64 8149682749811636649, i64 7996368386864852241, i64 -5418521077001738218, i64 2272751375275116884, i64 -4998218325867713638, i64 6283910235119342795, i64 3313159739380252477, i64 -3650699770362940667, i64 -5878642117485876169, i64 799122237680540934, i64 -7277948052740116830, i64 -6531054288896421461, i64 -6621465912587556127, i64 1521807422850783971, i64 -6857282172849871165, i64 293882304077960259, i64 1314136481088317843, i64 1970534131087816527, i64 -6140298750688580707, i64 7121824884161937241, i64 -2634033591053749751, i64 1543154891613666582, i64 -3330617551759023230, i64 3532887726615082321, i64 6482153562006598836, i64 -398578050618633754, i64 -5166514435910649563, i64 3481227329242842205, i64 -1913482695142533405, i64 59825703523405767, i64 -1946435763427140247, i64 -1687025084714737082, i64 712402964784278169, i64 8254958269690226663, i64 478515452133061544, i64 2336575617575429192, i64 2178435689049937796, i64 7419263559649165230, i64 4747486012827069970, i64 3504575793310403434, i64 -4315058475391576690, i64 4217991179178575878, i64 959979885605600596, i64 6613385748468100220, i64 -8333297141318130698, i64 -298460628315290138, i64 5351768241691024851, i64 4323942457860144745, i64 7429139827232263652, i64 6378328163000263511, i64 8702234127207285973, i64 -6638052531422145280, i64 -5378521230687371124, i64 3846867137636593996, i64 -1736702930077000081, i64 -7795784581225495687, i64 -2110290965513971035, i64 -6287519946212514313, i64 -4186512835963131928, i64 -198195512777651349, i64 -7185993262175024078, i64 5011374771245037574, i64 -6494105761039967967, i64 431359576894314608, i64 5940479248123117086, i64 -7692974433371376653, i64 1042452445612159252, i64 -8372814992302898943, i64 7761558529205524071, i64 172365726415273806, i64 3545912880358647426, i64 3917840410751058670, i64 8575046367053031357, i64 3699933340393723443, i64 6417021649418609139, i64 -3686522658318620498, i64 -7492188715700260596, i64 692233158367411946, i64 3492422246424480033, i64 -2457494843318733326, i64 -6677641746225041306, i64 6223381524609624527, i64 4112748820402194700, i64 9025537721198878397, i64 -7545873076889459395, i64 -8368147949779300619, i64 1576284736221009247, i64 -1834802313814968422, i64 -1844434274002822755, i64 -2405149742159855780, i64 -2180797888484668944, i64 -6780380070908321934, i64 7838476620443406497, i64 -288961576135590818, i64 4476046126886154119, i64 1486267007492879359, i64 -6132127035791993673, i64 4397702844799170947, i64 -7068808243943384037, i64 -2002309746494624359, i64 57733815773697640, i64 4203962068139501159, i64 -6586684300670199751, i64 5723559427951192769, i64 -745706267759874274, i64 2721686223996105337, i64 -4649367151301651510, i64 -9039440811286678375, i64 3395418418833140099, i64 3710614928836972680, i64 -1629250300754217087, i64 -7409888309552772667, i64 3168858642089696736, i64 -2719302308754207631, i64 8174863997767615482, i64 -7145567867731899753, i64 -1011869350044778702, i64 -2033414299935084578, i64 140809998859267899, i64 -300336359393583144, i64 1655340333061127373, i64 -2983599449224585701, i64 -5421952589351246326, i64 458641573344529551, i64 -5832306707876134485, i64 5705567148492551033, i64 4520052156658900094, i64 -8595203196983052822, i64 7034534165778239437, i64 4578271240880618168, i64 3887599851482851224, i64 8766599907583596840, i64 -2031697325860911631, i64 6953250605631257575, i64 6695695741862840327, i64 -8792170856209969992, i64 -888709903260600436, i64 -5816513441706464849, i64 -6519092646562539601, i64 -1178112509076312147, i64 8969388284132483150, i64 -932542564772476327, i64 -1942770261979423256, i64 1700763457732239291, i64 990929069152350015, i64 -8666071186302326839, i64 -3812923913958595305, i64 8858291851407929095, i64 2941666248368075685, i64 -874921500502958049, i64 -80519450816058231, i64 -484884700939350250, i64 7455193336729219575, i64 8434831465792003571, i64 7026472933985826535, i64 3186429190894808602, i64 -1701440796361789452, i64 -507985171235710610, i64 561360854328413866, i64 6052538505126875384, i64 3264046216972575556, i64 8608574354222225237, i64 3234414865417084201, i64 -4959405756161527323, i64 -5420773937255853052, i64 -38895433632894622, i64 1342401740002520615, i64 -3905249474831665430, i64 -451447724755563037, i64 -5867190304566448840, i64 -9074311359604941715, i64 8918868937441115812, i64 -8368020656190789857, i64 -3372673122991893437, i64 -3756448987742803717, i64 -2451046447609658339, i64 -5842032948905640308, i64 -4187282347920383416, i64 5431776473991058499, i64 -624701543451828334, i64 3449238878255609826, i64 4413490113388855272, i64 -5155561479165309714, i64 -8422124737313323059, i64 -6963590560489236870, i64 -2021161988806767386, i64 -619210546093104151, i64 -7644592902438368047, i64 -2711663326855519612, i64 -189105974084585469, i64 9184527651843443301, i64 3401983627324906887, i64 6873064239409983575, i64 -8097450540292673763, i64 -8120117888240399243, i64 -2042384676735620153, i64 -6958809750523432203, i64 8150493229721006502, i64 -7351442014933138820, i64 3600501226152423984, i64 6898432467640351165, i64 -3598492515482231819, i64 1952972112295817909, i64 2849583411068397204, i64 -3937806712295239795, i64 -2813090958179152065, i64 -7918149436389237174, i64 -6159559505720818255, i64 -7771367558599466701, i64 -1453098631658092428, i64 4025306302036220881, i64 3504982128882298495, i64 8969654361819299504, i64 4541969618256315885, i64 7795084194567273691, i64 -7216568463375831194, i64 -5417365418014282548, i64 -3222717727878859671, i64 -8509549771184987482, i64 4143183550008497476, i64 -7578820264888172708, i64 -3322470853159711935, i64 2458372001523402803, i64 2118108016941339578, i64 464045418247217762, i64 8573831596884962711, i64 2245944718056618556, i64 3936969236725938934, i64 -1280593371228434897, i64 7334233252827128966, i64 -6728012926112894477, i64 1818976598361649731, i64 -8495084035298421748, i64 2248987398372627494, i64 -1015044995362636549, i64 398117754135785235, i64 -3491355703191666243, i64 7446168987825205638, i64 -5500287550458369341, i64 -8289837818401499807, i64 5145405613625306987, i64 -918131995258412766, i64 -2742141380116972711, i64 4320661457190847663, i64 1155858745994557316, i64 -7719250375857076995, i64 -5868873609553959572, i64 759868434379088258, i64 -2023177052442500751, i64 3918382843920303475, i64 -3738957905691725710, i64 8976152315436593734, i64 -3955515542852450087, i64 6717598140199179159, i64 3500719080253081696, i64 -6188728016779304701, i64 -6026033611594669002, i64 -3546930926551314476, i64 -6003546317121671585, i64 -1780642360034022566, i64 -2462376308504617105, i64 4176634240359381935, i64 3312505968398960356, i64 -3027555046531178618, i64 1567160970007218171, i64 -3905812560725354528, i64 -3808283974163265129, i64 -8593624326282299918, i64 -5633401906914721573, i64 -3706310410706032360, i64 -7394727750707236160, i64 8247385213077034229, i64 5581860068052145775, i64 -1253035621634893967, i64 1293818168892525589, i64 1138117978299743365, i64 -6489308016651330577, i64 -4520757829322695982, i64 6720969102427981435, i64 6876057445634833471, i64 6671821222328847532, i64 -5527104680816525219, i64 -4796041721103542524, i64 2831642126708248866, i64 4773152198382926377, i64 3623729515123084157, i64 -5137224606441549449, i64 -2092180837035968209, i64 -3738172767990061900, i64 3402087781041789390, i64 -5566912940587726498, i64 8514105651433952231, i64 -8919108779481952982, i64 1583613993144919376, i64 8550247371428337703, i64 -6735512022680576654, i64 -5817882840880566088, i64 3902167538820950351, i64 3354942128027685960, i64 8550439355747424076, i64 -6644645884319180634, i64 -7242162108340251917, i64 3144578060159793045, i64 -630678040832191022, i64 -7227525920734942678, i64 -3926299100069311146, i64 7660014090662540521, i64 7902313627362088067, i64 -1443824371105205904, i64 -2800615480451168261, i64 1731225154406432385, i64 -3912339551508951878, i64 -1985531989081878992, i64 5830113412962481508, i64 -840717890878167435, i64 -1629750787817771978, i64 -6644628295735398782, i64 -8146453591664422125, i64 -4186389897555298246, i64 2134997058028579077, i64 7612790744703475487, i64 92057851050539815, i64 8876582470249516573, i64 6567828886448436455, i64 -7309164774767982956, i64 3475236033648148964, i64 -6382079919143225199, i64 -6789707077161745136, i64 4366364666930208763, i64 3868397326074555487, i64 478552232134066601, i64 5743079921663316080, i64 7094560550281243757, i64 -9095854796587580285, i64 -3129668499821013360, i64 3481494266354609610, i64 5452375389030739105, i64 8315614577790512289, i64 5554126380789181384, i64 -8899899761972172504, i64 -1421646416107015862, i64 -2550441693637752862, i64 -2473632082706909054, i64 -1829144209709752353, i64 4762650847677174418, i64 990959405833396533, i64 1431500207023327985, i64 6510459008052257060, i64 -8875774543326442856, i64 6273703158298074434, i64 4147450888437907215, i64 -2000384920721814741, i64 -3044538573643106070, i64 1214417125678839957, i64 -3307883669652436777, i64 -8252615864949700571, i64 8253228095956030651, i64 1232380455731704073, i64 -7029807113714478525, i64 6413069571083871093, i64 9053650230379984931, i64 -4958320570216498565, i64 -1238828352081795423, i64 6890608661039969257, i64 7294695041194295087, i64 7281848898031618457, i64 8923455308701510163, i64 6474011000066677922, i64 8848495813916527087, i64 -4940020465381133289, i64 7777090411874602042, i64 -3960713154384787520, i64 -4093605733183656391, i64 -8607055862208328857, i64 -8242624648851714660, i64 -7214610902420710320, i64 -5168217331035574131, i64 4512570932088008609, i64 -1655315995264307428, i64 3317961152819236441, i64 5052038212203936204, i64 -2449156087541014930, i64 589697903501928011, i64 -2785116081753606325, i64 -3232112087170157661, i64 -5313066534096012151, i64 -1470315176555096627, i64 -6039685514205779682, i64 -3551452395623779722, i64 4224898094285167648, i64 -2445099534003456606, i64 -8096857993653424058, i64 -2237703747522316537, i64 8127043162501084916, i64 2682333814431778580, i64 7355467773642323140, i64 4796329272213754007, i64 -4830496376415839155, i64 -4868604767628680459, i64 2538302058992071680, i64 2920473644680453358, i64 -3413108718764012140, i64 4474066919211159833, i64 -6938514731509033342, i64 -9037327341401383916, i64 3954755742583506643, i64 -7160910291877454451, i64 -5656405950709964064, i64 7867238717789157882, i64 -7329270940326112696, i64 613089664296871018, i64 1077419925598436381, i64 -803382551631663655, i64 -3213530206526687629, i64 -5360623627488929329, i64 -7167679732364785010, i64 -7552714029232582866, i64 7232547363938066072, i64 -4248902792496685698, i64 1451087413793342004, i64 5804349655173368019, i64 4770654546983920460, i64 8661729520359552529, i64 7590400100150709036, i64 3028287533912216077, i64 4671832179137942142, i64 3778326802798327037, i64 -1722276239417655472, i64 7262861475381131142, i64 -8619844124020828810, i64 -3049362209371135437, i64 -6224572037007641934, i64 1917569316206353298, i64 -8438087205743572615, i64 6702225255950892015, i64 -1397506208021256442, i64 4447378433529508779, i64 4630394347844241469, i64 2235745809099718012, i64 871570797224027191, i64 -2703267101070053148, i64 2646996179027236944, i64 4395579446380356120, i64 4261162111743762454, i64 4213543184260148584, i64 -4140348105690984397, i64 2196637401095660892, i64 5980531264816914419, i64 -8297132355305236850, i64 -9101510546111845048, i64 8315987896361824035, i64 779296907477732307, i64 6314492275096839906, i64 -5341049134730755198, i64 2430338658279657516, i64 4837358280677801602, i64 4805718987330311677, i64 3488617665304171904, i64 8527741929320901231, i64 -8458002172866706972, i64 -3019156235979262167, i64 589800946927493532, i64 1856595032421012897, i64 4873058860788469974, i64 -4460488251917037396, i64 -3929636150232866338, i64 -1710552933180690511, i64 5554835905979171303, i64 1273731728183258870, i64 770824994809211725, i64 -5215131996744524943, i64 1025760438810300451, i64 -7404153554944264251, i64 -111005025140304910, i64 1090617310947354721, i64 186249907855728386, i64 1901878249076666685, i64 -7772803052505448953, i64 -3956058129793011139, i64 -1326475802861749048, i64 7613983498222081580, i64 -1542078773145867629, i64 4283215352258186212, i64 3772421390626618325, i64 330326478746697043, i64 -7776621810534548279, i64 8140536968743043055, i64 3489895929539777084, i64 4626332220080986202, i64 5499979185906082815, i64 5718718891571514123, i64 -7825624777206117209, i64 2310666859357571140, i64 7109591342202097444, i64 1883456979374346092, i64 -8554733139488075343, i64 7837909323446845560, i64 6718760749800073600, i64 1543860033744512172, i64 7282500479110435974, i64 -3971875101515377983, i64 -2746793216623710013, i64 -7073845639891406870, i64 3277970209369548830, i64 -651077886431785358, i64 593417898453262219, i64 -9174834084474009169, i64 7261756316139925352, i64 410140938556031151, i64 5215820856443606771, i64 -8625212846352351987, i64 -4357007690273576709, i64 -5124780062491402579, i64 4814378802178394517, i64 8728575190111626068, i64 8022152990655471899, i64 552709484076576646, i64 -4407167323612195761, i64 -8572536561655809589, i64 -3869906514835471855, i64 2644956115570664842, i64 1823343788336589977, i64 -9192050937511359296, i64 -6357885636571842802, i64 3053357890135426377, i64 -4993817728521795157, i64 -8631284627983421920, i64 1489601722984520522, i64 1019485903981094103, i64 4873686281067711460, i64 -1148989493916733879, i64 -384001749439394987, i64 7707051728489797294, i64 5059591509123849501, i64 -3250024219913891352, i64 8401612387104624830, i64 -5636113695292999611, i64 5937845117283385108, i64 -4634585749276833604, i64 3323987210673067961, i64 -3753848320156136157, i64 -5452376773600932128, i64 -3474738222182591317, i64 60954043307651770, i64 4850979754186722641, i64 -3046500260922956801, i64 1714383162915050630, i64 -3790724579445545141, i64 3074435681271440939, i64 2705492382481002255, i64 4829005310152070234, i64 -7187327587821460598, i64 -1784480522049797257, i64 -2733888690541269015, i64 2804556944364039061, i64 8637682414235597163, i64 7440026174419868977, i64 4835661473604417661, i64 -3958346088904752981, i64 -3437663914769965223, i64 -727798028528340564, i64 -663500439547153254, i64 -1461436763542247303, i64 5600826770587761353, i64 -7047540870761603395, i64 -5818756550994199284, i64 408439469317761314, i64 1376570045103434228, i64 -1070109456625309626, i64 4530985898088861060, i64 1489801952523440013, i64 7624711626329512371, i64 -1681985639259344982, i64 -6074350585712612720, i64 963802406301029062, i64 -2806480761369376903, i64 6201210855972618860, i64 -8578073636849934503, i64 6677510804109935319, i64 -1983182666274144296, i64 5284130676435941661, i64 5251013608014326927, i64 3916929790238508060, i64 3844891940461958707, i64 3291762686586798642, i64 1905414623064312897, i64 -7443365187975796512, i64 8464454029699491765, i64 7975510491119672413, i64 -6807234418852366899, i64 -3360667207182148491, i64 2400036395563126855, i64 4933250573381097851, i64 -2123827137736697138, i64 9190241184883973089, i64 6446891745030080134, i64 -3749714368619391780, i64 340505478670041692, i64 2714879329396946643, i64 -526470687921756622, i64 -902885616052155849, i64 -2869056322027245979, i64 -1613879197356682672, i64 8428494382491061492, i64 4560491067687189768, i64 7555888040196816523, i64 -245043477707071314, i64 -3136293611255616649, i64 -2724484145517442834, i64 -4395686196432673143, i64 -3384499310195238425, i64 -8275699759685197798, i64 4638299314205947896, i64 -500985475377355139, i64 7469029930151308016, i64 -2937073287442434872, i64 -7694834210813437059, i64 1469908093901210908, i64 -3381885234854275631, i64 6068593643409629013, i64 -5909332810770112137, i64 -5172881463406990862, i64 -8464779688577489080, i64 699869407368816203, i64 -4084564051145767051, i64 2176771369742144165, i64 -6252510892362022410, i64 -8930210992562318049, i64 6066402922593141078, i64 -5483992286550006219, i64 2042434238026675116, i64 7600203423850243179, i64 -6677359802252745958, i64 -6159708597529783032, i64 -5828097972668169135, i64 2533867416135935823, i64 1300847396005023330, i64 9057751269695407711, i64 -5090232772642765615, i64 1761005480562753363, i64 -5216710187700052506, i64 -246696472653529969, i64 -8141209467460934491, i64 8859905440925943407, i64 -5813106227827176495, i64 4104571498398932126, i64 3367954904367642599, i64 8831008937647711524, i64 -3047451007113899389, i64 -2570553157143909934, i64 5926657432574737381, i64 -6524454388561942210, i64 3451055558675472415, i64 -2144949526632075995, i64 -1829447484647880397, i64 -2388046587394196803, i64 -8075757669176305891, i64 -8765026244401158005, i64 -8726601003668440565, i64 -4096098884062712502, i64 -1195204869348963353, i64 -1297953327966283419, i64 8587728365464983049, i64 -1251839516062929121, i64 -596841529902821093, i64 4129496800119564431, i64 8113083190596834316, i64 4747681592909686406, i64 -1938551556631124199, i64 -7269270417483380466, i64 -8056038371344362615, i64 2740682000476904550, i64 1606903282385887756, i64 838939820026511783, i64 -8464580965152707902, i64 -728494642567415297, i64 6055125791936044545, i64 1812836302149880674, i64 -5106342376165433343, i64 4076132604788239068, i64 3018127816469597141, i64 -76002642256596707, i64 -5965606167491479634, i64 -39861028522574779, i64 5097551009543541911, i64 -1010320484178454132, i64 -8411012008701380437, i64 -2965897181960793224, i64 7216208580576801092, i64 -6244481990250273446, i64 -4698777433701646893, i64 -4226598928214173552, i64 -1383584342246235790, i64 3670414343766246839, i64 -6241306067504314663, i64 5762456942909207024, i64 2535488376263380855, i64 -2662271057966233859, i64 3971009940620860602, i64 1463949640577651016, i64 -8182643862594424433, i64 -2470109057735503994, i64 4948713172524476836, i64 -4352419634996086217, i64 753534686951233500, i64 -3029747183352231346, i64 5869722656344623784, i64 3356029090547337387, i64 4571510231554997628, i64 8874605477943669646, i64 2657316544481940277, i64 8858147773827590527, i64 9160197985241338949, i64 331528987958248063, i64 8800839146198809842, i64 -7910857370568061133, i64 -454217145878330998, i64 4626783331961387428, i64 922497879592505251, i64 -4511409722228502835, i64 5578484452975439552, i64 -7754319099442818709, i64 5142779942897064340, i64 -1264674853204823144, i64 -1264674891790878494, i64 3517180357287016674, i64 -8580713301561955877, i64 -8583018171938439566, i64 2566162912056114802, i64 8849522353538655933, i64 5477333803286531871, i64 6395810011353578618, i64 -1683266842974873513, i64 -4895268253274609659, i64 -4686964891870046950, i64 8397216200768047538, i64 -8780070795420315058, i64 5921522925242541752, i64 5236689445362494197, i64 8343494142300682778, i64 7915497133981923908, i64 -5796575019660926211, i64 -4623209797794278828, i64 -4058740301666779711, i64 1844033621193059429, i64 4159566059627028002, i64 1092433515347917436, i64 -1014084798717620952, i64 -7522867637757227225, i64 -7518779646324840542, i64 -2610862445046730947, i64 -8850426094793750195, i64 -1034693571228877873, i64 2377090283578574787, i64 -8985723849805891966, i64 6618092354129332323, i64 8651515888157030415, i64 5471800627862507028, i64 4535822002454837763, i64 5026832058147137366, i64 -3693763064253049124, i64 1890520391165652862, i64 -1200960592375176267, i64 -8755859515432045533, i64 -653534136617471722, i64 1388162969861002333, i64 6328027347079916537, i64 6478628884903530441, i64 -2589075574345072137, i64 -1207437664705226391, i64 2495307959502330835, i64 -7087574803388241254, i64 -5549600464212098502, i64 1624680532696938403, i64 -4825843657444511363, i64 -1972783221823302840, i64 -3366786286117177429, i64 6644055703656468833, i64 -7660702524210705891, i64 5843649915279232858, i64 -7187099658636620710, i64 -7382989342201214574, i64 -7068935110171872901, i64 2562192298137081394, i64 -842278841027298115, i64 6984247903848452858, i64 1927675084377893046, i64 -3785301927033692399, i64 -9061813983513940500, i64 4830704697017589283, i64 931138072537741388, i64 -6772937652472159989, i64 -6325549898043744743, i64 -5840009696512868666, i64 5410943427574582005, i64 409228085805201685, i64 -8122007358775284026, i64 4963517609952259924, i64 -3380298516561943387, i64 6430347959390083185, i64 9207620562569108413, i64 -8967936719143436121, i64 -8384890962093463529, i64 5625993291120532013, i64 -2038451328794966684, i64 -4401489890630084438, i64 2149710699187203745, i64 5086078781746403429, i64 -4164570966584489195, i64 2207481081006586345, i64 -2566044238723326524, i64 5549948406260174677, i64 1096796979760213916, i64 -2863753830673537342, i64 -8800860188618021052, i64 93503875965412498, i64 1072988352562523770, i64 -843290860547401511, i64 -8010625939162510825, i64 -2248765062600498229, i64 550539103260369295, i64 -8114774824481649105, i64 2016225944390427363, i64 5723543465161011832, i64 1870057919633998647, i64 51050067559353434, i64 -3400527741611515037, i64 5394649668171164489, i64 1250791857769223437, i64 -3772190904559570343, i64 2045521651594262997, i64 3889870677877379429, i64 8494412448178316521, i64 -7036656462334114890, i64 5646908208230861861, i64 -815310086605075704, i64 -2574724853305801506, i64 106928431974856224, i64 -8925114612059308331, i64 -2219995704578242734, i64 -7018209459811468619, i64 1737328483905245097, i64 8497224449220911814, i64 -3748971283611085788, i64 7649601806574977521, i64 5337639897317210700, i64 2784275652233656208, i64 -3927019301624458841, i64 4402429426897072099, i64 -998130958356861230, i64 -8427278485488479549, i64 -7763641739836754901, i64 4580784350322454923, i64 -3124569635046684371, i64 -5033830647702674727, i64 4470590145742836298, i64 -7390720562888913534, i64 45399059127691043, i64 4948916081848051749, i64 -2679980267534210532, i64 -7312269582739468489, i64 -1690139374842610267, i64 2349518330893405134, i64 -3621338610901315236, i64 -5268948618680049554, i64 5527238792684748424, i64 9119551831728693910, i64 -8213057714746475590, i64 -4189956637164349977, i64 -8388746695609450389, i64 4545210491859466907, i64 5322950738797304032, i64 -772026635937001558, i64 5979152030069901026, i64 5706555912505899197, i64 5890611109400406679, i64 1309628610100915783, i64 1835375699899067983, i64 8745070713467575845, i64 -8297488492695683820, i64 -8287877753217500215, i64 8524389332102685154, i64 7876476463650390717, i64 -5567631197993385162, i64 -345736226710713549, i64 -985233443749789346, i64 3733737221493456954, i64 -1663010660433476689, i64 -884516544604825838, i64 3265689502782533405, i64 -1037932348810505077, i64 6815034803539516993, i64 7544280512996896057, i64 -954443393246201523, i64 -2890777097115740498, i64 2267981723291022631, i64 -1834160318219926642, i64 1607359115969851706, i64 7264671805826931925, i64 2094487225013856876, i64 -4548811924670372616, i64 4177421662439383329, i64 -7459403345913690698, i64 -8812572028645274645, i64 7808498429726703595, i64 -8243996048940725737, i64 2206675453608158725, i64 4189701429280511518, i64 -3706066405652046012, i64 -5844112828548066203, i64 -8098533444442629929, i64 -4279393316966891664, i64 -8548696262476068809, i64 4547896040136928299, i64 -4377437751246119875, i64 1938485777506441835, i64 7456851916464344075, i64 8038730864562887744, i64 -4358656658816803567, i64 -4015007711465692852, i64 5502362367099295005, i64 7652906512665566755, i64 -5699613018823255697, i64 3020348677078206782, i64 -6449824193401968502, i64 2898402016771387499, i64 -6167265669156878918, i64 1394533636670127164, i64 -4668562067913501606, i64 -4294727806594023490, i64 3453525188859201450, i64 -3114778398708120309, i64 4011863369933119354, i64 632216490609820733, i64 -6750356320054060738, i64 -8918362266764289454, i64 5549041832476454307, i64 -7781409404652407665, i64 6916682127469727295, i64 4387974566966429639, i64 -4749148804850983541, i64 6934180056566945093, i64 964380669333241817, i64 -8184555300296516442, i64 4147627714220149224, i64 6162498026852276405, i64 -5468277479816085041, i64 140168240868299530, i64 -9214595120797138820, i64 3534161610573338040, i64 1211839205266980182, i64 727724412601575402, i64 -8579000307238509616, i64 7715665885736861067, i64 6608902878373136565, i64 3790864201861787458, i64 -8511159293584800179, i64 -6922042110873710442, i64 -4688811841881931858, i64 627997323961335647, i64 5414436320499813677, i64 -8123860290996123672, i64 9060605147727367757, i64 -152012138536386758, i64 8812894175405631427, i64 1386980671332217419, i64 1077163780065819002, i64 -319091864399386552, i64 -1288286577033636035, i64 -7734672348438446722, i64 5837860940664402070, i64 -3929114164262841911, i64 -7577057363927760010, i64 -127412731867601468, i64 7141309119182457018, i64 -5796121599261349285, i64 -4815252131736371640, i64 8036193083129750410, i64 -3284865280920355995, i64 -1873110602824234973, i64 -5263540989694123913, i64 2597799374818372610, i64 3156864824409593630, i64 -1930067339197721138, i64 -5399594682853233868, i64 9036073228766422478, i64 1133365774395974812, i64 -3257839222078266641, i64 -6747437460057405524, i64 -3554695530074031598, i64 -3459056013438973001, i64 6498825096619495998, i64 4008858528633873818, i64 -667654763047091844, i64 -4281070973032012132, i64 -6715024558512854671, i64 6468168193187585684, i64 8804805711670845254, i64 4146283808724860543, i64 5339845452122418329, i64 2794593665073559016, i64 -8219001981227317757, i64 1830473773182444839, i64 5496355519111358611, i64 -2501322023196243046, i64 -4110617552896293113, i64 -2412506807431309615, i64 5467236605150507639, i64 -7894153743689626034, i64 -6860177644389728197, i64 -4745468244846466176, i64 -8757330700274955986, i64 -6575300484675271821, i64 -2171676781859062711, i64 8249613146313461086, i64 4928315355621573754, i64 -6831441935740795447, i64 -3404235250259031633, i64 -8589868340210839011, i64 6741974535606708035, i64 1315469220930663695, i64 8250363021041107706, i64 -7210744604447808823, i64 -8972312910757825461, i64 8823248627121984807, i64 -7988392035612305040, i64 691195051567990543, i64 7804054840595574386, i64 -4517053861395768247, i64 -1581799830077864819, i64 -6550812734071295273, i64 2984585396039145253, i64 1378536820140537737, i64 -6276725665601466359, i64 6482776332435241631, i64 3738484411447178482, i64 -6120271444844897648, i64 4111237021711082020, i64 8008686528021301779, i64 -4471749810179299484, i64 -5751241270648657888, i64 -3033555150642767330, i64 4112400658523816873, i64 3900607542418181213, i64 2437261425607112876, i64 -2955590219259001060, i64 8716982022204529000, i64 -7808865550766477638, i64 6116534094158582743, i64 -1615474805395451839, i64 -7143524198568897302, i64 3147188031756947610, i64 -8243915921799306035, i64 -74874029589070414, i64 1646750288903795171, i64 -8619736891935586384, i64 5971327283898063978, i64 3171571604306310197, i64 3620494697316175737, i64 -7288305708196204365, i64 -8610277266968706332, i64 3310686637752795424, i64 4328219124032694133, i64 -3826799545794147321, i64 -7054328237187735600, i64 211866202849762667, i64 5866046729152121690, i64 -6069973155563043498, i64 4521361534008905639, i64 2868403584631795485, i64 684158355962985050, i64 2611305695464695231, i64 5360983893415590514, i64 3613410574642793169, i64 745545476160403908, i64 -5369123810003320218, i64 6298413688667620501, i64 -5673185202937686350, i64 -4446212531641307735, i64 3432059337412598930, i64 7083118996486248781, i64 -6334754599965986229, i64 -3755826173737943808, i64 -4150523428953869823, i64 6283183272149492502, i64 -7289302110192851554, i64 -8376517305071242971, i64 7831170950299989977, i64 -6470474972799250074, i64 -5330003589430485220, i64 -4456899647745943305, i64 -7290667219095991853, i64 -3438232078452747558, i64 -7516865335550220853, i64 -2014760396444246765, i64 -4311384718334316486, i64 -7469425916497715351, i64 8953233355111433733, i64 3452506032496836282, i64 3335317178846438688, i64 2944393014719040281, i64 -981876194639160000, i64 5411125962295404110, i64 -3006999554996244762, i64 -9054426242146316163, i64 1436253087509658777, i64 -472651328907120999, i64 7732271431957006642, i64 5565612552896367145, i64 7388503673931462774, i64 1174991746340530334, i64 -5555668957489753394, i64 6387219046794366076, i64 -1404358094195385922, i64 6868155402583373534, i64 -8932388404207709621, i64 3691525080144947765, i64 7253547367265891722, i64 8513114905296673902, i64 2266016165894964463, i64 7675862973209821592, i64 3671150548134654082, i64 -1217780792833691247, i64 -6600444819768175258, i64 7393123464773839664, i64 9013507476716449611, i64 -787177254888802029, i64 1829301006409815646, i64 2419884055255682980, i64 2447629486254530995, i64 3757196808647196453, i64 727783344876923701, i64 2266317118899329565, i64 -7618381964790453919, i64 -2656645906314540572, i64 460730239695948542, i64 -3526908888075449967, i64 -2544186554018536258, i64 3018845653338129426, i64 -1652191423952454409, i64 3211931057468553810, i64 8591762655356194369, i64 -3130941483529629186, i64 -7975182606065543978, i64 -5361793821683673670, i64 2985754805423230997, i64 -8840895198176162615, i64 3752230432477322381, i64 -8922811667361119057, i64 -1191248201377554832, i64 7506441036923661854, i64 -6688789166286071374, i64 -8225387014542184803, i64 -5114818225778264679, i64 423582506820201420, i64 2985198705600315495, i64 -4462620610788459653, i64 5356011749413368751, i64 -9113269783429902503, i64 6884880715704256642, i64 -1883167988539789358, i64 4537827435442360376, i64 -5985693983999910851, i64 2660456557577710215, i64 8076079747219441332, i64 4818351319685153913, i64 7515230567739315003, i64 -2818128317451338890, i64 -1522979333930553698, i64 3295941329086047926, i64 1409593637853838753, i64 -4281035424109079947, i64 -5769398753588242740, i64 -506394351609384461, i64 7906692907792269033, i64 4781761854392331432, i64 9186475738824645691, i64 5781992927458459517, i64 -3924555605979680868, i64 -6361557304922758867, i64 5431983441955761211, i64 4551427727798527842, i64 -5270799623896621538, i64 3757611985334767961, i64 -4594657354964468296, i64 6519868303823297614, i64 4405036056246968907, i64 3586630627278683440, i64 -2411274301795774813, i64 -585945522063311817, i64 -6242619429217326278, i64 -8597747077989634342, i64 -1093112431312731074, i64 -8656917438400536677, i64 -6237155315116398833, i64 990426415632171604, i64 4923758351647091385, i64 1722382580794524100, i64 -1089312665900259134, i64 4529991714008273897, i64 -2732860509428150531, i64 -2464384223866797843, i64 -2580239612926195856, i64 -7244891092677198811, i64 1217758122852196893, i64 3928765808847627810, i64 -2135915825986572358, i64 -8920040810347088096, i64 -4616009572646669512, i64 3265546018554818589, i64 -7849551409138766391, i64 -2530466098107235831, i64 -2202470396958306624, i64 -6550637113855061013, i64 -5763639144714194651, i64 4243547160102723246, i64 2313222702173407489, i64 1944957409898403246, i64 -5942926916079068469, i64 -7866489814968294364, i64 -191530514886066084, i64 1855436303556235683, i64 -2700530914357625259, i64 -4222274094130224765, i64 6803715896968204203, i64 -7582786965174943505, i64 8064758742024411332, i64 7588183045487928914, i64 -6301812492868326311, i64 -2948592104174937025, i64 587602670861825906, i64 -8157809761911288022, i64 7237962356855284254, i64 -3933260672282823989, i64 4189480853872793286, i64 5915513851469194335, i64 -7416358997786267384, i64 -8788077891875516545, i64 1187582595614286092, i64 8721380222891065382, i64 1475828158780572679, i64 5802065828061401410, i64 3244954174175658530, i64 2903767459460230513, i64 4623526876112265481, i64 -3922438394051159257, i64 -7995468045733910974, i64 -9074536361884747911, i64 -1411278632248437048, i64 -4719598453837315038, i64 -6183101211477567225, i64 2401764989412601573, i64 7053798320965805523, i64 -329122101415578798, i64 2454712777655781475, i64 5985292332811661426, i64 3608733048065202435, i64 88520443954243166, i64 -598585408175924290, i64 -7719802450565235611, i64 -5559532675361089433, i64 4133505074757564184, i64 5235317715995583625, i64 4690730492496915222, i64 -3042822326858924868, i64 4851732532581092131, i64 7624354865680031587, i64 -8304965175775379164, i64 714945404142599894, i64 8181683252198746163, i64 -361060236797271793, i64 2048885714969391223, i64 -2861201042452991271, i64 956340481528429499, i64 -7608018838296518998, i64 8061042159036085457, i64 1462010287562428407, i64 -7942370452123634985, i64 -1173854984536297069, i64 -1550067424985355779, i64 -5824082145858858062, i64 6465557477818896758, i64 -3566621244501247416, i64 3045814115898450537, i64 -4160638239823870942, i64 -8476954790837118669, i64 -787980277641927113, i64 3906261090368168845, i64 1521287419700451990, i64 -3449455008420231097, i64 -7099594619702660569, i64 -1688405988931235558, i64 2748408089781363268, i64 -6579093322029134823, i64 -7739290342229608368, i64 4865311547563920584, i64 9090606399427089454, i64 5604893119035737573, i64 -2014357480350191065, i64 2644000044031028143, i64 -6322444676342954974, i64 2617254040057291058, i64 -3903580569521578622, i64 -3172604716266653848, i64 -5348803807662716643, i64 -1549541040943085780, i64 -42908372637826379, i64 -6434527344247788364, i64 -4465042946881246985, i64 8627848359720876538, i64 -6533918500866675574, i64 8934127842021063823, i64 2921419711342116633, i64 -8714494609834888065, i64 -2238699370874228964, i64 -8698180273160816827, i64 -8926397464901201283, i64 -1993831828119237444, i64 3029721107045449915, i64 -2046439555631081415, i64 1085296899105624227, i64 -8875147869674521377, i64 -7340568645238672919, i64 291042892467497214, i64 3672222202351278003, i64 -2522597207706125261, i64 2414482725296310215, i64 8664474749856628915, i64 -737519450218553903, i64 -2737093916687620241, i64 272004691409873881, i64 -2073570684817612598, i64 2745693179400702403, i64 688777156593426755, i64 7212209724755292138, i64 5762237302634287150, i64 -4740737565395988361, i64 -3997580409801858843, i64 7276881630256101156, i64 6525742231603877993, i64 1081018013028535062, i64 7965872055441164692, i64 2967615206298176601, i64 4145518929581764207, i64 -5127861962712048892, i64 -2305963787725252916, i64 -3382994138526958049, i64 5539403125544794086, i64 1040532522698225638, i64 -8330501849665872790, i64 -1835116322755503098, i64 -898547326749519571, i64 3428098638006844401, i64 -6841748988977727726, i64 2883912733450242142, i64 -8389016550914972965, i64 1205153202268777925, i64 2933790191345346954, i64 1180723662790146520, i64 1246741224935712163, i64 1715909091710277461, i64 -7305544610450884184, i64 7539658219852838117, i64 -2417956452411546554, i64 2901944460875846736, i64 3135834047410505411, i64 -7144642991003978725, i64 -488820595748048183, i64 -7795361253077700554, i64 910524610387741029, i64 8091803955477926673, i64 -1762259149320142568, i64 -8307905530731715986, i64 -8690604097503402489, i64 3586226559618240080, i64 -7743171843598511646, i64 -7242630565374789450, i64 7993084029651694294, i64 -5078675268848119908, i64 1473480900012862299, i64 796942174524349765, i64 516468959287475129, i64 -7772716864937253863, i64 2313165472134592019, i64 795152289204646469, i64 2265855482631789634, i64 -7577092075211753680, i64 -2618906789154606619, i64 -3936922116465134855, i64 -2904110871362280041, i64 1782145951316846328, i64 7551116430396908515, i64 -505453416020655372, i64 -54597390777654798, i64 -5431096613764220014, i64 4207498715347142444, i64 -6727547780516628198, i64 -3691378755375695895, i64 8680660270195085445, i64 -5388306011900592215, i64 1769189200922161509, i64 963221814031946428, i64 -6121981456275993656, i64 -6024551363609455245, i64 -967207771004255839, i64 1843591589704041758, i64 -4194515401699625246, i64 7311213148192884304, i64 602792240783734407, i64 3416859817171983217, i64 -7914938165276025386, i64 -216262089388187626, i64 8495800345264903087, i64 -6666618381764808912, i64 -8318331144144863572, i64 -1085934329246765034, i64 -1640444833774038597, i64 8518926396912952109, i64 -2228076200469131129, i64 6518372279598431137, i64 1500012401558717672, i64 4573087041632142493, i64 -8874350385645924345, i64 -7216935637286328938, i64 -4028099934244051785, i64 -7391047759368677589, i64 2840019589157678912, i64 6253156645868599702, i64 -8147540462443491928, i64 -3994818083101446169, i64 -5276645616507733305, i64 -3199919793228844138, i64 -3344231939740792283, i64 -4731248254082222362, i64 -7838438661668415769, i64 8638465884173168089, i64 -5499113014533423452, i64 6499500585276576609, i64 -1455098277778235683, i64 -3695438982254825584, i64 -449308985258679024, i64 -3966459508167905353, i64 -812503617041798359, i64 3661427933865134675, i64 4643401559433296851, i64 3573265477570378722, i64 8974701636023104155, i64 3645209998176427129, i64 5794993607856461596, i64 7585665499559185889, i64 5631911803273136313, i64 2843900895795769802, i64 -5561074612283568100, i64 1144964259939282328, i64 -8424445575365939488, i64 7780962022158190173, i64 7327987357417057562, i64 7026303511992586545, i64 1475009756708572468, i64 -3948338365922796435, i64 3769954656068540998, i64 7804240103596497355, i64 -8312035156845672505, i64 -5447700022203696903, i64 6894625895159591393, i64 8445259042016958698, i64 -5503190965982819047, i64 6443887345575350714, i64 -608534115614363092, i64 530669712812647549, i64 4352112425990170339, i64 -149448410044377257, i64 623562907715700642, i64 -8797370716693608052, i64 -6692339728797632510, i64 -5999506092197318971, i64 8838693035388748168, i64 2064700325096813901, i64 7519114750913441886, i64 1510126638583127430, i64 1929685833837087705, i64 6546801644835396923, i64 -7391646755007520442, i64 5210821476282833560, i64 5786955796449643843, i64 -3315043853274212357, i64 -4637407017309379817, i64 9058962533832561645, i64 4628845601332250926, i64 -2459004713758271912, i64 -8979540677143675585, i64 1654679284982788817, i64 -2003314631116128340, i64 -7881256933808892048, i64 4278115362046415194, i64 -2049901350308254178, i64 -175043372698117346, i64 -5209309794510760338, i64 3991292662651802572, i64 5392668479319545329, i64 7456509015429608631, i64 1667312334934742768, i64 2997034472983912402, i64 -8306461446880829252, i64 -1703125078161385808, i64 5696484647792044526, i64 -4300367395551457764, i64 -757004402747750214, i64 -2294001846721101509, i64 1467176809464355075, i64 902406844616631797, i64 1138924087180053316, i64 6220767605267117078, i64 3522022393546122971, i64 -4018505617289991753, i64 -7255663168788277535, i64 -3727301899434217167, i64 8217275622823626898, i64 -2479835404904504403, i64 2999078033077512606, i64 7473553288924970427, i64 6479089551771759754, i64 4591409992695848613, i64 4249584042778731052, i64 1601537940012432084, i64 408067991374295092, i64 2820016593984055783, i64 -5383663237753599203, i64 6759898048740932863, i64 9181187150441490979, i64 998006456551897652, i64 -5987436788198779148, i64 -5204397593836056157, i64 -1805478921549313198, i64 5838539528927975812, i64 -7111178479040236997, i64 -3379719363483099098, i64 -3413115380608191342, i64 473966380901126795, i64 7073875532171241055, i64 5077534740974415111, i64 -4893575037598720821, i64 -4184065977032147715, i64 -8297309115270871442, i64 -6340755391052412330, i64 5247612011024812756, i64 2642806650174089317, i64 8652321203113846067, i64 519433830432481563, i64 8323662612839750252, i64 4591047176301864530, i64 -7458394496703304994, i64 -1859682818517608812, i64 -8090828452777343966, i64 5768096775402375645, i64 -4595618042426870372, i64 -8112633347465621415, i64 6603219287600840188, i64 8768875012230253505, i64 7504673059414177327, i64 -8880779497900288557, i64 5399065322588200966, i64 2501228667591925058, i64 -4568194858739069417, i64 7966148744733595323, i64 -1994694701544804731, i64 7752353228049705004, i64 261310657269041780, i64 -4775828619138458871, i64 4734698134616369671, i64 6039710386107026144, i64 -2224644300668063140, i64 5979188944054195467, i64 1714211631955330245, i64 4130162149499791929, i64 -3862830452335682851, i64 8535919503200628286, i64 -6553911539433954321, i64 -6034665927523268230, i64 -5939243664884871362, i64 -5694288927184760955, i64 4999756056817018850, i64 7157399331220805228, i64 -1265523989520403838, i64 4447823481374601326, i64 -7331399075370977754, i64 -6564498415454820335, i64 940217604300253469, i64 -2435300231146562441, i64 2170276452543255665, i64 871732130855415695, i64 -7099599186336088807, i64 -5558601237297784912, i64 -7282230971174139096, i64 6435663256014360795, i64 5003370235644840448, i64 -359096542419167380, i64 -1023450716126083763, i64 351829987917142374, i64 5002457378133630818, i64 2966617247572208334, i64 -3033937456664670886, i64 -6823369200417329847, i64 168862285450106754, i64 -7166677695192522988, i64 -5009139727862584000, i64 9184096791143846570, i64 3429435373073048518, i64 -1131946751198841056, i64 6784697964759486032, i64 1524031040234592333, i64 2659777659890506913, i64 -7734953168488456150, i64 7183546765588191536, i64 -1548788223981759453, i64 -813340361235453476, i64 7432573872584630584, i64 -4192064940366165173, i64 -6639569268237185137, i64 7097741689532254249, i64 -9053449708811047209, i64 -4400397124205852641, i64 -4536944847581581658, i64 637397648122965604, i64 -5326552123987249131, i64 -7742214384347880582, i64 -6682447473790409128, i64 172382552543432780, i64 1437642029806751581, i64 3121647261748091784, i64 3438448674008774258, i64 -913869108603306465, i64 -6986302055885594047, i64 -6638481910505448179, i64 2501507948724491058, i64 2868460388058688374, i64 -2068723750457647291, i64 3133681389894866264, i64 -1280915152357933497, i64 -7407529539612646319, i64 -8866124848154298361, i64 -4762902847245580026, i64 -1796116264115335720, i64 8188743876082414909, i64 7580228773919282179, i64 8921542438640400831, i64 1561923835038153795, i64 -2617568633447812175, i64 3178577471255986971, i64 -5876975370575829737, i64 7283515982764167336, i64 1192781843404465895, i64 -3705938548344083256, i64 -6929897848145240074, i64 -4964786833533910422, i64 8546246508017850832, i64 6486739625852204530, i64 -7881353676227071672, i64 5600036093566943351, i64 -4065601435783641750, i64 2725296650104771848, i64 -9091554165795592200, i64 -3776115629194428571, i64 499582149111472657, i64 -7166681073705917162, i64 5839139936294946126, i64 7285013905522977049, i64 -5984279662610874497, i64 5634480407326585096, i64 7247056127982003477, i64 -1781719352519940642, i64 -2506115043507496618, i64 -5924001595131723054, i64 -1623175312356103736, i64 -9001731673998226266, i64 -8293859351769562564, i64 1497364836644021302, i64 -7420196764741287631, i64 2566078561214450850, i64 -5436279617418221499, i64 5312993189972954075, i64 582094798409581929, i64 -3972489221808453542, i64 3945960210678283523, i64 -7808963812996731587, i64 -7609621160946771289, i64 8371605866317104095, i64 3561959428877030618, i64 1937769004867575167, i64 -6730164861941628195, i64 3018788712103014284, i64 5138017032339634498, i64 5057255937971280484, i64 4302703256690651691, i64 -6119971052119925672, i64 2422447113017047317, i64 6099219107867686762, i64 -5279835332471293548, i64 -3986160063701511032, i64 -3120227156674060223, i64 6858032674428899951, i64 -6313395626560811891, i64 -8747998461904554016, i64 7684296905077043850, i64 3875929594717622491, i64 5715215668176340018, i64 -2363801551458223466, i64 6705186536683135488, i64 -5358087825693839630, i64 481867954680956706, i64 2863607333845340491, i64 6239884922525794999, i64 -4632292913983343379, i64 1819463524813558415, i64 6074464939862068855, i64 -2326284833357390604, i64 8899482917038988718, i64 5685322678794324081, i64 -5274645054020896997, i64 596688889352957344, i64 -1385286131611124010, i64 7513613081401791428, i64 4815874214143241285, i64 3646520517518694903, i64 5359172249652091874, i64 -4765655560224345320, i64 4913999326061474326, i64 6436368509718929075, i64 2782910492069769214, i64 7056921059990544988, i64 4450872217133331565, i64 223390264929196237, i64 -8669242240451369388, i64 -7357423262247712817, i64 4756434016057800048, i64 7611757626183682661, i64 676695222896462276, i64 -83658163342530696, i64 5878907434982901541, i64 3257263225549035898, i64 1818404676629008842, i64 -6429360432234110646, i64 -1955955464179951070, i64 -6596724156515761066, i64 4227618558497826547, i64 4058850938968798701, i64 -5864589691001675281, i64 8829418585398927059, i64 -3551678358941249857, i64 6652864773912674518, i64 5523673519124474694, i64 582815282509902640, i64 -8598100611848123646, i64 4597497454356153647, i64 6762894222602821589, i64 7662341040316648328, i64 6329853046794545824, i64 -5250433388432301649, i64 -6507875191012813041, i64 -3535706450404758074, i64 -6351229321170063002, i64 5105346517644320707, i64 6252234189667229579, i64 -3662689680182940444, i64 3060933404201448381, i64 6420977951651283365, i64 -5366624209940557045, i64 2932108432619954712, i64 7265900219658564277, i64 -2501776568173732383, i64 -5992617580259618387, i64 -3632980373186644107, i64 -5885227395403057867, i64 -269712473833237682, i64 3697072164824595446, i64 8440585383379599807, i64 -1527500144599583790, i64 762104620809865201, i64 8802953596623941409, i64 4739818726133685373, i64 -2034124459113289071, i64 4298632886435160739, i64 -5117370475999758609, i64 8577183538953770116, i64 -316494145139365311, i64 1482087752450252607, i64 5914039819598199138, i64 2812795899271439531, i64 -5092285918261720071, i64 7607289810790824466, i64 -6097279120555288972, i64 2541032705608310611, i64 753321833745150794, i64 2449489085309422361, i64 5847824965165551767, i64 -6552033932343260872, i64 2669388691784953738, i64 3057771155898935031, i64 3065924271467242747, i64 -840939960700491714, i64 5889770285138360644, i64 -2921383571305563052, i64 4679614536048616707, i64 7660689302600203177, i64 -4276274805233073929, i64 -7683346144833986151, i64 -8821843834243858005, i64 757868427551250555, i64 -105083122151518820, i64 2063596183054079787, i64 4631386855939061336, i64 -2735380527386081658, i64 -3466354961547475020, i64 -3722410716748056213, i64 -4875868658586193237, i64 6936249860186312799, i64 -7712349470327555704, i64 -3393626963755742125, i64 -6788130301720104211, i64 6542878683315375635, i64 1618322019788373262, i64 3405712976631564537, i64 3441778999006716444, i64 -8715092746165265497, i64 -9094204336687691826, i64 5004078106734431882, i64 6307703778412309403, i64 6310711120153913844, i64 7764311633278473543, i64 -1553921825770158208, i64 -199146325175169332, i64 -2253863148597052303, i64 -5526498566417038032, i64 -2003276865722408944, i64 1012137754720033206, i64 -7512991278306157264, i64 3863253689916184299, i64 -6134804835428182603, i64 8608877707298913528, i64 5269394183869773251, i64 -3562886881796715859, i64 9096668957472158293, i64 6602766625252883360, i64 -4864541597691761738, i64 1928347933940630870, i64 -6424999940526942438, i64 -1695875047983299211, i64 -7870164822636501164, i64 59251421120576584, i64 -6511922809995694747, i64 -2608345875789479933, i64 4855290955318750878, i64 -2407971498230307496, i64 -8619386803839967306, i64 5800753266289312575, i64 4589242253092060037, i64 -5152979572085383876, i64 8979750724994209275, i64 -6092618803564964772, i64 -1002972935136919039, i64 -8728474983520310477, i64 -8453941182203747327, i64 -5115569398585407289, i64 -7619320544461167576, i64 -3429678296709064958, i64 -4837592502004378315, i64 -3742606223898469414, i64 1531795067619321215, i64 4725605900447649262, i64 -7853942995086382405, i64 -6851397438528771366, i64 -4279770226574913363, i64 5495423029090960726, i64 -8939911419930447126, i64 -6135116265756332573, i64 -7152704350049726226, i64 8456301433010607523, i64 982136549284107009, i64 -8262457056649603339, i64 -7782456232762576155, i64 3957914321761812562, i64 -8802730467942920532, i64 -2237563338541628964, i64 2114983023498110347, i64 -2599828635648982326, i64 -8008058678502234161, i64 5858254532443211381, i64 -6095431703559741795, i64 -2002618740799818622, i64 6353035540654788964, i64 4160125108349446991, i64 4551541417433096221, i64 5226227983205447079, i64 -2693124088931733791, i64 -9187228587241352403, i64 -5153754508492088137, i64 -7261961688106790076, i64 376891273633453872, i64 8576246948683639160, i64 863060873277232153, i64 864152053228483578, i64 -673483215745560774, i64 2032174138724896420, i64 -8789304036818558567, i64 -3692269029959329707, i64 -7413994767081118608, i64 -7896024748527104927, i64 1902675350861489818, i64 5300321777867367994, i64 5509952141718505283, i64 3592414978137029266, i64 4447935890751401337, i64 2520063817810008943, i64 -7553141492809027120, i64 -5859732370465851533, i64 760113487602946386, i64 -6589764012145765529, i64 7523077386804773680, i64 -6418969246007835613, i64 -3158748590584468578, i64 2393076897123292645, i64 1506830361300264718, i64 6433696535082099017, i64 8858903835687781690, i64 -7841986349938856392, i64 -3074242505360749541, i64 -6479722128425748787, i64 8453437708365583082, i64 -3031354280857155063, i64 -4259619105677661755, i64 8824737308636302102, i64 -732847146579564710, i64 7449952402622878641, i64 -3692269027346296051, i64 6721166478972021672, i64 -1333352476420588769, i64 -1707808001070092450, i64 -6775552097878374420, i64 2284285031189904435, i64 -547394104030117062, i64 1975904595938390965, i64 -2787513328730412298, i64 7296690755076510768, i64 -2787150285582740349, i64 -4972944214544985040, i64 -8370115082466795695, i64 4374222090169563404, i64 -4378701928360777143, i64 3593551398169896442, i64 3679629085790549796, i64 -981991253388831078, i64 2931171962541499804, i64 -6780200653167924907, i64 -3351689149595716623, i64 -1737462296841653541, i64 5147128359258681761, i64 5540938363976947179, i64 -4422087287641462247, i64 758395766823757925, i64 3123651685123025035, i64 6609843200017229148, i64 8218854305536308705, i64 3196555245290393404, i64 -8948902551052811045, i64 -2434156810263081188, i64 8226553831520515818, i64 60098675752618933, i64 5912308570929386699, i64 -6267629091834164035, i64 7343840909782489034, i64 6554262237340035733, i64 -1547094404819977265, i64 5833211146910647246, i64 -7604516825701887651, i64 -1965058281684349206, i64 6673487692979089294, i64 8803702702662894873, i64 630427012313501757, i64 -6109805692751578312, i64 -8309126639951467675, i64 8532646442194851638, i64 7641693508271395865, i64 -6864834905501527690, i64 -7775084665046009872, i64 -2601736467640746835, i64 1122817143983112379, i64 -7775190457213786813, i64 -8613194755148458960, i64 7430145606938724568, i64 6193866917058729449, i64 -6835656128659936511, i64 -8656463463642419260, i64 3339530224132506341, i64 -6267869671148920428, i64 3155366969272959717, i64 -2090104481893176172, i64 -5181441599297101878, i64 5407876553548704587, i64 1208284982496998028, i64 3447438167324625279, i64 -4394560219861938523, i64 -8258769558405795884, i64 -3534272220637942968, i64 4240137340575851290, i64 1970162746743698342, i64 2428978180885175077, i64 -7528885162323207202, i64 2150377526240167557, i64 -477839169375850980, i64 2431041738152526635, i64 3022139145615437982, i64 2513746419012584238, i64 3223488311963345567, i64 4820293118949866054, i64 -5761505278848604897, i64 -4346368845765012129, i64 7638763134845757162, i64 -3493000609180181256, i64 4701506716317915513, i64 -9131890128799897190, i64 3092167371107451282, i64 -1616509323959474356, i64 -2665292648036274886, i64 -3903222995741936469, i64 -5587491767362880594, i64 8911858547393896171, i64 3625442369001761322, i64 -4287238805174940872, i64 7722825545421700316, i64 -6555181867532130133, i64 8144359829550010017, i64 -4515626734274993689, i64 -5441607263665712117, i64 -8828586468224806194, i64 6154271822480409942, i64 -3818077937579643657, i64 7314333067711701102, i64 7752320415311747665, i64 -2781168428947724272, i64 384650285630041261, i64 3674561008363214372, i64 4243177066035971283, i64 -5565413207529780436, i64 -7337011095551222184, i64 8434342790767713617, i64 3444929770096885174, i64 -8807319374690350890, i64 4227906697564196178, i64 199194951159623724, i64 2343857221102407355, i64 6007899163829253837, i64 7051276984362741944, i64 -4287239776467371163, i64 -5273928239736742794, i64 -4076867914493208678, i64 -8262721934367068799, i64 -8237021229829924093, i64 -4076691992517489951, i64 1185388491202817429, i64 -2937203661432395301, i64 4206478406978555115, i64 7688719239928564749, i64 7602540899030439962, i64 7307604118701780692, i64 6072736303988017466, i64 -455617003832899327, i64 -6507489029343953909, i64 -3140860266381736111, i64 3280685113240931169, i64 3473601449727502989, i64 699716089437103708, i64 766233761263442674, i64 -6763573770331337400, i64 -2845230056686813777, i64 -8598583994127429376, i64 -8031401279577461670, i64 -5032059364236042921, i64 -8444997721156866728, i64 852354800166967132, i64 -5881342088326129620, i64 6249172582596881109, i64 7010487390071107755, i64 3090829899105673902, i64 -1697522274977443206, i64 -8616580497417876476, i64 -8492182130390624857, i64 -5115465991713527216, i64 -1659292614323244960, i64 3192595055762846304, i64 7565345658125115671, i64 -6643581133366049680, i64 4142960806745444638, i64 2383562810579984936, i64 4216974915135193116, i64 -2567734004864219026, i64 1939663623953234054, i64 1973781375699133076, i64 -7128835245015942703, i64 5767704970230392995, i64 2812200555726341378, i64 -4221166015901579006, i64 -3276854076587538326, i64 7584953794670776520, i64 5347186287473141860, i64 -4722116445612693195, i64 6347936996745211728, i64 2051767984562117753, i64 6797628514967058776, i64 5483519618249420139, i64 -8342079727568078437, i64 -753016110978213686, i64 -3012064435322920145, i64 3859801360697765055, i64 4005788905079670639, i64 8435191795162617298, i64 7112810987854660525, i64 2181754735123164050, i64 -6513324837966673876, i64 -7850323372090727003, i64 8954358279066784558, i64 -7073769543567600217, i64 -5579608338066316119, i64 9001081800251049595, i64 1441832052940678971, i64 5735381973503655935, i64 4274279816896685432, i64 -1936041839162850785, i64 -4800994016631334278, i64 -3760681950964695394, i64 -757102834588385377, i64 6849621845974286298, i64 3168688983947830063, i64 -1452977247310922401, i64 -3271172493874395972, i64 -1858812097775030441, i64 -3929272056490440571, i64 609922017123043700, i64 -8571632050159436592, i64 8240824696094822536, i64 2891594345248920652, i64 745926910565261450, i64 -3674886835527410060, i64 6374763011950118480, i64 8353254553892256143, i64 1445092132488111000, i64 12930186], align 8
@df.arr.4 = private unnamed_addr constant [2 x i64] [i64 2, i64 0], align 8
@df.arr.5 = private unnamed_addr constant [2 x i64] [i64 3, i64 0], align 8

define i64 @df.main$2edart..worker() nounwind {
b0:
  %gcq = alloca { ptr, i64, [3 x i64] }, align 8
  %pilhaq = alloca i8, align 1
  %area = call ptr @df.get_area()
  %ctx = call ptr @dartforge_context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load ptr, ptr %pilhalp, align 8
  %pilhab = icmp ult ptr %pilhaq, %pilhal
  %pilhax = call i1 @llvm.expect.i1(i1 %pilhab, i1 false)
  br i1 %pilhax, label %pilha.estouro, label %pilha.ok
pilha.estouro:
  call void @dartforge_stack_overflow()
  ret i64 0
pilha.ok:
  store { ptr, i64, [3 x i64] } { ptr null, i64 4611686018427387907, [3 x i64] zeroinitializer }, ptr %gcq
  %gcs0 = getelementptr inbounds { ptr, i64, [3 x i64] }, ptr %gcq, i64 0, i32 2, i64 0
  %gcs1 = getelementptr inbounds { ptr, i64, [3 x i64] }, ptr %gcq, i64 0, i32 2, i64 1
  %gcs2 = getelementptr inbounds { ptr, i64, [3 x i64] }, ptr %gcq, i64 0, i32 2, i64 2
  %gcant = load ptr, ptr %ctxtopo, align 8
  store ptr %gcant, ptr %gcq, align 8
  store ptr %gcq, ptr %ctxtopo, align 8
  %v0 = call i64 @dartforge_object_new(i64 1073741568, i64 8)
  %fxa0 = add i64 %v0, -2
  %fcb0 = inttoptr i64 %fxa0 to ptr
  %fp0 = getelementptr inbounds i8, ptr %fcb0, i64 16
  %fg0.0 = getelementptr inbounds i64, ptr %fp0, i64 0
  store i64 0, ptr %fg0.0, align 8
  %fmp0.0 = getelementptr inbounds i8, ptr %fcb0, i64 8
  %fm0.0 = load i32, ptr %fmp0.0, align 4
  %fmc0.0 = and i32 %fm0.0, -2
  store i32 %fmc0.0, ptr %fmp0.0, align 4
  %fg0.1 = getelementptr inbounds i64, ptr %fp0, i64 1
  store i64 0, ptr %fg0.1, align 8
  %fmp0.1 = getelementptr inbounds i8, ptr %fcb0, i64 8
  %fm0.1 = load i32, ptr %fmp0.1, align 4
  %fmc0.1 = and i32 %fm0.1, -3
  %fms0.1 = or i32 %fm0.1, 2
  store i32 %fms0.1, ptr %fmp0.1, align 4
  %fg0.2 = getelementptr inbounds i64, ptr %fp0, i64 2
  store i64 0, ptr %fg0.2, align 8
  %fmp0.2 = getelementptr inbounds i8, ptr %fcb0, i64 8
  %fm0.2 = load i32, ptr %fmp0.2, align 4
  %fmc0.2 = and i32 %fm0.2, -5
  %fms0.2 = or i32 %fm0.2, 4
  store i32 %fms0.2, ptr %fmp0.2, align 4
  %fg0.3 = getelementptr inbounds i64, ptr %fp0, i64 3
  store i64 0, ptr %fg0.3, align 8
  %fmp0.3 = getelementptr inbounds i8, ptr %fcb0, i64 8
  %fm0.3 = load i32, ptr %fmp0.3, align 4
  %fmc0.3 = and i32 %fm0.3, -9
  %fms0.3 = or i32 %fm0.3, 8
  store i32 %fms0.3, ptr %fmp0.3, align 4
  %fg0.4 = getelementptr inbounds i64, ptr %fp0, i64 4
  store i64 0, ptr %fg0.4, align 8
  %fmp0.4 = getelementptr inbounds i8, ptr %fcb0, i64 8
  %fm0.4 = load i32, ptr %fmp0.4, align 4
  %fmc0.4 = and i32 %fm0.4, -17
  %fms0.4 = or i32 %fm0.4, 16
  store i32 %fms0.4, ptr %fmp0.4, align 4
  %fg0.5 = getelementptr inbounds i64, ptr %fp0, i64 5
  store i64 0, ptr %fg0.5, align 8
  %fmp0.5 = getelementptr inbounds i8, ptr %fcb0, i64 8
  %fm0.5 = load i32, ptr %fmp0.5, align 4
  %fmc0.5 = and i32 %fm0.5, -33
  store i32 %fmc0.5, ptr %fmp0.5, align 4
  %fg0.6 = getelementptr inbounds i64, ptr %fp0, i64 6
  store i64 0, ptr %fg0.6, align 8
  %fmp0.6 = getelementptr inbounds i8, ptr %fcb0, i64 8
  %fm0.6 = load i32, ptr %fmp0.6, align 4
  %fmc0.6 = and i32 %fm0.6, -65
  store i32 %fmc0.6, ptr %fmp0.6, align 4
  %fg0.7 = getelementptr inbounds i64, ptr %fp0, i64 7
  store i64 0, ptr %fg0.7, align 8
  %fmp0.7 = getelementptr inbounds i8, ptr %fcb0, i64 8
  %fm0.7 = load i32, ptr %fmp0.7, align 4
  %fmc0.7 = and i32 %fm0.7, -129
  store i32 %fmc0.7, ptr %fmp0.7, align 4
  store i64 %v0, ptr %gcs0
  %ga1 = getelementptr i64, ptr %area, i64 3
  %v1 = load i8, ptr %ga1
  %c0 = zext i8 %v1 to i64
  %v2 = icmp ne i64 %c0, 0
  br i1 %v2, label %b1, label %b2
b1:
  %ga3 = getelementptr i64, ptr %area, i64 2
  %v3 = load i64, ptr %ga3
  br label %b3
b2:
  %v4 = call i64 @df.rti.1d66eeb6210799c5.main$2edart()
  br label %b3
b3:
  %v5 = phi i64 [ %v3, %b1 ], [ %v4, %b2 ]
  %v6 = call i64 @df.dart$3aasync.._makeAsyncAwaitCompleter(i64 %v5)
  store i64 %v6, ptr %gcs1
  %v7 = load i8, ptr %ctx, align 8
  %c1 = zext i8 %v7 to i64
  %v8 = icmp ne i64 %c1, 0
  br i1 %v8, label %b5, label %b4
b4:
  %fcb9 = call ptr @df.body(i64 %v0, ptr %ctx)
  %fp9 = getelementptr inbounds i8, ptr %fcb9, i64 16
  %fg9 = getelementptr inbounds i64, ptr %fp9, i64 1
  store i64 %v6, ptr %fg9, align 8
  %fmp9 = getelementptr inbounds i8, ptr %fcb9, i64 8
  %fm9 = load i32, ptr %fmp9, align 4
  %fmc9 = and i32 %fm9, -3
  %fms9 = or i32 %fm9, 2
  store i32 %fms9, ptr %fmp9, align 4
  call void @df.barrier(i64 %v0, i64 %v6)
  br label %wb9.fim
wb9.fim:
  %v10 = call i64 @df.allocate(i64 55834640385, i64 1)
  %bp10 = inttoptr i64 %v10 to ptr
  %bg10_0 = getelementptr inbounds i8, ptr %bp10, i64 14
  store i64 %v0, ptr %bg10_0, align 8
  %bm10 = getelementptr inbounds i8, ptr %bp10, i64 6
  store i32 1, ptr %bm10, align 4
  store i64 %v10, ptr %gcs2
  %v11 = call i64 @df.allocate(i64 47244902401, i64 4)
  %bp11 = inttoptr i64 %v11 to ptr
  %bg11_k = getelementptr inbounds i8, ptr %bp11, i64 14
  store i64 ptrtoint (ptr @df.main$2edart..worker$async$qead0428d$ent to i64), ptr %bg11_k, align 8
  %bg11_e = getelementptr inbounds i8, ptr %bp11, i64 22
  store i64 %v10, ptr %bg11_e, align 8
  %bg11_t = getelementptr inbounds i8, ptr %bp11, i64 30
  store i64 0, ptr %bg11_t, align 8
  %bg11_a = getelementptr inbounds i8, ptr %bp11, i64 38
  store i64 0, ptr %bg11_a, align 8
  %bm11 = getelementptr inbounds i8, ptr %bp11, i64 6
  store i32 2, ptr %bm11, align 4
  store i64 %v11, ptr %gcs2
  %v12 = call i64 @df.dart$3aasync.._wrapBody(i64 %v11)
  store i64 %v12, ptr %gcs2
  %v13 = load i8, ptr %ctx, align 8
  %c2 = zext i8 %v13 to i64
  %v14 = icmp ne i64 %c2, 0
  br i1 %v14, label %b7, label %b6
b5:
  %gcvolta5 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta5, ptr %ctxtopo, align 8
  ret i64 0
b6:
  %fcb15 = call ptr @df.body(i64 %v0, ptr %ctx)
  %fp15 = getelementptr inbounds i8, ptr %fcb15, i64 16
  %fg15 = getelementptr inbounds i64, ptr %fp15, i64 2
  store i64 %v12, ptr %fg15, align 8
  %fmp15 = getelementptr inbounds i8, ptr %fcb15, i64 8
  %fm15 = load i32, ptr %fmp15, align 4
  %fmc15 = and i32 %fm15, -5
  %fms15 = or i32 %fm15, 4
  store i32 %fms15, ptr %fmp15, align 4
  call void @df.barrier(i64 %v0, i64 %v12)
  br label %wb15.fim
wb15.fim:
  store i64 0, ptr %gcs0
  %v16 = call i64 @df.dart$3aasync.._asyncStartSync(i64 %v12, i64 %v6)
  %v17 = load i8, ptr %ctx, align 8
  %c3 = zext i8 %v17 to i64
  %v18 = icmp ne i64 %c3, 0
  br i1 %v18, label %b9, label %b8
b7:
  %gcvolta7 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta7, ptr %ctxtopo, align 8
  ret i64 0
b8:
  %gcvolta8 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta8, ptr %ctxtopo, align 8
  ret i64 %v16
b9:
  %gcvolta9 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta9, ptr %ctxtopo, align 8
  ret i64 0
}

define i64 @df.main$2edart..worker$async$qead0428d(i64 %v0, i64 %v1, i64 %v2) nounwind {
b0:
  %sargs14 = alloca [1 x i64]
  %sargs27 = alloca [2 x i64]
  %sargs37 = alloca [1 x i64]
  %jbuf88 = alloca [10 x i64]
  %sargs92 = alloca [3 x i64]
  %gcq = alloca { ptr, i64, [6 x i64] }, align 8
  %pilhaq = alloca i8, align 1
  %area = call ptr @df.get_area()
  %ctx = call ptr @dartforge_context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load ptr, ptr %pilhalp, align 8
  %pilhab = icmp ult ptr %pilhaq, %pilhal
  %pilhax = call i1 @llvm.expect.i1(i1 %pilhab, i1 false)
  br i1 %pilhax, label %pilha.estouro, label %pilha.ok
pilha.estouro:
  call void @dartforge_stack_overflow()
  ret i64 0
pilha.ok:
  store { ptr, i64, [6 x i64] } { ptr null, i64 4611686018427387910, [6 x i64] zeroinitializer }, ptr %gcq
  %gcs0 = getelementptr inbounds { ptr, i64, [6 x i64] }, ptr %gcq, i64 0, i32 2, i64 0
  %gcs1 = getelementptr inbounds { ptr, i64, [6 x i64] }, ptr %gcq, i64 0, i32 2, i64 1
  %gcs2 = getelementptr inbounds { ptr, i64, [6 x i64] }, ptr %gcq, i64 0, i32 2, i64 2
  %gcs3 = getelementptr inbounds { ptr, i64, [6 x i64] }, ptr %gcq, i64 0, i32 2, i64 3
  %gcs4 = getelementptr inbounds { ptr, i64, [6 x i64] }, ptr %gcq, i64 0, i32 2, i64 4
  %gcs5 = getelementptr inbounds { ptr, i64, [6 x i64] }, ptr %gcq, i64 0, i32 2, i64 5
  %gcant = load ptr, ptr %ctxtopo, align 8
  store ptr %gcant, ptr %gcq, align 8
  store ptr %gcq, ptr %ctxtopo, align 8
  store i64 %v0, ptr %gcs0
  store i64 %v1, ptr %gcs1
  store i64 %v2, ptr %gcs2
  %ep3 = inttoptr i64 %v0 to ptr
  %eg3 = getelementptr inbounds i8, ptr %ep3, i64 14
  %u3 = load i64, ptr %eg3, align 8
  %v3 = add i64 %u3, 0
  store i64 %v3, ptr %gcs0
  %fcb4 = call ptr @df.body(i64 %v3, ptr %ctx)
  %fp4 = getelementptr inbounds i8, ptr %fcb4, i64 16
  %fg4 = getelementptr inbounds i64, ptr %fp4, i64 0
  %v4 = load i64, ptr %fg4, align 8
  %fcb5 = call ptr @df.body(i64 %v3, ptr %ctx)
  %fp5 = getelementptr inbounds i8, ptr %fcb5, i64 16
  %fg5 = getelementptr inbounds i64, ptr %fp5, i64 1
  %v5 = load i64, ptr %fg5, align 8
  store i64 %v5, ptr %gcs3
  %fcb6 = call ptr @df.body(i64 %v3, ptr %ctx)
  %fp6 = getelementptr inbounds i8, ptr %fcb6, i64 16
  %fg6 = getelementptr inbounds i64, ptr %fp6, i64 2
  %v6 = load i64, ptr %fg6, align 8
  store i64 %v6, ptr %gcs4
  switch i64 %v4, label %b1 [ i64 1, label %b24 ]
b1:
  %v7 = add i64 0, 37
  %fcb107 = call ptr @df.body(i64 %v3, ptr %ctx)
  %fp107 = getelementptr inbounds i8, ptr %fcb107, i64 16
  %fg107 = getelementptr inbounds i64, ptr %fp107, i64 6
  store i64 %v7, ptr %fg107, align 8
  %fmp107 = getelementptr inbounds i8, ptr %fcb107, i64 8
  %fm107 = load i32, ptr %fmp107, align 4
  %fmc107 = and i32 %fm107, -65
  store i32 %fmc107, ptr %fmp107, align 4
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %ta10 = getelementptr inbounds i8, ptr %ctx, i64 112
  %tb10 = load ptr, ptr %ta10, align 8
  %tfa10 = getelementptr inbounds i8, ptr %ctx, i64 120
  %tf10 = load ptr, ptr %tfa10, align 8
  %tnx10 = getelementptr i8, ptr %tb10, i64 40
  %tz10 = icmp ugt ptr %tnx10, %tf10
  %tnp10 = getelementptr inbounds i8, ptr %ctx, i64 56
  %tn10 = load i64, ptr %tnp10, align 8
  %tk10 = icmp ule i64 %tn10, 582
  %tq10 = or i1 %tz10, %tk10
  %tx10 = call i1 @llvm.expect.i1(i1 %tq10, i1 false)
  br i1 %tx10, label %ao10.lento, label %ao10.reg
ao10.reg:
  %trp10 = getelementptr inbounds i8, ptr %ctx, i64 48
  %tr10 = load ptr, ptr %trp10, align 8
  %trb10 = getelementptr inbounds i8, ptr %tr10, i64 582
  %trv10 = load i8, ptr %trb10, align 1
  %trz10 = icmp eq i8 %trv10, 0
  %try10 = call i1 @llvm.expect.i1(i1 %trz10, i1 false)
  br i1 %try10, label %ao10.lento, label %ao10.rapido
ao10.rapido:
  store ptr %tnx10, ptr %ta10, align 8
  %tpp10 = getelementptr inbounds i8, ptr %tb10, i64 16
  store i64 0, ptr %tpp10, align 8
  store i64 2499671162881, ptr %tb10, align 8
  %thb10 = ptrtoint ptr %tb10 to i64
  %th10 = add i64 %thb10, 2
  br label %ao10.fim
ao10.lento:
  %tl10 = call i64 @dartforge_object_new_t(i64 582, i64 3, ptr @df.mt.dart$3acore.DateTime)
  br label %ao10.fim
ao10.fim:
  %v10 = phi i64 [ %th10, %ao10.rapido ], [ %tl10, %ao10.lento ]
  store i64 %v10, ptr %gcs1
  call void @df.dart$3acore.DateTime.new$3anow(i64 %v10)
  %v12 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v12 to i64
  %v13 = icmp ne i64 %c0, 0
  br i1 %v13, label %b2, label %b3
b2:
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  store i64 0, ptr %gcs4
  %v101 = call i64 @dartforge_exception_peek_ref()
  store i64 %v101, ptr %gcs0
  %v102 = call i64 @dartforge_stack_trace_get()
  store i64 %v102, ptr %gcs1
  %xcp103 = load i8, ptr %ctx, align 8
  %xcn103 = icmp ne i8 %xcp103, 0
  %xce103 = call i1 @llvm.expect.i1(i1 %xcn103, i1 false)
  br i1 %xce103, label %xc103.limpar, label %xc103.fim
xc103.limpar:
  call void @dartforge_exception_clear()
  br label %xc103.fim
xc103.fim:
  %v104 = call i64 @df.dart$3aasync.._asyncRethrow(i64 %v101, i64 %v102, i64 %v5)
  %v105 = load i8, ptr %ctx, align 8
  %c1 = zext i8 %v105 to i64
  %v106 = icmp ne i64 %c1, 0
  br i1 %v106, label %b36, label %b35
b3:
  %sat14 = getelementptr [1 x i64], ptr %sargs14, i64 0, i64 0
  store i64 0, ptr %sat14
  %sic14 = getelementptr i64, ptr %area, i64 26
  ; df.selector-exact 582
  %sf14 = call ptr @df.selector(ptr %sic14, i64 %v10, i64 -8375378311371268431, ptr @df.seln.0, i64 24)
  %v14 = call i64 %sf14(i64 %v10, ptr %sargs14, ptr @df.arr.0)
  store i64 %v14, ptr %gcs1
  %v15 = load i8, ptr %ctx, align 8
  %c2 = zext i8 %v15 to i64
  %v16 = icmp ne i64 %c2, 0
  br i1 %v16, label %b2, label %b4
b4:
  %v17 = call i64 @df.unbox_int(i64 %v14)
  %v18 = load i8, ptr %ctx, align 8
  %c3 = zext i8 %v18 to i64
  %v19 = icmp ne i64 %c3, 0
  br i1 %v19, label %b2, label %b5
b5:
  %fcb108 = call ptr @df.body(i64 %v3, ptr %ctx)
  %fp108 = getelementptr inbounds i8, ptr %fcb108, i64 16
  %fg108 = getelementptr inbounds i64, ptr %fp108, i64 7
  store i64 %v17, ptr %fg108, align 8
  %fmp108 = getelementptr inbounds i8, ptr %fcb108, i64 8
  %fm108 = load i32, ptr %fmp108, align 4
  %fmc108 = and i32 %fm108, -129
  store i32 %fmc108, ptr %fmp108, align 4
  store i64 0, ptr %gcs1
  %lsp22 = getelementptr i64, ptr %area, i64 28
  %lsv22 = load i64, ptr %lsp22, align 8
  %lsz22 = icmp eq i64 %lsv22, 0
  %lsx22 = call i1 @llvm.expect.i1(i1 %lsz22, i1 false)
  br i1 %lsx22, label %ls22.nova, label %ls22.fim
ls22.nova:
  %lsn22 = call i64 @dartforge_string_new(ptr @.str.0, i64 88)
  store i64 %lsn22, ptr %lsp22, align 8
  br label %ls22.fim
ls22.fim:
  %v22 = phi i64 [ %lsv22, %b5 ], [ %lsn22, %ls22.nova ]
  store i64 %v22, ptr %gcs1
  %v23 = call i64 @df.dart$3aio.File.new(i64 %v22)
  store i64 %v23, ptr %gcs1
  %v24 = load i8, ptr %ctx, align 8
  %c4 = zext i8 %v24 to i64
  %v25 = icmp ne i64 %c4, 0
  br i1 %v25, label %b2, label %b6
b6:
  %lsp26 = getelementptr i64, ptr %area, i64 30
  %lsv26 = load i64, ptr %lsp26, align 8
  %lsz26 = icmp eq i64 %lsv26, 0
  %lsx26 = call i1 @llvm.expect.i1(i1 %lsz26, i1 false)
  br i1 %lsx26, label %ls26.nova, label %ls26.fim
ls26.nova:
  %lsn26 = call i64 @dartforge_string_new(ptr @.str.1, i64 9)
  store i64 %lsn26, ptr %lsp26, align 8
  br label %ls26.fim
ls26.fim:
  %v26 = phi i64 [ %lsv26, %b6 ], [ %lsn26, %ls26.nova ]
  store i64 %v26, ptr %gcs2
  %sa27_0 = getelementptr [2 x i64], ptr %sargs27, i64 0, i64 0
  store i64 %v26, ptr %sa27_0
  %sat27 = getelementptr [2 x i64], ptr %sargs27, i64 0, i64 1
  store i64 0, ptr %sat27
  %sic27 = getelementptr i64, ptr %area, i64 32
  %sf27 = call ptr @df.selector(ptr %sic27, i64 %v23, i64 6686196670178363055, ptr @df.seln.1, i64 20)
  %v27 = call i64 %sf27(i64 %v23, ptr %sargs27, ptr @df.arr.1)
  %v28 = load i8, ptr %ctx, align 8
  %c5 = zext i8 %v28 to i64
  %v29 = icmp ne i64 %c5, 0
  br i1 %v29, label %b2, label %b7
b7:
  %v31 = load i8, ptr %ctx, align 8
  %c6 = zext i8 %v31 to i64
  %v32 = icmp ne i64 %c6, 0
  br i1 %v32, label %b2, label %b8
b8:
  br label %b9
b9:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %lsp33 = getelementptr i64, ptr %area, i64 34
  %lsv33 = load i64, ptr %lsp33, align 8
  %lsz33 = icmp eq i64 %lsv33, 0
  %lsx33 = call i1 @llvm.expect.i1(i1 %lsz33, i1 false)
  br i1 %lsx33, label %ls33.nova, label %ls33.fim
ls33.nova:
  %lsn33 = call i64 @dartforge_string_new(ptr @.str.2, i64 86)
  store i64 %lsn33, ptr %lsp33, align 8
  br label %ls33.fim
ls33.fim:
  %v33 = phi i64 [ %lsv33, %b9 ], [ %lsn33, %ls33.nova ]
  store i64 %v33, ptr %gcs1
  %v34 = call i64 @df.dart$3aio.File.new(i64 %v33)
  store i64 %v34, ptr %gcs1
  %v35 = load i8, ptr %ctx, align 8
  %c7 = zext i8 %v35 to i64
  %v36 = icmp ne i64 %c7, 0
  br i1 %v36, label %b2, label %b12
b10:
  %ip44 = getelementptr inbounds i8, ptr %ctx, i64 32
  %iv44 = load atomic i8, ptr %ip44 monotonic, align 8
  %v44 = call i8 @llvm.expect.i8(i8 %iv44, i8 0)
  %c8 = zext i8 %v44 to i64
  %v45 = icmp ne i64 %c8, 0
  br i1 %v45, label %b15, label %b16
b11:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs4
  %lsp75 = getelementptr i64, ptr %area, i64 36
  %lsv75 = load i64, ptr %lsp75, align 8
  %lsz75 = icmp eq i64 %lsv75, 0
  %lsx75 = call i1 @llvm.expect.i1(i1 %lsz75, i1 false)
  br i1 %lsx75, label %ls75.nova, label %ls75.fim
ls75.nova:
  %lsn75 = call i64 @dartforge_string_new(ptr @.str.3, i64 85)
  store i64 %lsn75, ptr %lsp75, align 8
  br label %ls75.fim
ls75.fim:
  %v75 = phi i64 [ %lsv75, %b11 ], [ %lsn75, %ls75.nova ]
  store i64 %v75, ptr %gcs1
  %v76 = call i64 @df.dart$3aio.File.new(i64 %v75)
  store i64 %v76, ptr %gcs1
  %v77 = load i8, ptr %ctx, align 8
  %c9 = zext i8 %v77 to i64
  %v78 = icmp ne i64 %c9, 0
  br i1 %v78, label %b2, label %b30
b12:
  %sat37 = getelementptr [1 x i64], ptr %sargs37, i64 0, i64 0
  store i64 0, ptr %sat37
  %sic37 = getelementptr i64, ptr %area, i64 38
  %sf37 = call ptr @df.selector(ptr %sic37, i64 %v34, i64 -2868057963325022603, ptr @df.seln.2, i64 13)
  %v37 = call i64 %sf37(i64 %v34, ptr %sargs37, ptr @df.arr.0)
  store i64 %v37, ptr %gcs1
  %v38 = load i8, ptr %ctx, align 8
  %c10 = zext i8 %v38 to i64
  %v39 = icmp ne i64 %c10, 0
  br i1 %v39, label %b2, label %b13
b13:
  %v40 = call i1 @df.unbox_bool(i64 %v37)
  %v41 = load i8, ptr %ctx, align 8
  %c11 = zext i8 %v41 to i64
  %v42 = icmp ne i64 %c11, 0
  br i1 %v42, label %b2, label %b14
b14:
  %v43 = xor i1 %v40, true
  br i1 %v43, label %b10, label %b11
b15:
  store i64 0, ptr %gcs1
  call void @dartforge_safepoint()
  %v47 = load i8, ptr %ctx, align 8
  %c12 = zext i8 %v47 to i64
  %v48 = icmp ne i64 %c12, 0
  br i1 %v48, label %b2, label %b17
b16:
  store i64 0, ptr %gcs1
  %v49 = call i64 @dfc.a78712e154cd8104.get()
  store i64 %v49, ptr %gcs1
  %v50 = load i8, ptr %ctx, align 8
  %c13 = zext i8 %v50 to i64
  %v51 = icmp ne i64 %c13, 0
  br i1 %v51, label %b2, label %b18
b17:
  br label %b16
b18:
  %ga52 = getelementptr i64, ptr %area, i64 3
  %v52 = load i8, ptr %ga52
  %c14 = zext i8 %v52 to i64
  %v53 = icmp ne i64 %c14, 0
  br i1 %v53, label %b19, label %b20
b19:
  %ga54 = getelementptr i64, ptr %area, i64 2
  %v54 = load i64, ptr %ga54
  br label %b21
b20:
  %v55 = call i64 @df.rti.1d66eeb6210799c5.main$2edart()
  br label %b21
b21:
  %v56 = phi i64 [ %v54, %b19 ], [ %v55, %b20 ]
  %v57 = call i64 @df.dart$3aasync.Future.new$3adelayed(i64 %v49, i64 0, i64 %v56)
  store i64 %v57, ptr %gcs1
  %v58 = load i8, ptr %ctx, align 8
  %c15 = zext i8 %v58 to i64
  %v59 = icmp ne i64 %c15, 0
  br i1 %v59, label %b2, label %b22
b22:
  %fcb60 = call ptr @df.body(i64 %v3, ptr %ctx)
  %fp60 = getelementptr inbounds i8, ptr %fcb60, i64 16
  %fg60 = getelementptr inbounds i64, ptr %fp60, i64 0
  store i64 1, ptr %fg60, align 8
  %fmp60 = getelementptr inbounds i8, ptr %fcb60, i64 8
  %fm60 = load i32, ptr %fmp60, align 4
  %fmc60 = and i32 %fm60, -2
  store i32 %fmc60, ptr %fmp60, align 4
  store i64 0, ptr %gcs0
  %v61 = call i64 @df.dart$3aasync.._asyncAwait(i64 %v57, i64 %v6)
  %v62 = load i8, ptr %ctx, align 8
  %c16 = zext i8 %v62 to i64
  %v63 = icmp ne i64 %c16, 0
  br i1 %v63, label %b2, label %b23
b23:
  %gcvolta23 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta23, ptr %ctxtopo, align 8
  ret i64 0
b24:
  %v64 = call i64 @df.unbox_int(i64 %v1)
  %v65 = load i8, ptr %ctx, align 8
  %c17 = zext i8 %v65 to i64
  %v66 = icmp ne i64 %c17, 0
  br i1 %v66, label %b2, label %b25
b25:
  %v67 = icmp eq i64 %v64, 1
  br i1 %v67, label %b26, label %b27
b26:
  %fcb68 = call ptr @df.body(i64 %v2, ptr %ctx)
  %fp68 = getelementptr inbounds i8, ptr %fcb68, i64 16
  %fg68 = getelementptr inbounds i64, ptr %fp68, i64 0
  %v68 = load i64, ptr %fg68, align 8
  store i64 %v68, ptr %gcs0
  %fcb69 = call ptr @df.body(i64 %v2, ptr %ctx)
  %fp69 = getelementptr inbounds i8, ptr %fcb69, i64 16
  %fg69 = getelementptr inbounds i64, ptr %fp69, i64 1
  %v69 = load i64, ptr %fg69, align 8
  store i64 %v69, ptr %gcs1
  store i64 0, ptr %gcs2
  store i64 0, ptr %gcs4
  call void @dartforge_throw_with_stack_trace(i64 %v68, i8 3, i64 %v69)
  br label %b2
b27:
  %v73 = load i8, ptr %ctx, align 8
  %c18 = zext i8 %v73 to i64
  %v74 = icmp ne i64 %c18, 0
  br i1 %v74, label %b2, label %b29
b29:
  br label %b9
b30:
  %lsp79 = getelementptr i64, ptr %area, i64 40
  %lsv79 = load i64, ptr %lsp79, align 8
  %lsz79 = icmp eq i64 %lsv79, 0
  %lsx79 = call i1 @llvm.expect.i1(i1 %lsz79, i1 false)
  br i1 %lsx79, label %ls79.nova, label %ls79.fim
ls79.nova:
  %lsn79 = call i64 @dartforge_string_new(ptr @.str.4, i64 4)
  store i64 %lsn79, ptr %lsp79, align 8
  br label %ls79.fim
ls79.fim:
  %v79 = phi i64 [ %lsv79, %b30 ], [ %lsn79, %ls79.nova ]
  store i64 %v79, ptr %gcs2
  %fcb80 = call ptr @df.body(i64 %v3, ptr %ctx)
  %fp80 = getelementptr inbounds i8, ptr %fcb80, i64 16
  %fg80 = getelementptr inbounds i64, ptr %fp80, i64 6
  %v80 = load i64, ptr %fg80, align 8
  %lsp81 = getelementptr i64, ptr %area, i64 42
  %lsv81 = load i64, ptr %lsp81, align 8
  %lsz81 = icmp eq i64 %lsv81, 0
  %lsx81 = call i1 @llvm.expect.i1(i1 %lsz81, i1 false)
  br i1 %lsx81, label %ls81.nova, label %ls81.fim
ls81.nova:
  %lsn81 = call i64 @dartforge_string_new(ptr @.str.5, i64 1)
  store i64 %lsn81, ptr %lsp81, align 8
  br label %ls81.fim
ls81.fim:
  %v81 = phi i64 [ %lsv81, %ls79.fim ], [ %lsn81, %ls81.nova ]
  store i64 %v81, ptr %gcs4
  %fcb82 = call ptr @df.body(i64 %v3, ptr %ctx)
  %fp82 = getelementptr inbounds i8, ptr %fcb82, i64 16
  %fg82 = getelementptr inbounds i64, ptr %fp82, i64 7
  %v82 = load i64, ptr %fg82, align 8
  %v83 = add i64 0, 0
  %v84 = icmp sgt i64 %v82, %v83
  store i64 0, ptr %gcs0
  %v85 = zext i1 %v84 to i8
  %v86 = call i64 @dartforge_to_string_bool(i8 %v85)
  store i64 %v86, ptr %gcs0
  %lsp87 = getelementptr i64, ptr %area, i64 44
  %lsv87 = load i64, ptr %lsp87, align 8
  %lsz87 = icmp eq i64 %lsv87, 0
  %lsx87 = call i1 @llvm.expect.i1(i1 %lsz87, i1 false)
  br i1 %lsx87, label %ls87.nova, label %ls87.fim
ls87.nova:
  %lsn87 = call i64 @dartforge_string_new(ptr @.str.6, i64 1)
  store i64 %lsn87, ptr %lsp87, align 8
  br label %ls87.fim
ls87.fim:
  %v87 = phi i64 [ %lsv87, %ls81.fim ], [ %lsn87, %ls87.nova ]
  store i64 %v87, ptr %gcs5
  %jk88_0 = getelementptr [10 x i64], ptr %jbuf88, i64 0, i64 0
  store i64 0, ptr %jk88_0
  %jp88_0 = getelementptr [10 x i64], ptr %jbuf88, i64 0, i64 1
  store i64 %v79, ptr %jp88_0
  %jk88_1 = getelementptr [10 x i64], ptr %jbuf88, i64 0, i64 2
  store i64 1, ptr %jk88_1
  %jp88_1 = getelementptr [10 x i64], ptr %jbuf88, i64 0, i64 3
  store i64 %v80, ptr %jp88_1
  %jk88_2 = getelementptr [10 x i64], ptr %jbuf88, i64 0, i64 4
  store i64 0, ptr %jk88_2
  %jp88_2 = getelementptr [10 x i64], ptr %jbuf88, i64 0, i64 5
  store i64 %v81, ptr %jp88_2
  %jk88_3 = getelementptr [10 x i64], ptr %jbuf88, i64 0, i64 6
  store i64 0, ptr %jk88_3
  %jp88_3 = getelementptr [10 x i64], ptr %jbuf88, i64 0, i64 7
  store i64 %v86, ptr %jp88_3
  %jk88_4 = getelementptr [10 x i64], ptr %jbuf88, i64 0, i64 8
  store i64 0, ptr %jk88_4
  %jp88_4 = getelementptr [10 x i64], ptr %jbuf88, i64 0, i64 9
  store i64 %v87, ptr %jp88_4
  %v88 = call i64 @dartforge_string_join_typed(ptr %jbuf88, i64 5)
  store i64 %v88, ptr %gcs0
  store i64 0, ptr %gcs2
  store i64 0, ptr %gcs4
  store i64 0, ptr %gcs5
  %v89 = call i64 @df.dart$3aio.FileMode.append()
  store i64 %v89, ptr %gcs2
  %v90 = load i8, ptr %ctx, align 8
  %c19 = zext i8 %v90 to i64
  %v91 = icmp ne i64 %c19, 0
  br i1 %v91, label %b2, label %b31
b31:
  %sa92_0 = getelementptr [3 x i64], ptr %sargs92, i64 0, i64 0
  store i64 %v88, ptr %sa92_0
  %sa92_1 = getelementptr [3 x i64], ptr %sargs92, i64 0, i64 1
  store i64 %v89, ptr %sa92_1
  %sat92 = getelementptr [3 x i64], ptr %sargs92, i64 0, i64 2
  store i64 0, ptr %sat92
  %sic92 = getelementptr i64, ptr %area, i64 46
  %sf92 = call ptr @df.selector(ptr %sic92, i64 %v76, i64 6686196670178363055, ptr @df.seln.1, i64 20)
  %v92 = call i64 %sf92(i64 %v76, ptr %sargs92, ptr @df.arr.2)
  %v93 = load i8, ptr %ctx, align 8
  %c20 = zext i8 %v93 to i64
  %v94 = icmp ne i64 %c20, 0
  br i1 %v94, label %b2, label %b32
b32:
  %v96 = load i8, ptr %ctx, align 8
  %c21 = zext i8 %v96 to i64
  %v97 = icmp ne i64 %c21, 0
  br i1 %v97, label %b2, label %b33
b33:
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v98 = call i64 @df.dart$3aasync.._asyncReturn(i64 0, i64 %v5)
  %v99 = load i8, ptr %ctx, align 8
  %c22 = zext i8 %v99 to i64
  %v100 = icmp ne i64 %c22, 0
  br i1 %v100, label %b2, label %b34
b34:
  %gcvolta34 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta34, ptr %ctxtopo, align 8
  ret i64 0
b35:
  %gcvolta35 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta35, ptr %ctxtopo, align 8
  ret i64 0
b36:
  %gcvolta36 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta36, ptr %ctxtopo, align 8
  ret i64 0
}

define linkonce_odr i64 @dfc.a78712e154cd8104.get() nounwind {
b0:
  %gcq = alloca { ptr, i64, [1 x i64] }, align 8
  %pilhaq = alloca i8, align 1
  %area = call ptr @df.get_area()
  %ctx = call ptr @dartforge_context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load ptr, ptr %pilhalp, align 8
  %pilhab = icmp ult ptr %pilhaq, %pilhal
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
  %ga0 = getelementptr i64, ptr %area, i64 1
  %v0 = load i8, ptr %ga0
  %c0 = zext i8 %v0 to i64
  %v1 = icmp ne i64 %c0, 0
  br i1 %v1, label %b1, label %b2
b1:
  %ga2 = getelementptr i64, ptr %area, i64 0
  %v2 = load i64, ptr %ga2
  %gcvolta1 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1, ptr %ctxtopo, align 8
  ret i64 %v2
b2:
  %v3 = add i64 0, 10
  %v4 = add i64 0, 0
  %v5 = add i64 0, 0
  %v6 = add i64 0, 0
  %v7 = add i64 0, 0
  %v8 = add i64 0, 0
  %ta9 = getelementptr inbounds i8, ptr %ctx, i64 80
  %tb9 = load ptr, ptr %ta9, align 8
  %tfa9 = getelementptr inbounds i8, ptr %ctx, i64 88
  %tf9 = load ptr, ptr %tfa9, align 8
  %tnx9 = getelementptr i8, ptr %tb9, i64 24
  %tz9 = icmp ugt ptr %tnx9, %tf9
  %tnp9 = getelementptr inbounds i8, ptr %ctx, i64 56
  %tn9 = load i64, ptr %tnp9, align 8
  %tk9 = icmp ule i64 %tn9, 584
  %tq9 = or i1 %tz9, %tk9
  %tx9 = call i1 @llvm.expect.i1(i1 %tq9, i1 false)
  br i1 %tx9, label %ao9.lento, label %ao9.reg
ao9.reg:
  %trp9 = getelementptr inbounds i8, ptr %ctx, i64 48
  %tr9 = load ptr, ptr %trp9, align 8
  %trb9 = getelementptr inbounds i8, ptr %tr9, i64 584
  %trv9 = load i8, ptr %trb9, align 1
  %trz9 = icmp eq i8 %trv9, 0
  %try9 = call i1 @llvm.expect.i1(i1 %trz9, i1 false)
  br i1 %try9, label %ao9.lento, label %ao9.rapido
ao9.rapido:
  store ptr %tnx9, ptr %ta9, align 8
  %tpp9 = getelementptr inbounds i8, ptr %tb9, i64 16
  store i64 0, ptr %tpp9, align 8
  store i64 2508260966401, ptr %tb9, align 8
  %thb9 = ptrtoint ptr %tb9 to i64
  %th9 = add i64 %thb9, 2
  br label %ao9.fim
ao9.lento:
  %tl9 = call i64 @dartforge_object_new_t(i64 584, i64 1, ptr @df.mt.dart$3acore.Duration)
  br label %ao9.fim
ao9.fim:
  %v9 = phi i64 [ %th9, %ao9.rapido ], [ %tl9, %ao9.lento ]
  store i64 %v9, ptr %gcs0
  call void @df.dart$3acore.Duration.new(i64 %v9, i64 %v4, i64 %v5, i64 %v6, i64 %v7, i64 %v3, i64 %v8)
  %v11 = load i8, ptr %ctx, align 8
  %c1 = zext i8 %v11 to i64
  %v12 = icmp ne i64 %c1, 0
  br i1 %v12, label %b4, label %b3
b3:
  %v13 = call i64 @dartforge_canonical_constant(i64 %v9)
  store i64 %v13, ptr %gcs0
  %ga14 = getelementptr i64, ptr %area, i64 0
  store i64 %v13, ptr %ga14
  %gr14 = ptrtoint ptr %ga14 to i64
  call void @dartforge_gc_global_root(i64 %gr14, i64 %v13)
  call void @dartforge_mark_constant(i64 %v13, i64 ptrtoint (ptr @dfc.a78712e154cd8104.get to i64))
  %ga16 = getelementptr i64, ptr %area, i64 1
  store i8 1, ptr %ga16
  %gcvolta3 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta3, ptr %ctxtopo, align 8
  ret i64 %v13
b4:
  %gcvolta4 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta4, ptr %ctxtopo, align 8
  ret i64 0
}

define i64 @df.rti.1d66eeb6210799c5.main$2edart() nounwind {
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
  %lsp3 = getelementptr i64, ptr %area, i64 22
  %lsv3 = load i64, ptr %lsp3, align 8
  %lsz3 = icmp eq i64 %lsv3, 0
  %lsx3 = call i1 @llvm.expect.i1(i1 %lsz3, i1 false)
  br i1 %lsx3, label %ls3.nova, label %ls3.fim
ls3.nova:
  %lsn3 = call i64 @dartforge_string_new(ptr @.str.7, i64 4)
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

define i64 @df.main$2edart..worker$async$qead0428d$ent(i64 %v0, ptr %v1, ptr %v2) nounwind {
b0:
  %gcq = alloca { ptr, i64, [3 x i64] }, align 8
  %pilhaq = alloca i8, align 1
  %ctx = call ptr @dartforge_context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load ptr, ptr %pilhalp, align 8
  %pilhab = icmp ult ptr %pilhaq, %pilhal
  %pilhax = call i1 @llvm.expect.i1(i1 %pilhab, i1 false)
  br i1 %pilhax, label %pilha.estouro, label %pilha.ok
pilha.estouro:
  call void @dartforge_stack_overflow()
  ret i64 0
pilha.ok:
  store { ptr, i64, [3 x i64] } { ptr null, i64 4611686018427387907, [3 x i64] zeroinitializer }, ptr %gcq
  %gcs0 = getelementptr inbounds { ptr, i64, [3 x i64] }, ptr %gcq, i64 0, i32 2, i64 0
  %gcs1 = getelementptr inbounds { ptr, i64, [3 x i64] }, ptr %gcq, i64 0, i32 2, i64 1
  %gcs2 = getelementptr inbounds { ptr, i64, [3 x i64] }, ptr %gcq, i64 0, i32 2, i64 2
  %gcant = load ptr, ptr %ctxtopo, align 8
  store ptr %gcant, ptr %gcq, align 8
  store ptr %gcq, ptr %ctxtopo, align 8
  %fcb3 = call ptr @df.body(i64 %v0, ptr %ctx)
  %fp3 = getelementptr inbounds i8, ptr %fcb3, i64 16
  %fg3 = getelementptr inbounds i64, ptr %fp3, i64 1
  %v3 = load i64, ptr %fg3, align 8
  store i64 %v3, ptr %gcs0
  %li4 = getelementptr i64, ptr %v2, i64 0
  %v4 = load i64, ptr %li4
  %li5 = getelementptr i64, ptr %v2, i64 1
  %v5 = load i64, ptr %li5
  %v6 = xor i64 %v4, 2
  %v7 = or i64 %v6, %v5
  %v8 = icmp eq i64 %v7, 0
  br i1 %v8, label %b1, label %b2
b1:
  %li10 = getelementptr i64, ptr %v2, i64 0
  %v10 = load i64, ptr %li10
  %li11 = getelementptr i64, ptr %v1, i64 0
  %v11 = load i64, ptr %li11
  store i64 %v11, ptr %gcs1
  %li12 = getelementptr i64, ptr %v1, i64 1
  %v12 = load i64, ptr %li12
  store i64 %v12, ptr %gcs2
  %v13 = call i64 @df.main$2edart..worker$async$qead0428d(i64 %v3, i64 %v11, i64 %v12)
  %v14 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v14 to i64
  %v15 = icmp ne i64 %c0, 0
  br i1 %v15, label %b4, label %b3
b2:
  store i64 0, ptr %gcs0
  call void @dartforge_nsm_call()
  %gcvolta2 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2, ptr %ctxtopo, align 8
  ret i64 0
b3:
  %gcvolta3 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta3, ptr %ctxtopo, align 8
  ret i64 %v13
b4:
  %gcvolta4 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta4, ptr %ctxtopo, align 8
  ret i64 0
}

define void @dart_main() nounwind {
b0:
  %gcq = alloca { ptr, i64, [3 x i64] }, align 8
  %pilhaq = alloca i8, align 1
  %area = call ptr @df.get_area()
  %ctx = call ptr @dartforge_context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load ptr, ptr %pilhalp, align 8
  %pilhab = icmp ult ptr %pilhaq, %pilhal
  %pilhax = call i1 @llvm.expect.i1(i1 %pilhab, i1 false)
  br i1 %pilhax, label %pilha.estouro, label %pilha.ok
pilha.estouro:
  call void @dartforge_stack_overflow()
  ret void
pilha.ok:
  store { ptr, i64, [3 x i64] } { ptr null, i64 4611686018427387907, [3 x i64] zeroinitializer }, ptr %gcq
  %gcs0 = getelementptr inbounds { ptr, i64, [3 x i64] }, ptr %gcq, i64 0, i32 2, i64 0
  %gcs1 = getelementptr inbounds { ptr, i64, [3 x i64] }, ptr %gcq, i64 0, i32 2, i64 1
  %gcs2 = getelementptr inbounds { ptr, i64, [3 x i64] }, ptr %gcq, i64 0, i32 2, i64 2
  %gcant = load ptr, ptr %ctxtopo, align 8
  store ptr %gcant, ptr %gcq, align 8
  store ptr %gcq, ptr %ctxtopo, align 8
  %v0 = call i64 @dartforge_object_new(i64 1073741568, i64 6)
  %fxa0 = add i64 %v0, -2
  %fcb0 = inttoptr i64 %fxa0 to ptr
  %fp0 = getelementptr inbounds i8, ptr %fcb0, i64 16
  %fg0.0 = getelementptr inbounds i64, ptr %fp0, i64 0
  store i64 0, ptr %fg0.0, align 8
  %fmp0.0 = getelementptr inbounds i8, ptr %fcb0, i64 8
  %fm0.0 = load i32, ptr %fmp0.0, align 4
  %fmc0.0 = and i32 %fm0.0, -2
  store i32 %fmc0.0, ptr %fmp0.0, align 4
  %fg0.1 = getelementptr inbounds i64, ptr %fp0, i64 1
  store i64 0, ptr %fg0.1, align 8
  %fmp0.1 = getelementptr inbounds i8, ptr %fcb0, i64 8
  %fm0.1 = load i32, ptr %fmp0.1, align 4
  %fmc0.1 = and i32 %fm0.1, -3
  %fms0.1 = or i32 %fm0.1, 2
  store i32 %fms0.1, ptr %fmp0.1, align 4
  %fg0.2 = getelementptr inbounds i64, ptr %fp0, i64 2
  store i64 0, ptr %fg0.2, align 8
  %fmp0.2 = getelementptr inbounds i8, ptr %fcb0, i64 8
  %fm0.2 = load i32, ptr %fmp0.2, align 4
  %fmc0.2 = and i32 %fm0.2, -5
  %fms0.2 = or i32 %fm0.2, 4
  store i32 %fms0.2, ptr %fmp0.2, align 4
  %fg0.3 = getelementptr inbounds i64, ptr %fp0, i64 3
  store i64 0, ptr %fg0.3, align 8
  %fmp0.3 = getelementptr inbounds i8, ptr %fcb0, i64 8
  %fm0.3 = load i32, ptr %fmp0.3, align 4
  %fmc0.3 = and i32 %fm0.3, -9
  %fms0.3 = or i32 %fm0.3, 8
  store i32 %fms0.3, ptr %fmp0.3, align 4
  %fg0.4 = getelementptr inbounds i64, ptr %fp0, i64 4
  store i64 0, ptr %fg0.4, align 8
  %fmp0.4 = getelementptr inbounds i8, ptr %fcb0, i64 8
  %fm0.4 = load i32, ptr %fmp0.4, align 4
  %fmc0.4 = and i32 %fm0.4, -17
  %fms0.4 = or i32 %fm0.4, 16
  store i32 %fms0.4, ptr %fmp0.4, align 4
  %fg0.5 = getelementptr inbounds i64, ptr %fp0, i64 5
  store i64 0, ptr %fg0.5, align 8
  %fmp0.5 = getelementptr inbounds i8, ptr %fcb0, i64 8
  %fm0.5 = load i32, ptr %fmp0.5, align 4
  %fmc0.5 = and i32 %fm0.5, -33
  store i32 %fmc0.5, ptr %fmp0.5, align 4
  store i64 %v0, ptr %gcs0
  %ga1 = getelementptr i64, ptr %area, i64 3
  %v1 = load i8, ptr %ga1
  %c0 = zext i8 %v1 to i64
  %v2 = icmp ne i64 %c0, 0
  br i1 %v2, label %b1, label %b2
b1:
  %ga3 = getelementptr i64, ptr %area, i64 2
  %v3 = load i64, ptr %ga3
  br label %b3
b2:
  %v4 = call i64 @df.rti.1d66eeb6210799c5.main$2edart()
  br label %b3
b3:
  %v5 = phi i64 [ %v3, %b1 ], [ %v4, %b2 ]
  %v6 = call i64 @df.dart$3aasync.._makeAsyncAwaitCompleter(i64 %v5)
  store i64 %v6, ptr %gcs1
  %v7 = load i8, ptr %ctx, align 8
  %c1 = zext i8 %v7 to i64
  %v8 = icmp ne i64 %c1, 0
  br i1 %v8, label %b5, label %b4
b4:
  %fcb9 = call ptr @df.body(i64 %v0, ptr %ctx)
  %fp9 = getelementptr inbounds i8, ptr %fcb9, i64 16
  %fg9 = getelementptr inbounds i64, ptr %fp9, i64 1
  store i64 %v6, ptr %fg9, align 8
  %fmp9 = getelementptr inbounds i8, ptr %fcb9, i64 8
  %fm9 = load i32, ptr %fmp9, align 4
  %fmc9 = and i32 %fm9, -3
  %fms9 = or i32 %fm9, 2
  store i32 %fms9, ptr %fmp9, align 4
  call void @df.barrier(i64 %v0, i64 %v6)
  br label %wb9.fim
wb9.fim:
  %v10 = call i64 @df.allocate(i64 55834640385, i64 1)
  %bp10 = inttoptr i64 %v10 to ptr
  %bg10_0 = getelementptr inbounds i8, ptr %bp10, i64 14
  store i64 %v0, ptr %bg10_0, align 8
  %bm10 = getelementptr inbounds i8, ptr %bp10, i64 6
  store i32 1, ptr %bm10, align 4
  store i64 %v10, ptr %gcs2
  %v11 = call i64 @df.allocate(i64 47244902401, i64 4)
  %bp11 = inttoptr i64 %v11 to ptr
  %bg11_k = getelementptr inbounds i8, ptr %bp11, i64 14
  store i64 ptrtoint (ptr @dart_main$async$q0fee530a$ent to i64), ptr %bg11_k, align 8
  %bg11_e = getelementptr inbounds i8, ptr %bp11, i64 22
  store i64 %v10, ptr %bg11_e, align 8
  %bg11_t = getelementptr inbounds i8, ptr %bp11, i64 30
  store i64 0, ptr %bg11_t, align 8
  %bg11_a = getelementptr inbounds i8, ptr %bp11, i64 38
  store i64 0, ptr %bg11_a, align 8
  %bm11 = getelementptr inbounds i8, ptr %bp11, i64 6
  store i32 2, ptr %bm11, align 4
  store i64 %v11, ptr %gcs2
  %v12 = call i64 @df.dart$3aasync.._wrapBody(i64 %v11)
  store i64 %v12, ptr %gcs2
  %v13 = load i8, ptr %ctx, align 8
  %c2 = zext i8 %v13 to i64
  %v14 = icmp ne i64 %c2, 0
  br i1 %v14, label %b7, label %b6
b5:
  %gcvolta5 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta5, ptr %ctxtopo, align 8
  ret void
b6:
  %fcb15 = call ptr @df.body(i64 %v0, ptr %ctx)
  %fp15 = getelementptr inbounds i8, ptr %fcb15, i64 16
  %fg15 = getelementptr inbounds i64, ptr %fp15, i64 2
  store i64 %v12, ptr %fg15, align 8
  %fmp15 = getelementptr inbounds i8, ptr %fcb15, i64 8
  %fm15 = load i32, ptr %fmp15, align 4
  %fmc15 = and i32 %fm15, -5
  %fms15 = or i32 %fm15, 4
  store i32 %fms15, ptr %fmp15, align 4
  call void @df.barrier(i64 %v0, i64 %v12)
  br label %wb15.fim
wb15.fim:
  store i64 0, ptr %gcs0
  %v16 = call i64 @df.dart$3aasync.._asyncStartSync(i64 %v12, i64 %v6)
  %v17 = load i8, ptr %ctx, align 8
  %c3 = zext i8 %v17 to i64
  %v18 = icmp ne i64 %c3, 0
  br i1 %v18, label %b9, label %b8
b7:
  %gcvolta7 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta7, ptr %ctxtopo, align 8
  ret void
b8:
  %gcvolta8 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta8, ptr %ctxtopo, align 8
  ret void
b9:
  %gcvolta9 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta9, ptr %ctxtopo, align 8
  ret void
}

define i64 @dart_main$async$q0fee530a(i64 %v0, i64 %v1, i64 %v2) nounwind {
b0:
  %gcq = alloca { ptr, i64, [6 x i64] }, align 8
  %pilhaq = alloca i8, align 1
  %area = call ptr @df.get_area()
  %ctx = call ptr @dartforge_context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load ptr, ptr %pilhalp, align 8
  %pilhab = icmp ult ptr %pilhaq, %pilhal
  %pilhax = call i1 @llvm.expect.i1(i1 %pilhab, i1 false)
  br i1 %pilhax, label %pilha.estouro, label %pilha.ok
pilha.estouro:
  call void @dartforge_stack_overflow()
  ret i64 0
pilha.ok:
  store { ptr, i64, [6 x i64] } { ptr null, i64 4611686018427387910, [6 x i64] zeroinitializer }, ptr %gcq
  %gcs0 = getelementptr inbounds { ptr, i64, [6 x i64] }, ptr %gcq, i64 0, i32 2, i64 0
  %gcs1 = getelementptr inbounds { ptr, i64, [6 x i64] }, ptr %gcq, i64 0, i32 2, i64 1
  %gcs2 = getelementptr inbounds { ptr, i64, [6 x i64] }, ptr %gcq, i64 0, i32 2, i64 2
  %gcs3 = getelementptr inbounds { ptr, i64, [6 x i64] }, ptr %gcq, i64 0, i32 2, i64 3
  %gcs4 = getelementptr inbounds { ptr, i64, [6 x i64] }, ptr %gcq, i64 0, i32 2, i64 4
  %gcs5 = getelementptr inbounds { ptr, i64, [6 x i64] }, ptr %gcq, i64 0, i32 2, i64 5
  %gcant = load ptr, ptr %ctxtopo, align 8
  store ptr %gcant, ptr %gcq, align 8
  store ptr %gcq, ptr %ctxtopo, align 8
  store i64 %v0, ptr %gcs0
  store i64 %v1, ptr %gcs1
  store i64 %v2, ptr %gcs2
  %ep3 = inttoptr i64 %v0 to ptr
  %eg3 = getelementptr inbounds i8, ptr %ep3, i64 14
  %u3 = load i64, ptr %eg3, align 8
  %v3 = add i64 %u3, 0
  store i64 %v3, ptr %gcs0
  %fcb4 = call ptr @df.body(i64 %v3, ptr %ctx)
  %fp4 = getelementptr inbounds i8, ptr %fcb4, i64 16
  %fg4 = getelementptr inbounds i64, ptr %fp4, i64 0
  %v4 = load i64, ptr %fg4, align 8
  %fcb5 = call ptr @df.body(i64 %v3, ptr %ctx)
  %fp5 = getelementptr inbounds i8, ptr %fcb5, i64 16
  %fg5 = getelementptr inbounds i64, ptr %fp5, i64 1
  %v5 = load i64, ptr %fg5, align 8
  store i64 %v5, ptr %gcs3
  %fcb6 = call ptr @df.body(i64 %v3, ptr %ctx)
  %fp6 = getelementptr inbounds i8, ptr %fcb6, i64 16
  %fg6 = getelementptr inbounds i64, ptr %fp6, i64 2
  %v6 = load i64, ptr %fg6, align 8
  store i64 %v6, ptr %gcs4
  switch i64 %v4, label %b1 [ i64 1, label %b5 i64 2, label %b13 ]
b1:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v52 = call i64 @dartforge_object_new(i64 1073741568, i64 8)
  %fxa52 = add i64 %v52, -2
  %fcb52 = inttoptr i64 %fxa52 to ptr
  %fp52 = getelementptr inbounds i8, ptr %fcb52, i64 16
  %fg52.0 = getelementptr inbounds i64, ptr %fp52, i64 0
  store i64 0, ptr %fg52.0, align 8
  %fmp52.0 = getelementptr inbounds i8, ptr %fcb52, i64 8
  %fm52.0 = load i32, ptr %fmp52.0, align 4
  %fmc52.0 = and i32 %fm52.0, -2
  store i32 %fmc52.0, ptr %fmp52.0, align 4
  %fg52.1 = getelementptr inbounds i64, ptr %fp52, i64 1
  store i64 0, ptr %fg52.1, align 8
  %fmp52.1 = getelementptr inbounds i8, ptr %fcb52, i64 8
  %fm52.1 = load i32, ptr %fmp52.1, align 4
  %fmc52.1 = and i32 %fm52.1, -3
  %fms52.1 = or i32 %fm52.1, 2
  store i32 %fms52.1, ptr %fmp52.1, align 4
  %fg52.2 = getelementptr inbounds i64, ptr %fp52, i64 2
  store i64 0, ptr %fg52.2, align 8
  %fmp52.2 = getelementptr inbounds i8, ptr %fcb52, i64 8
  %fm52.2 = load i32, ptr %fmp52.2, align 4
  %fmc52.2 = and i32 %fm52.2, -5
  %fms52.2 = or i32 %fm52.2, 4
  store i32 %fms52.2, ptr %fmp52.2, align 4
  %fg52.3 = getelementptr inbounds i64, ptr %fp52, i64 3
  store i64 0, ptr %fg52.3, align 8
  %fmp52.3 = getelementptr inbounds i8, ptr %fcb52, i64 8
  %fm52.3 = load i32, ptr %fmp52.3, align 4
  %fmc52.3 = and i32 %fm52.3, -9
  %fms52.3 = or i32 %fm52.3, 8
  store i32 %fms52.3, ptr %fmp52.3, align 4
  %fg52.4 = getelementptr inbounds i64, ptr %fp52, i64 4
  store i64 0, ptr %fg52.4, align 8
  %fmp52.4 = getelementptr inbounds i8, ptr %fcb52, i64 8
  %fm52.4 = load i32, ptr %fmp52.4, align 4
  %fmc52.4 = and i32 %fm52.4, -17
  %fms52.4 = or i32 %fm52.4, 16
  store i32 %fms52.4, ptr %fmp52.4, align 4
  %fg52.5 = getelementptr inbounds i64, ptr %fp52, i64 5
  store i64 0, ptr %fg52.5, align 8
  %fmp52.5 = getelementptr inbounds i8, ptr %fcb52, i64 8
  %fm52.5 = load i32, ptr %fmp52.5, align 4
  %fmc52.5 = and i32 %fm52.5, -33
  store i32 %fmc52.5, ptr %fmp52.5, align 4
  %fg52.6 = getelementptr inbounds i64, ptr %fp52, i64 6
  store i64 0, ptr %fg52.6, align 8
  %fmp52.6 = getelementptr inbounds i8, ptr %fcb52, i64 8
  %fm52.6 = load i32, ptr %fmp52.6, align 4
  %fmc52.6 = and i32 %fm52.6, -65
  store i32 %fmc52.6, ptr %fmp52.6, align 4
  %fg52.7 = getelementptr inbounds i64, ptr %fp52, i64 7
  store i64 0, ptr %fg52.7, align 8
  %fmp52.7 = getelementptr inbounds i8, ptr %fcb52, i64 8
  %fm52.7 = load i32, ptr %fmp52.7, align 4
  %fmc52.7 = and i32 %fm52.7, -129
  store i32 %fmc52.7, ptr %fmp52.7, align 4
  store i64 %v52, ptr %gcs1
  %ga53 = getelementptr i64, ptr %area, i64 3
  %v53 = load i8, ptr %ga53
  %c0 = zext i8 %v53 to i64
  %v54 = icmp ne i64 %c0, 0
  br i1 %v54, label %b23, label %b24
b23:
  %ga55 = getelementptr i64, ptr %area, i64 2
  %v55 = load i64, ptr %ga55
  br label %b25
b24:
  %v56 = call i64 @df.rti.1d66eeb6210799c5.main$2edart()
  br label %b25
b25:
  %v57 = phi i64 [ %v55, %b23 ], [ %v56, %b24 ]
  %v58 = call i64 @df.dart$3aasync.._makeAsyncAwaitCompleter(i64 %v57)
  store i64 %v58, ptr %gcs2
  %v59 = load i8, ptr %ctx, align 8
  %c1 = zext i8 %v59 to i64
  %v60 = icmp ne i64 %c1, 0
  br i1 %v60, label %b27, label %b26
b26:
  %fcb61 = call ptr @df.body(i64 %v52, ptr %ctx)
  %fp61 = getelementptr inbounds i8, ptr %fcb61, i64 16
  %fg61 = getelementptr inbounds i64, ptr %fp61, i64 1
  store i64 %v58, ptr %fg61, align 8
  %fmp61 = getelementptr inbounds i8, ptr %fcb61, i64 8
  %fm61 = load i32, ptr %fmp61, align 4
  %fmc61 = and i32 %fm61, -3
  %fms61 = or i32 %fm61, 2
  store i32 %fms61, ptr %fmp61, align 4
  call void @df.barrier(i64 %v52, i64 %v58)
  br label %wb61.fim
wb61.fim:
  %v62 = call i64 @df.allocate(i64 55834640385, i64 1)
  %bp62 = inttoptr i64 %v62 to ptr
  %bg62_0 = getelementptr inbounds i8, ptr %bp62, i64 14
  store i64 %v52, ptr %bg62_0, align 8
  %bm62 = getelementptr inbounds i8, ptr %bp62, i64 6
  store i32 1, ptr %bm62, align 4
  store i64 %v62, ptr %gcs5
  %v63 = call i64 @df.allocate(i64 47244902401, i64 4)
  %bp63 = inttoptr i64 %v63 to ptr
  %bg63_k = getelementptr inbounds i8, ptr %bp63, i64 14
  store i64 ptrtoint (ptr @df.main$2edart..worker$async$qead0428d$ent to i64), ptr %bg63_k, align 8
  %bg63_e = getelementptr inbounds i8, ptr %bp63, i64 22
  store i64 %v62, ptr %bg63_e, align 8
  %bg63_t = getelementptr inbounds i8, ptr %bp63, i64 30
  store i64 0, ptr %bg63_t, align 8
  %bg63_a = getelementptr inbounds i8, ptr %bp63, i64 38
  store i64 0, ptr %bg63_a, align 8
  %bm63 = getelementptr inbounds i8, ptr %bp63, i64 6
  store i32 2, ptr %bm63, align 4
  store i64 %v63, ptr %gcs5
  %v64 = call i64 @df.dart$3aasync.._wrapBody(i64 %v63)
  store i64 %v64, ptr %gcs5
  %v65 = load i8, ptr %ctx, align 8
  %c2 = zext i8 %v65 to i64
  %v66 = icmp ne i64 %c2, 0
  br i1 %v66, label %b29, label %b28
b27:
  br label %b32
b28:
  %fcb67 = call ptr @df.body(i64 %v52, ptr %ctx)
  %fp67 = getelementptr inbounds i8, ptr %fcb67, i64 16
  %fg67 = getelementptr inbounds i64, ptr %fp67, i64 2
  store i64 %v64, ptr %fg67, align 8
  %fmp67 = getelementptr inbounds i8, ptr %fcb67, i64 8
  %fm67 = load i32, ptr %fmp67, align 4
  %fmc67 = and i32 %fm67, -5
  %fms67 = or i32 %fm67, 4
  store i32 %fms67, ptr %fmp67, align 4
  call void @df.barrier(i64 %v52, i64 %v64)
  br label %wb67.fim
wb67.fim:
  store i64 0, ptr %gcs1
  %v68 = call i64 @df.dart$3aasync.._asyncStartSync(i64 %v64, i64 %v58)
  %v69 = load i8, ptr %ctx, align 8
  %c3 = zext i8 %v69 to i64
  %v70 = icmp ne i64 %c3, 0
  br i1 %v70, label %b31, label %b30
b29:
  br label %b32
b30:
  br label %b32
b31:
  br label %b32
b32:
  %v71 = phi i64 [ 0, %b27 ], [ 0, %b29 ], [ %v68, %b30 ], [ 0, %b31 ]
  store i64 %v71, ptr %gcs1
  %v8 = load i8, ptr %ctx, align 8
  %c4 = zext i8 %v8 to i64
  %v9 = icmp ne i64 %c4, 0
  br i1 %v9, label %b2, label %b3
b2:
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  store i64 0, ptr %gcs4
  store i64 0, ptr %gcs5
  %v46 = call i64 @dartforge_exception_peek_ref()
  store i64 %v46, ptr %gcs0
  %v47 = call i64 @dartforge_stack_trace_get()
  store i64 %v47, ptr %gcs1
  %xcp48 = load i8, ptr %ctx, align 8
  %xcn48 = icmp ne i8 %xcp48, 0
  %xce48 = call i1 @llvm.expect.i1(i1 %xcn48, i1 false)
  br i1 %xce48, label %xc48.limpar, label %xc48.fim
xc48.limpar:
  call void @dartforge_exception_clear()
  br label %xc48.fim
xc48.fim:
  %v49 = call i64 @df.dart$3aasync.._asyncRethrow(i64 %v46, i64 %v47, i64 %v5)
  %v50 = load i8, ptr %ctx, align 8
  %c5 = zext i8 %v50 to i64
  %v51 = icmp ne i64 %c5, 0
  br i1 %v51, label %b21, label %b20
b3:
  %fcb10 = call ptr @df.body(i64 %v3, ptr %ctx)
  %fp10 = getelementptr inbounds i8, ptr %fcb10, i64 16
  %fg10 = getelementptr inbounds i64, ptr %fp10, i64 0
  store i64 1, ptr %fg10, align 8
  %fmp10 = getelementptr inbounds i8, ptr %fcb10, i64 8
  %fm10 = load i32, ptr %fmp10, align 4
  %fmc10 = and i32 %fm10, -2
  store i32 %fmc10, ptr %fmp10, align 4
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs2
  store i64 0, ptr %gcs5
  %v11 = call i64 @df.dart$3aasync.._asyncAwait(i64 %v71, i64 %v6)
  %v12 = load i8, ptr %ctx, align 8
  %c6 = zext i8 %v12 to i64
  %v13 = icmp ne i64 %c6, 0
  br i1 %v13, label %b2, label %b4
b4:
  %gcvolta4 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta4, ptr %ctxtopo, align 8
  ret i64 0
b5:
  %v14 = call i64 @df.unbox_int(i64 %v1)
  %v15 = load i8, ptr %ctx, align 8
  %c7 = zext i8 %v15 to i64
  %v16 = icmp ne i64 %c7, 0
  br i1 %v16, label %b2, label %b6
b6:
  %v17 = icmp eq i64 %v14, 1
  br i1 %v17, label %b7, label %b8
b7:
  %fcb18 = call ptr @df.body(i64 %v2, ptr %ctx)
  %fp18 = getelementptr inbounds i8, ptr %fcb18, i64 16
  %fg18 = getelementptr inbounds i64, ptr %fp18, i64 0
  %v18 = load i64, ptr %fg18, align 8
  store i64 %v18, ptr %gcs0
  %fcb19 = call ptr @df.body(i64 %v2, ptr %ctx)
  %fp19 = getelementptr inbounds i8, ptr %fcb19, i64 16
  %fg19 = getelementptr inbounds i64, ptr %fp19, i64 1
  %v19 = load i64, ptr %fg19, align 8
  store i64 %v19, ptr %gcs1
  store i64 0, ptr %gcs2
  store i64 0, ptr %gcs4
  call void @dartforge_throw_with_stack_trace(i64 %v18, i8 3, i64 %v19)
  br label %b2
b8:
  %v23 = load i8, ptr %ctx, align 8
  %c8 = zext i8 %v23 to i64
  %v24 = icmp ne i64 %c8, 0
  br i1 %v24, label %b2, label %b10
b10:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v72 = call i64 @dartforge_object_new(i64 1073741568, i64 8)
  %fxa72 = add i64 %v72, -2
  %fcb72 = inttoptr i64 %fxa72 to ptr
  %fp72 = getelementptr inbounds i8, ptr %fcb72, i64 16
  %fg72.0 = getelementptr inbounds i64, ptr %fp72, i64 0
  store i64 0, ptr %fg72.0, align 8
  %fmp72.0 = getelementptr inbounds i8, ptr %fcb72, i64 8
  %fm72.0 = load i32, ptr %fmp72.0, align 4
  %fmc72.0 = and i32 %fm72.0, -2
  store i32 %fmc72.0, ptr %fmp72.0, align 4
  %fg72.1 = getelementptr inbounds i64, ptr %fp72, i64 1
  store i64 0, ptr %fg72.1, align 8
  %fmp72.1 = getelementptr inbounds i8, ptr %fcb72, i64 8
  %fm72.1 = load i32, ptr %fmp72.1, align 4
  %fmc72.1 = and i32 %fm72.1, -3
  %fms72.1 = or i32 %fm72.1, 2
  store i32 %fms72.1, ptr %fmp72.1, align 4
  %fg72.2 = getelementptr inbounds i64, ptr %fp72, i64 2
  store i64 0, ptr %fg72.2, align 8
  %fmp72.2 = getelementptr inbounds i8, ptr %fcb72, i64 8
  %fm72.2 = load i32, ptr %fmp72.2, align 4
  %fmc72.2 = and i32 %fm72.2, -5
  %fms72.2 = or i32 %fm72.2, 4
  store i32 %fms72.2, ptr %fmp72.2, align 4
  %fg72.3 = getelementptr inbounds i64, ptr %fp72, i64 3
  store i64 0, ptr %fg72.3, align 8
  %fmp72.3 = getelementptr inbounds i8, ptr %fcb72, i64 8
  %fm72.3 = load i32, ptr %fmp72.3, align 4
  %fmc72.3 = and i32 %fm72.3, -9
  %fms72.3 = or i32 %fm72.3, 8
  store i32 %fms72.3, ptr %fmp72.3, align 4
  %fg72.4 = getelementptr inbounds i64, ptr %fp72, i64 4
  store i64 0, ptr %fg72.4, align 8
  %fmp72.4 = getelementptr inbounds i8, ptr %fcb72, i64 8
  %fm72.4 = load i32, ptr %fmp72.4, align 4
  %fmc72.4 = and i32 %fm72.4, -17
  %fms72.4 = or i32 %fm72.4, 16
  store i32 %fms72.4, ptr %fmp72.4, align 4
  %fg72.5 = getelementptr inbounds i64, ptr %fp72, i64 5
  store i64 0, ptr %fg72.5, align 8
  %fmp72.5 = getelementptr inbounds i8, ptr %fcb72, i64 8
  %fm72.5 = load i32, ptr %fmp72.5, align 4
  %fmc72.5 = and i32 %fm72.5, -33
  store i32 %fmc72.5, ptr %fmp72.5, align 4
  %fg72.6 = getelementptr inbounds i64, ptr %fp72, i64 6
  store i64 0, ptr %fg72.6, align 8
  %fmp72.6 = getelementptr inbounds i8, ptr %fcb72, i64 8
  %fm72.6 = load i32, ptr %fmp72.6, align 4
  %fmc72.6 = and i32 %fm72.6, -65
  store i32 %fmc72.6, ptr %fmp72.6, align 4
  %fg72.7 = getelementptr inbounds i64, ptr %fp72, i64 7
  store i64 0, ptr %fg72.7, align 8
  %fmp72.7 = getelementptr inbounds i8, ptr %fcb72, i64 8
  %fm72.7 = load i32, ptr %fmp72.7, align 4
  %fmc72.7 = and i32 %fm72.7, -129
  store i32 %fmc72.7, ptr %fmp72.7, align 4
  store i64 %v72, ptr %gcs1
  %ga73 = getelementptr i64, ptr %area, i64 3
  %v73 = load i8, ptr %ga73
  %c9 = zext i8 %v73 to i64
  %v74 = icmp ne i64 %c9, 0
  br i1 %v74, label %b34, label %b35
b34:
  %ga75 = getelementptr i64, ptr %area, i64 2
  %v75 = load i64, ptr %ga75
  br label %b36
b35:
  %v76 = call i64 @df.rti.1d66eeb6210799c5.main$2edart()
  br label %b36
b36:
  %v77 = phi i64 [ %v75, %b34 ], [ %v76, %b35 ]
  %v78 = call i64 @df.dart$3aasync.._makeAsyncAwaitCompleter(i64 %v77)
  store i64 %v78, ptr %gcs2
  %v79 = load i8, ptr %ctx, align 8
  %c10 = zext i8 %v79 to i64
  %v80 = icmp ne i64 %c10, 0
  br i1 %v80, label %b38, label %b37
b37:
  %fcb81 = call ptr @df.body(i64 %v72, ptr %ctx)
  %fp81 = getelementptr inbounds i8, ptr %fcb81, i64 16
  %fg81 = getelementptr inbounds i64, ptr %fp81, i64 1
  store i64 %v78, ptr %fg81, align 8
  %fmp81 = getelementptr inbounds i8, ptr %fcb81, i64 8
  %fm81 = load i32, ptr %fmp81, align 4
  %fmc81 = and i32 %fm81, -3
  %fms81 = or i32 %fm81, 2
  store i32 %fms81, ptr %fmp81, align 4
  call void @df.barrier(i64 %v72, i64 %v78)
  br label %wb81.fim
wb81.fim:
  %v82 = call i64 @df.allocate(i64 55834640385, i64 1)
  %bp82 = inttoptr i64 %v82 to ptr
  %bg82_0 = getelementptr inbounds i8, ptr %bp82, i64 14
  store i64 %v72, ptr %bg82_0, align 8
  %bm82 = getelementptr inbounds i8, ptr %bp82, i64 6
  store i32 1, ptr %bm82, align 4
  store i64 %v82, ptr %gcs5
  %v83 = call i64 @df.allocate(i64 47244902401, i64 4)
  %bp83 = inttoptr i64 %v83 to ptr
  %bg83_k = getelementptr inbounds i8, ptr %bp83, i64 14
  store i64 ptrtoint (ptr @df.main$2edart..worker$async$qead0428d$ent to i64), ptr %bg83_k, align 8
  %bg83_e = getelementptr inbounds i8, ptr %bp83, i64 22
  store i64 %v82, ptr %bg83_e, align 8
  %bg83_t = getelementptr inbounds i8, ptr %bp83, i64 30
  store i64 0, ptr %bg83_t, align 8
  %bg83_a = getelementptr inbounds i8, ptr %bp83, i64 38
  store i64 0, ptr %bg83_a, align 8
  %bm83 = getelementptr inbounds i8, ptr %bp83, i64 6
  store i32 2, ptr %bm83, align 4
  store i64 %v83, ptr %gcs5
  %v84 = call i64 @df.dart$3aasync.._wrapBody(i64 %v83)
  store i64 %v84, ptr %gcs5
  %v85 = load i8, ptr %ctx, align 8
  %c11 = zext i8 %v85 to i64
  %v86 = icmp ne i64 %c11, 0
  br i1 %v86, label %b40, label %b39
b38:
  br label %b43
b39:
  %fcb87 = call ptr @df.body(i64 %v72, ptr %ctx)
  %fp87 = getelementptr inbounds i8, ptr %fcb87, i64 16
  %fg87 = getelementptr inbounds i64, ptr %fp87, i64 2
  store i64 %v84, ptr %fg87, align 8
  %fmp87 = getelementptr inbounds i8, ptr %fcb87, i64 8
  %fm87 = load i32, ptr %fmp87, align 4
  %fmc87 = and i32 %fm87, -5
  %fms87 = or i32 %fm87, 4
  store i32 %fms87, ptr %fmp87, align 4
  call void @df.barrier(i64 %v72, i64 %v84)
  br label %wb87.fim
wb87.fim:
  store i64 0, ptr %gcs1
  %v88 = call i64 @df.dart$3aasync.._asyncStartSync(i64 %v84, i64 %v78)
  %v89 = load i8, ptr %ctx, align 8
  %c12 = zext i8 %v89 to i64
  %v90 = icmp ne i64 %c12, 0
  br i1 %v90, label %b42, label %b41
b40:
  br label %b43
b41:
  br label %b43
b42:
  br label %b43
b43:
  %v91 = phi i64 [ 0, %b38 ], [ 0, %b40 ], [ %v88, %b41 ], [ 0, %b42 ]
  store i64 %v91, ptr %gcs1
  %v26 = load i8, ptr %ctx, align 8
  %c13 = zext i8 %v26 to i64
  %v27 = icmp ne i64 %c13, 0
  br i1 %v27, label %b2, label %b11
b11:
  %fcb28 = call ptr @df.body(i64 %v3, ptr %ctx)
  %fp28 = getelementptr inbounds i8, ptr %fcb28, i64 16
  %fg28 = getelementptr inbounds i64, ptr %fp28, i64 0
  store i64 2, ptr %fg28, align 8
  %fmp28 = getelementptr inbounds i8, ptr %fcb28, i64 8
  %fm28 = load i32, ptr %fmp28, align 4
  %fmc28 = and i32 %fm28, -2
  store i32 %fmc28, ptr %fmp28, align 4
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs2
  store i64 0, ptr %gcs5
  %v29 = call i64 @df.dart$3aasync.._asyncAwait(i64 %v91, i64 %v6)
  %v30 = load i8, ptr %ctx, align 8
  %c14 = zext i8 %v30 to i64
  %v31 = icmp ne i64 %c14, 0
  br i1 %v31, label %b2, label %b12
b12:
  %gcvolta12 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta12, ptr %ctxtopo, align 8
  ret i64 0
b13:
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs4
  %v32 = call i64 @df.unbox_int(i64 %v1)
  %v33 = load i8, ptr %ctx, align 8
  %c15 = zext i8 %v33 to i64
  %v34 = icmp ne i64 %c15, 0
  br i1 %v34, label %b2, label %b14
b14:
  %v35 = icmp eq i64 %v32, 1
  br i1 %v35, label %b15, label %b16
b15:
  %fcb36 = call ptr @df.body(i64 %v2, ptr %ctx)
  %fp36 = getelementptr inbounds i8, ptr %fcb36, i64 16
  %fg36 = getelementptr inbounds i64, ptr %fp36, i64 0
  %v36 = load i64, ptr %fg36, align 8
  store i64 %v36, ptr %gcs0
  %fcb37 = call ptr @df.body(i64 %v2, ptr %ctx)
  %fp37 = getelementptr inbounds i8, ptr %fcb37, i64 16
  %fg37 = getelementptr inbounds i64, ptr %fp37, i64 1
  %v37 = load i64, ptr %fg37, align 8
  store i64 %v37, ptr %gcs1
  store i64 0, ptr %gcs2
  call void @dartforge_throw_with_stack_trace(i64 %v36, i8 3, i64 %v37)
  br label %b2
b16:
  %v41 = load i8, ptr %ctx, align 8
  %c16 = zext i8 %v41 to i64
  %v42 = icmp ne i64 %c16, 0
  br i1 %v42, label %b2, label %b18
b18:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v43 = call i64 @df.dart$3aasync.._asyncReturn(i64 0, i64 %v5)
  %v44 = load i8, ptr %ctx, align 8
  %c17 = zext i8 %v44 to i64
  %v45 = icmp ne i64 %c17, 0
  br i1 %v45, label %b2, label %b19
b19:
  %gcvolta19 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta19, ptr %ctxtopo, align 8
  ret i64 0
b20:
  %gcvolta20 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta20, ptr %ctxtopo, align 8
  ret i64 0
b21:
  %gcvolta21 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta21, ptr %ctxtopo, align 8
  ret i64 0
}

define i64 @dart_main$async$q0fee530a$ent(i64 %v0, ptr %v1, ptr %v2) nounwind {
b0:
  %gcq = alloca { ptr, i64, [3 x i64] }, align 8
  %pilhaq = alloca i8, align 1
  %ctx = call ptr @dartforge_context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load ptr, ptr %pilhalp, align 8
  %pilhab = icmp ult ptr %pilhaq, %pilhal
  %pilhax = call i1 @llvm.expect.i1(i1 %pilhab, i1 false)
  br i1 %pilhax, label %pilha.estouro, label %pilha.ok
pilha.estouro:
  call void @dartforge_stack_overflow()
  ret i64 0
pilha.ok:
  store { ptr, i64, [3 x i64] } { ptr null, i64 4611686018427387907, [3 x i64] zeroinitializer }, ptr %gcq
  %gcs0 = getelementptr inbounds { ptr, i64, [3 x i64] }, ptr %gcq, i64 0, i32 2, i64 0
  %gcs1 = getelementptr inbounds { ptr, i64, [3 x i64] }, ptr %gcq, i64 0, i32 2, i64 1
  %gcs2 = getelementptr inbounds { ptr, i64, [3 x i64] }, ptr %gcq, i64 0, i32 2, i64 2
  %gcant = load ptr, ptr %ctxtopo, align 8
  store ptr %gcant, ptr %gcq, align 8
  store ptr %gcq, ptr %ctxtopo, align 8
  %fcb3 = call ptr @df.body(i64 %v0, ptr %ctx)
  %fp3 = getelementptr inbounds i8, ptr %fcb3, i64 16
  %fg3 = getelementptr inbounds i64, ptr %fp3, i64 1
  %v3 = load i64, ptr %fg3, align 8
  store i64 %v3, ptr %gcs0
  %li4 = getelementptr i64, ptr %v2, i64 0
  %v4 = load i64, ptr %li4
  %li5 = getelementptr i64, ptr %v2, i64 1
  %v5 = load i64, ptr %li5
  %v6 = xor i64 %v4, 2
  %v7 = or i64 %v6, %v5
  %v8 = icmp eq i64 %v7, 0
  br i1 %v8, label %b1, label %b2
b1:
  %li10 = getelementptr i64, ptr %v2, i64 0
  %v10 = load i64, ptr %li10
  %li11 = getelementptr i64, ptr %v1, i64 0
  %v11 = load i64, ptr %li11
  store i64 %v11, ptr %gcs1
  %li12 = getelementptr i64, ptr %v1, i64 1
  %v12 = load i64, ptr %li12
  store i64 %v12, ptr %gcs2
  %v13 = call i64 @dart_main$async$q0fee530a(i64 %v3, i64 %v11, i64 %v12)
  %v14 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v14 to i64
  %v15 = icmp ne i64 %c0, 0
  br i1 %v15, label %b4, label %b3
b2:
  store i64 0, ptr %gcs0
  call void @dartforge_nsm_call()
  %gcvolta2 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2, ptr %ctxtopo, align 8
  ret i64 0
b3:
  %gcvolta3 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta3, ptr %ctxtopo, align 8
  ret i64 %v13
b4:
  %gcvolta4 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta4, ptr %ctxtopo, align 8
  ret i64 0
}

define i64 @dartforge_call_dart0(i64 %v0) nounwind {
b0:
  %cargs1 = alloca [1 x i64]
  %gcq = alloca { ptr, i64, [1 x i64] }, align 8
  %pilhaq = alloca i8, align 1
  %ctx = call ptr @dartforge_context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load ptr, ptr %pilhalp, align 8
  %pilhab = icmp ult ptr %pilhaq, %pilhal
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
  %cat1 = getelementptr [1 x i64], ptr %cargs1, i64 0, i64 0
  store i64 0, ptr %cat1
  %cc1 = call i64 @dartforge_closure_entry(i64 %v0)
  %cz1 = icmp eq i64 %cc1, 0
  %cq1 = select i1 %cz1, i64 ptrtoint (ptr @df_clo_invalido to i64), i64 %cc1
  %cf1 = inttoptr i64 %cq1 to ptr
  %v1 = call i64 %cf1(i64 %v0, ptr %cargs1, ptr @df.arr.0)
  %v2 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v2 to i64
  %v3 = icmp ne i64 %c0, 0
  br i1 %v3, label %b2, label %b1
b1:
  %gcvolta1 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1, ptr %ctxtopo, align 8
  ret i64 %v1
b2:
  %gcvolta2 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2, ptr %ctxtopo, align 8
  ret i64 0
}

define i64 @df.$type.toString(i64 %v0) nounwind {
b0:
  %pilhaq = alloca i8, align 1
  %ctx = call ptr @dartforge_context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load ptr, ptr %pilhalp, align 8
  %pilhab = icmp ult ptr %pilhaq, %pilhal
  %pilhax = call i1 @llvm.expect.i1(i1 %pilhab, i1 false)
  br i1 %pilhax, label %pilha.estouro, label %pilha.ok
pilha.estouro:
  call void @dartforge_stack_overflow()
  ret i64 0
pilha.ok:
  %fcb1 = call ptr @df.body(i64 %v0, ptr %ctx)
  %fp1 = getelementptr inbounds i8, ptr %fcb1, i64 16
  %fg1 = getelementptr inbounds i64, ptr %fp1, i64 0
  %v1 = load i64, ptr %fg1, align 8
  %v2 = call i64 @dartforge_rti_string(i64 %v1)
  ret i64 %v2
}

define i64 @df.$type.$3d$3d$c(i64 %v0, ptr %v1, ptr %v2) nounwind {
b0:
  %gcq = alloca { ptr, i64, [2 x i64] }, align 8
  %pilhaq = alloca i8, align 1
  %ctx = call ptr @dartforge_context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load ptr, ptr %pilhalp, align 8
  %pilhab = icmp ult ptr %pilhaq, %pilhal
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

define i64 @df.$type.toString$c(i64 %v0, ptr %v1, ptr %v2) nounwind {
b0:
  %pilhaq = alloca i8, align 1
  %ctx = call ptr @dartforge_context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load ptr, ptr %pilhalp, align 8
  %pilhab = icmp ult ptr %pilhaq, %pilhal
  %pilhax = call i1 @llvm.expect.i1(i1 %pilhab, i1 false)
  br i1 %pilhax, label %pilha.estouro, label %pilha.ok
pilha.estouro:
  call void @dartforge_stack_overflow()
  ret i64 0
pilha.ok:
  %fcb6 = call ptr @df.body(i64 %v0, ptr %ctx)
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

define void @dartforge_rti_init() nounwind {
b0:
  %v0 = getelementptr i64, ptr @df.arr.3, i64 0
  call void @dartforge_rti_init_table(ptr %v0, i64 24443)
  ret void
}

define i64 @df.ffi.v_lp$ent(i64 %v0, ptr %v1, ptr %v2) nounwind {
b0:
  %gcq = alloca { ptr, i64, [3 x i64] }, align 8
  %pilhaq = alloca i8, align 1
  %ctx = call ptr @dartforge_context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load ptr, ptr %pilhalp, align 8
  %pilhab = icmp ult ptr %pilhaq, %pilhal
  %pilhax = call i1 @llvm.expect.i1(i1 %pilhab, i1 false)
  br i1 %pilhax, label %pilha.estouro, label %pilha.ok
pilha.estouro:
  call void @dartforge_stack_overflow()
  ret i64 0
pilha.ok:
  store { ptr, i64, [3 x i64] } { ptr null, i64 4611686018427387907, [3 x i64] zeroinitializer }, ptr %gcq
  %gcs0 = getelementptr inbounds { ptr, i64, [3 x i64] }, ptr %gcq, i64 0, i32 2, i64 0
  %gcs1 = getelementptr inbounds { ptr, i64, [3 x i64] }, ptr %gcq, i64 0, i32 2, i64 1
  %gcs2 = getelementptr inbounds { ptr, i64, [3 x i64] }, ptr %gcq, i64 0, i32 2, i64 2
  %gcant = load ptr, ptr %ctxtopo, align 8
  store ptr %gcant, ptr %gcq, align 8
  store ptr %gcq, ptr %ctxtopo, align 8
  store i64 %v0, ptr %gcs0
  %li3 = getelementptr i64, ptr %v2, i64 0
  %v3 = load i64, ptr %li3
  %li4 = getelementptr i64, ptr %v2, i64 1
  %v4 = load i64, ptr %li4
  %v5 = xor i64 %v3, 2
  %v6 = or i64 %v5, %v4
  %v7 = icmp eq i64 %v6, 0
  br i1 %v7, label %b1, label %b2
b1:
  %li9 = getelementptr i64, ptr %v2, i64 0
  %v9 = load i64, ptr %li9
  %li10 = getelementptr i64, ptr %v1, i64 0
  %v10 = load i64, ptr %li10
  store i64 %v10, ptr %gcs1
  %li11 = getelementptr i64, ptr %v1, i64 1
  %v11 = load i64, ptr %li11
  store i64 %v11, ptr %gcs2
  %v12 = call i64 @dartforge_ffi_closure_address(i64 %v0)
  %v13 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v13 to i64
  %v14 = icmp ne i64 %c0, 0
  br i1 %v14, label %b4, label %b3
b2:
  store i64 0, ptr %gcs0
  call void @dartforge_nsm_call()
  %gcvolta2 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2, ptr %ctxtopo, align 8
  ret i64 0
b3:
  store i64 0, ptr %gcs0
  %v15 = call i64 @df.unbox_int(i64 %v10)
  %v16 = load i8, ptr %ctx, align 8
  %c1 = zext i8 %v16 to i64
  %v17 = icmp ne i64 %c1, 0
  br i1 %v17, label %b6, label %b5
b4:
  %gcvolta4 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta4, ptr %ctxtopo, align 8
  ret i64 0
b5:
  store i64 0, ptr %gcs1
  %v18 = call i64 @dartforge_ffi_pointer_address(i64 %v11)
  %v19 = load i8, ptr %ctx, align 8
  %c2 = zext i8 %v19 to i64
  %v20 = icmp ne i64 %c2, 0
  br i1 %v20, label %b8, label %b7
b6:
  %gcvolta6 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta6, ptr %ctxtopo, align 8
  ret i64 0
b7:
  store i64 0, ptr %gcs2
  %fn21 = inttoptr i64 %v12 to ptr
  %na21_1 = inttoptr i64 %v18 to ptr
  call void %fn21(i64 %v15, ptr %na21_1)
  %gcvolta7 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta7, ptr %ctxtopo, align 8
  ret i64 0
b8:
  %gcvolta8 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta8, ptr %ctxtopo, align 8
  ret i64 0
}

define void @df.ffi.v_lp$cb(i64 %v0, i64 %v1, i64 %v2) nounwind {
b0:
  %cargs8 = alloca [3 x i64]
  %gcq = alloca { ptr, i64, [3 x i64] }, align 8
  %pilhaq = alloca i8, align 1
  %ctx = call ptr @dartforge_context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load ptr, ptr %pilhalp, align 8
  %pilhab = icmp ult ptr %pilhaq, %pilhal
  %pilhax = call i1 @llvm.expect.i1(i1 %pilhab, i1 false)
  br i1 %pilhax, label %pilha.estouro, label %pilha.ok
pilha.estouro:
  call void @dartforge_stack_overflow()
  ret void
pilha.ok:
  store { ptr, i64, [3 x i64] } { ptr null, i64 4611686018427387907, [3 x i64] zeroinitializer }, ptr %gcq
  %gcs0 = getelementptr inbounds { ptr, i64, [3 x i64] }, ptr %gcq, i64 0, i32 2, i64 0
  %gcs1 = getelementptr inbounds { ptr, i64, [3 x i64] }, ptr %gcq, i64 0, i32 2, i64 1
  %gcs2 = getelementptr inbounds { ptr, i64, [3 x i64] }, ptr %gcq, i64 0, i32 2, i64 2
  %gcant = load ptr, ptr %ctxtopo, align 8
  store ptr %gcant, ptr %gcq, align 8
  store ptr %gcq, ptr %ctxtopo, align 8
  %v3 = call i64 @dartforge_ffi_callback_closure(i64 %v0)
  store i64 %v3, ptr %gcs0
  %v4 = call i64 @df.box_int(i64 %v1)
  store i64 %v4, ptr %gcs1
  %v5 = call i64 @dartforge_ffi_callback_pointer(i64 %v0, i64 1, i64 %v2)
  store i64 %v5, ptr %gcs2
  %v6 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v6 to i64
  %v7 = icmp ne i64 %c0, 0
  br i1 %v7, label %b2, label %b1
b1:
  %ca8_0 = getelementptr [3 x i64], ptr %cargs8, i64 0, i64 0
  store i64 %v4, ptr %ca8_0
  %ca8_1 = getelementptr [3 x i64], ptr %cargs8, i64 0, i64 1
  store i64 %v5, ptr %ca8_1
  %cat8 = getelementptr [3 x i64], ptr %cargs8, i64 0, i64 2
  store i64 0, ptr %cat8
  %cc8 = call i64 @dartforge_closure_entry(i64 %v3)
  %cz8 = icmp eq i64 %cc8, 0
  %cq8 = select i1 %cz8, i64 ptrtoint (ptr @df_clo_invalido to i64), i64 %cc8
  %cf8 = inttoptr i64 %cq8 to ptr
  %v8 = call i64 %cf8(i64 %v3, ptr %cargs8, ptr @df.arr.4)
  %v9 = load i8, ptr %ctx, align 8
  %c1 = zext i8 %v9 to i64
  %v10 = icmp ne i64 %c1, 0
  br i1 %v10, label %b4, label %b3
b2:
  %gcvolta2 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2, ptr %ctxtopo, align 8
  ret void
b3:
  %gcvolta3 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta3, ptr %ctxtopo, align 8
  ret void
b4:
  %gcvolta4 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta4, ptr %ctxtopo, align 8
  ret void
}

define i64 @df.ffi.v_p$ent(i64 %v0, ptr %v1, ptr %v2) nounwind {
b0:
  %gcq = alloca { ptr, i64, [2 x i64] }, align 8
  %pilhaq = alloca i8, align 1
  %ctx = call ptr @dartforge_context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load ptr, ptr %pilhalp, align 8
  %pilhab = icmp ult ptr %pilhaq, %pilhal
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
  %li3 = getelementptr i64, ptr %v2, i64 0
  %v3 = load i64, ptr %li3
  %li4 = getelementptr i64, ptr %v2, i64 1
  %v4 = load i64, ptr %li4
  %v5 = xor i64 %v3, 1
  %v6 = or i64 %v5, %v4
  %v7 = icmp eq i64 %v6, 0
  br i1 %v7, label %b1, label %b2
b1:
  %li9 = getelementptr i64, ptr %v2, i64 0
  %v9 = load i64, ptr %li9
  %li10 = getelementptr i64, ptr %v1, i64 0
  %v10 = load i64, ptr %li10
  store i64 %v10, ptr %gcs1
  %v11 = call i64 @dartforge_ffi_closure_address(i64 %v0)
  %v12 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v12 to i64
  %v13 = icmp ne i64 %c0, 0
  br i1 %v13, label %b4, label %b3
b2:
  store i64 0, ptr %gcs0
  call void @dartforge_nsm_call()
  %gcvolta2 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2, ptr %ctxtopo, align 8
  ret i64 0
b3:
  store i64 0, ptr %gcs0
  %v14 = call i64 @dartforge_ffi_pointer_address(i64 %v10)
  %v15 = load i8, ptr %ctx, align 8
  %c1 = zext i8 %v15 to i64
  %v16 = icmp ne i64 %c1, 0
  br i1 %v16, label %b6, label %b5
b4:
  %gcvolta4 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta4, ptr %ctxtopo, align 8
  ret i64 0
b5:
  store i64 0, ptr %gcs1
  %fn17 = inttoptr i64 %v11 to ptr
  %na17_0 = inttoptr i64 %v14 to ptr
  call void %fn17(ptr %na17_0)
  %gcvolta5 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta5, ptr %ctxtopo, align 8
  ret i64 0
b6:
  %gcvolta6 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta6, ptr %ctxtopo, align 8
  ret i64 0
}

define void @df.ffi.v_p$cb(i64 %v0, i64 %v1) nounwind {
b0:
  %cargs6 = alloca [2 x i64]
  %gcq = alloca { ptr, i64, [2 x i64] }, align 8
  %pilhaq = alloca i8, align 1
  %ctx = call ptr @dartforge_context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load ptr, ptr %pilhalp, align 8
  %pilhab = icmp ult ptr %pilhaq, %pilhal
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
  %v2 = call i64 @dartforge_ffi_callback_closure(i64 %v0)
  store i64 %v2, ptr %gcs0
  %v3 = call i64 @dartforge_ffi_callback_pointer(i64 %v0, i64 0, i64 %v1)
  store i64 %v3, ptr %gcs1
  %v4 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v4 to i64
  %v5 = icmp ne i64 %c0, 0
  br i1 %v5, label %b2, label %b1
b1:
  %ca6_0 = getelementptr [2 x i64], ptr %cargs6, i64 0, i64 0
  store i64 %v3, ptr %ca6_0
  %cat6 = getelementptr [2 x i64], ptr %cargs6, i64 0, i64 1
  store i64 0, ptr %cat6
  %cc6 = call i64 @dartforge_closure_entry(i64 %v2)
  %cz6 = icmp eq i64 %cc6, 0
  %cq6 = select i1 %cz6, i64 ptrtoint (ptr @df_clo_invalido to i64), i64 %cc6
  %cf6 = inttoptr i64 %cq6 to ptr
  %v6 = call i64 %cf6(i64 %v2, ptr %cargs6, ptr @df.arr.1)
  %v7 = load i8, ptr %ctx, align 8
  %c1 = zext i8 %v7 to i64
  %v8 = icmp ne i64 %c1, 0
  br i1 %v8, label %b4, label %b3
b2:
  %gcvolta2 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2, ptr %ctxtopo, align 8
  ret void
b3:
  %gcvolta3 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta3, ptr %ctxtopo, align 8
  ret void
b4:
  %gcvolta4 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta4, ptr %ctxtopo, align 8
  ret void
}

define i64 @df.ffi.p_$ent(i64 %v0, ptr %v1, ptr %v2) nounwind {
b0:
  %gcq = alloca { ptr, i64, [1 x i64] }, align 8
  %pilhaq = alloca i8, align 1
  %ctx = call ptr @dartforge_context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load ptr, ptr %pilhalp, align 8
  %pilhab = icmp ult ptr %pilhaq, %pilhal
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
  %li3 = getelementptr i64, ptr %v2, i64 0
  %v3 = load i64, ptr %li3
  %li4 = getelementptr i64, ptr %v2, i64 1
  %v4 = load i64, ptr %li4
  %v5 = xor i64 %v3, 0
  %v6 = or i64 %v5, %v4
  %v7 = icmp eq i64 %v6, 0
  br i1 %v7, label %b1, label %b2
b1:
  %li9 = getelementptr i64, ptr %v2, i64 0
  %v9 = load i64, ptr %li9
  %v10 = call i64 @dartforge_ffi_closure_address(i64 %v0)
  %v11 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v11 to i64
  %v12 = icmp ne i64 %c0, 0
  br i1 %v12, label %b4, label %b3
b2:
  store i64 0, ptr %gcs0
  call void @dartforge_nsm_call()
  %gcvolta2 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2, ptr %ctxtopo, align 8
  ret i64 0
b3:
  %fn13 = inttoptr i64 %v10 to ptr
  %nr13 = call ptr %fn13()
  %v13 = ptrtoint ptr %nr13 to i64
  %v14 = call i64 @dartforge_ffi_return_pointer(i64 %v13, i64 %v0)
  %v15 = load i8, ptr %ctx, align 8
  %c1 = zext i8 %v15 to i64
  %v16 = icmp ne i64 %c1, 0
  br i1 %v16, label %b6, label %b5
b4:
  %gcvolta4 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta4, ptr %ctxtopo, align 8
  ret i64 0
b5:
  %gcvolta5 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta5, ptr %ctxtopo, align 8
  ret i64 %v14
b6:
  %gcvolta6 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta6, ptr %ctxtopo, align 8
  ret i64 0
}

define i64 @df.ffi.p_$cb(i64 %v0) nounwind {
b0:
  %cargs2 = alloca [1 x i64]
  %gcq = alloca { ptr, i64, [1 x i64] }, align 8
  %pilhaq = alloca i8, align 1
  %ctx = call ptr @dartforge_context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load ptr, ptr %pilhalp, align 8
  %pilhab = icmp ult ptr %pilhaq, %pilhal
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
  %v1 = call i64 @dartforge_ffi_callback_closure(i64 %v0)
  store i64 %v1, ptr %gcs0
  %cat2 = getelementptr [1 x i64], ptr %cargs2, i64 0, i64 0
  store i64 0, ptr %cat2
  %cc2 = call i64 @dartforge_closure_entry(i64 %v1)
  %cz2 = icmp eq i64 %cc2, 0
  %cq2 = select i1 %cz2, i64 ptrtoint (ptr @df_clo_invalido to i64), i64 %cc2
  %cf2 = inttoptr i64 %cq2 to ptr
  %v2 = call i64 %cf2(i64 %v1, ptr %cargs2, ptr @df.arr.0)
  store i64 %v2, ptr %gcs0
  %v3 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v3 to i64
  %v4 = icmp ne i64 %c0, 0
  br i1 %v4, label %b2, label %b1
b1:
  %v5 = call i64 @dartforge_ffi_pointer_address(i64 %v2)
  %v6 = load i8, ptr %ctx, align 8
  %c1 = zext i8 %v6 to i64
  %v7 = icmp ne i64 %c1, 0
  br i1 %v7, label %b4, label %b3
b2:
  %gcvolta2 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2, ptr %ctxtopo, align 8
  ret i64 0
b3:
  %gcvolta3 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta3, ptr %ctxtopo, align 8
  ret i64 %v5
b4:
  %gcvolta4 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta4, ptr %ctxtopo, align 8
  ret i64 0
}

define i64 @df.ffi.a_lp$ent(i64 %v0, ptr %v1, ptr %v2) nounwind {
b0:
  %gcq = alloca { ptr, i64, [3 x i64] }, align 8
  %pilhaq = alloca i8, align 1
  %ctx = call ptr @dartforge_context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load ptr, ptr %pilhalp, align 8
  %pilhab = icmp ult ptr %pilhaq, %pilhal
  %pilhax = call i1 @llvm.expect.i1(i1 %pilhab, i1 false)
  br i1 %pilhax, label %pilha.estouro, label %pilha.ok
pilha.estouro:
  call void @dartforge_stack_overflow()
  ret i64 0
pilha.ok:
  store { ptr, i64, [3 x i64] } { ptr null, i64 4611686018427387907, [3 x i64] zeroinitializer }, ptr %gcq
  %gcs0 = getelementptr inbounds { ptr, i64, [3 x i64] }, ptr %gcq, i64 0, i32 2, i64 0
  %gcs1 = getelementptr inbounds { ptr, i64, [3 x i64] }, ptr %gcq, i64 0, i32 2, i64 1
  %gcs2 = getelementptr inbounds { ptr, i64, [3 x i64] }, ptr %gcq, i64 0, i32 2, i64 2
  %gcant = load ptr, ptr %ctxtopo, align 8
  store ptr %gcant, ptr %gcq, align 8
  store ptr %gcq, ptr %ctxtopo, align 8
  store i64 %v0, ptr %gcs0
  %li3 = getelementptr i64, ptr %v2, i64 0
  %v3 = load i64, ptr %li3
  %li4 = getelementptr i64, ptr %v2, i64 1
  %v4 = load i64, ptr %li4
  %v5 = xor i64 %v3, 2
  %v6 = or i64 %v5, %v4
  %v7 = icmp eq i64 %v6, 0
  br i1 %v7, label %b1, label %b2
b1:
  %li9 = getelementptr i64, ptr %v2, i64 0
  %v9 = load i64, ptr %li9
  %li10 = getelementptr i64, ptr %v1, i64 0
  %v10 = load i64, ptr %li10
  store i64 %v10, ptr %gcs1
  %li11 = getelementptr i64, ptr %v1, i64 1
  %v11 = load i64, ptr %li11
  store i64 %v11, ptr %gcs2
  %v12 = call i64 @dartforge_ffi_closure_address(i64 %v0)
  %v13 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v13 to i64
  %v14 = icmp ne i64 %c0, 0
  br i1 %v14, label %b4, label %b3
b2:
  store i64 0, ptr %gcs0
  call void @dartforge_nsm_call()
  %gcvolta2 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2, ptr %ctxtopo, align 8
  ret i64 0
b3:
  store i64 0, ptr %gcs0
  %v15 = call i64 @df.unbox_int(i64 %v10)
  %v16 = load i8, ptr %ctx, align 8
  %c1 = zext i8 %v16 to i64
  %v17 = icmp ne i64 %c1, 0
  br i1 %v17, label %b6, label %b5
b4:
  %gcvolta4 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta4, ptr %ctxtopo, align 8
  ret i64 0
b5:
  store i64 0, ptr %gcs1
  %v18 = call i64 @dartforge_ffi_pointer_address(i64 %v11)
  %v19 = load i8, ptr %ctx, align 8
  %c2 = zext i8 %v19 to i64
  %v20 = icmp ne i64 %c2, 0
  br i1 %v20, label %b8, label %b7
b6:
  %gcvolta6 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta6, ptr %ctxtopo, align 8
  ret i64 0
b7:
  store i64 0, ptr %gcs2
  %fn21 = inttoptr i64 %v12 to ptr
  %na21_1 = inttoptr i64 %v18 to ptr
  %nr21 = call i8 %fn21(i64 %v15, ptr %na21_1)
  %v21 = sext i8 %nr21 to i64
  %v22 = call i64 @df.box_int(i64 %v21)
  %gcvolta7 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta7, ptr %ctxtopo, align 8
  ret i64 %v22
b8:
  %gcvolta8 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta8, ptr %ctxtopo, align 8
  ret i64 0
}

define i64 @df.ffi.a_lp$cb(i64 %v0, i64 %v1, i64 %v2) nounwind {
b0:
  %cargs8 = alloca [3 x i64]
  %gcq = alloca { ptr, i64, [3 x i64] }, align 8
  %pilhaq = alloca i8, align 1
  %ctx = call ptr @dartforge_context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load ptr, ptr %pilhalp, align 8
  %pilhab = icmp ult ptr %pilhaq, %pilhal
  %pilhax = call i1 @llvm.expect.i1(i1 %pilhab, i1 false)
  br i1 %pilhax, label %pilha.estouro, label %pilha.ok
pilha.estouro:
  call void @dartforge_stack_overflow()
  ret i64 0
pilha.ok:
  store { ptr, i64, [3 x i64] } { ptr null, i64 4611686018427387907, [3 x i64] zeroinitializer }, ptr %gcq
  %gcs0 = getelementptr inbounds { ptr, i64, [3 x i64] }, ptr %gcq, i64 0, i32 2, i64 0
  %gcs1 = getelementptr inbounds { ptr, i64, [3 x i64] }, ptr %gcq, i64 0, i32 2, i64 1
  %gcs2 = getelementptr inbounds { ptr, i64, [3 x i64] }, ptr %gcq, i64 0, i32 2, i64 2
  %gcant = load ptr, ptr %ctxtopo, align 8
  store ptr %gcant, ptr %gcq, align 8
  store ptr %gcq, ptr %ctxtopo, align 8
  %v3 = call i64 @dartforge_ffi_callback_closure(i64 %v0)
  store i64 %v3, ptr %gcs0
  %v4 = call i64 @df.box_int(i64 %v1)
  store i64 %v4, ptr %gcs1
  %v5 = call i64 @dartforge_ffi_callback_pointer(i64 %v0, i64 1, i64 %v2)
  store i64 %v5, ptr %gcs2
  %v6 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v6 to i64
  %v7 = icmp ne i64 %c0, 0
  br i1 %v7, label %b2, label %b1
b1:
  %ca8_0 = getelementptr [3 x i64], ptr %cargs8, i64 0, i64 0
  store i64 %v4, ptr %ca8_0
  %ca8_1 = getelementptr [3 x i64], ptr %cargs8, i64 0, i64 1
  store i64 %v5, ptr %ca8_1
  %cat8 = getelementptr [3 x i64], ptr %cargs8, i64 0, i64 2
  store i64 0, ptr %cat8
  %cc8 = call i64 @dartforge_closure_entry(i64 %v3)
  %cz8 = icmp eq i64 %cc8, 0
  %cq8 = select i1 %cz8, i64 ptrtoint (ptr @df_clo_invalido to i64), i64 %cc8
  %cf8 = inttoptr i64 %cq8 to ptr
  %v8 = call i64 %cf8(i64 %v3, ptr %cargs8, ptr @df.arr.4)
  store i64 %v8, ptr %gcs0
  %v9 = load i8, ptr %ctx, align 8
  %c1 = zext i8 %v9 to i64
  %v10 = icmp ne i64 %c1, 0
  br i1 %v10, label %b4, label %b3
b2:
  %gcvolta2 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2, ptr %ctxtopo, align 8
  ret i64 0
b3:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v11 = call i64 @df.unbox_int(i64 %v8)
  %v12 = load i8, ptr %ctx, align 8
  %c2 = zext i8 %v12 to i64
  %v13 = icmp ne i64 %c2, 0
  br i1 %v13, label %b6, label %b5
b4:
  %gcvolta4 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta4, ptr %ctxtopo, align 8
  ret i64 0
b5:
  %gcvolta5 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta5, ptr %ctxtopo, align 8
  ret i64 %v11
b6:
  %gcvolta6 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta6, ptr %ctxtopo, align 8
  ret i64 0
}

define i64 @df.ffi.l_ppa$ent(i64 %v0, ptr %v1, ptr %v2) nounwind {
b0:
  %gcq = alloca { ptr, i64, [4 x i64] }, align 8
  %pilhaq = alloca i8, align 1
  %ctx = call ptr @dartforge_context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load ptr, ptr %pilhalp, align 8
  %pilhab = icmp ult ptr %pilhaq, %pilhal
  %pilhax = call i1 @llvm.expect.i1(i1 %pilhab, i1 false)
  br i1 %pilhax, label %pilha.estouro, label %pilha.ok
pilha.estouro:
  call void @dartforge_stack_overflow()
  ret i64 0
pilha.ok:
  store { ptr, i64, [4 x i64] } { ptr null, i64 4611686018427387908, [4 x i64] zeroinitializer }, ptr %gcq
  %gcs0 = getelementptr inbounds { ptr, i64, [4 x i64] }, ptr %gcq, i64 0, i32 2, i64 0
  %gcs1 = getelementptr inbounds { ptr, i64, [4 x i64] }, ptr %gcq, i64 0, i32 2, i64 1
  %gcs2 = getelementptr inbounds { ptr, i64, [4 x i64] }, ptr %gcq, i64 0, i32 2, i64 2
  %gcs3 = getelementptr inbounds { ptr, i64, [4 x i64] }, ptr %gcq, i64 0, i32 2, i64 3
  %gcant = load ptr, ptr %ctxtopo, align 8
  store ptr %gcant, ptr %gcq, align 8
  store ptr %gcq, ptr %ctxtopo, align 8
  store i64 %v0, ptr %gcs0
  %li3 = getelementptr i64, ptr %v2, i64 0
  %v3 = load i64, ptr %li3
  %li4 = getelementptr i64, ptr %v2, i64 1
  %v4 = load i64, ptr %li4
  %v5 = xor i64 %v3, 3
  %v6 = or i64 %v5, %v4
  %v7 = icmp eq i64 %v6, 0
  br i1 %v7, label %b1, label %b2
b1:
  %li9 = getelementptr i64, ptr %v2, i64 0
  %v9 = load i64, ptr %li9
  %li10 = getelementptr i64, ptr %v1, i64 0
  %v10 = load i64, ptr %li10
  store i64 %v10, ptr %gcs1
  %li11 = getelementptr i64, ptr %v1, i64 1
  %v11 = load i64, ptr %li11
  store i64 %v11, ptr %gcs2
  %li12 = getelementptr i64, ptr %v1, i64 2
  %v12 = load i64, ptr %li12
  store i64 %v12, ptr %gcs3
  %v13 = call i64 @dartforge_ffi_closure_address(i64 %v0)
  %v14 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v14 to i64
  %v15 = icmp ne i64 %c0, 0
  br i1 %v15, label %b4, label %b3
b2:
  store i64 0, ptr %gcs0
  call void @dartforge_nsm_call()
  %gcvolta2 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2, ptr %ctxtopo, align 8
  ret i64 0
b3:
  store i64 0, ptr %gcs0
  %v16 = call i64 @dartforge_ffi_pointer_address(i64 %v10)
  %v17 = load i8, ptr %ctx, align 8
  %c1 = zext i8 %v17 to i64
  %v18 = icmp ne i64 %c1, 0
  br i1 %v18, label %b6, label %b5
b4:
  %gcvolta4 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta4, ptr %ctxtopo, align 8
  ret i64 0
b5:
  store i64 0, ptr %gcs1
  %v19 = call i64 @dartforge_ffi_pointer_address(i64 %v11)
  %v20 = load i8, ptr %ctx, align 8
  %c2 = zext i8 %v20 to i64
  %v21 = icmp ne i64 %c2, 0
  br i1 %v21, label %b8, label %b7
b6:
  %gcvolta6 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta6, ptr %ctxtopo, align 8
  ret i64 0
b7:
  store i64 0, ptr %gcs2
  %v22 = call i64 @df.unbox_int(i64 %v12)
  %v23 = load i8, ptr %ctx, align 8
  %c3 = zext i8 %v23 to i64
  %v24 = icmp ne i64 %c3, 0
  br i1 %v24, label %b10, label %b9
b8:
  %gcvolta8 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta8, ptr %ctxtopo, align 8
  ret i64 0
b9:
  store i64 0, ptr %gcs3
  %fn25 = inttoptr i64 %v13 to ptr
  %na25_0 = inttoptr i64 %v16 to ptr
  %na25_1 = inttoptr i64 %v19 to ptr
  %na25_2 = trunc i64 %v22 to i8
  %v25 = call i64 %fn25(ptr %na25_0, ptr %na25_1, i8 signext %na25_2)
  %v26 = call i64 @df.box_int(i64 %v25)
  %gcvolta9 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta9, ptr %ctxtopo, align 8
  ret i64 %v26
b10:
  %gcvolta10 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta10, ptr %ctxtopo, align 8
  ret i64 0
}

define i64 @df.ffi.l_ppa$cb(i64 %v0, i64 %v1, i64 %v2, i64 %v3) nounwind {
b0:
  %cargs12 = alloca [4 x i64]
  %gcq = alloca { ptr, i64, [4 x i64] }, align 8
  %pilhaq = alloca i8, align 1
  %ctx = call ptr @dartforge_context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load ptr, ptr %pilhalp, align 8
  %pilhab = icmp ult ptr %pilhaq, %pilhal
  %pilhax = call i1 @llvm.expect.i1(i1 %pilhab, i1 false)
  br i1 %pilhax, label %pilha.estouro, label %pilha.ok
pilha.estouro:
  call void @dartforge_stack_overflow()
  ret i64 0
pilha.ok:
  store { ptr, i64, [4 x i64] } { ptr null, i64 4611686018427387908, [4 x i64] zeroinitializer }, ptr %gcq
  %gcs0 = getelementptr inbounds { ptr, i64, [4 x i64] }, ptr %gcq, i64 0, i32 2, i64 0
  %gcs1 = getelementptr inbounds { ptr, i64, [4 x i64] }, ptr %gcq, i64 0, i32 2, i64 1
  %gcs2 = getelementptr inbounds { ptr, i64, [4 x i64] }, ptr %gcq, i64 0, i32 2, i64 2
  %gcs3 = getelementptr inbounds { ptr, i64, [4 x i64] }, ptr %gcq, i64 0, i32 2, i64 3
  %gcant = load ptr, ptr %ctxtopo, align 8
  store ptr %gcant, ptr %gcq, align 8
  store ptr %gcq, ptr %ctxtopo, align 8
  %v4 = call i64 @dartforge_ffi_callback_closure(i64 %v0)
  store i64 %v4, ptr %gcs0
  %v5 = call i64 @dartforge_ffi_callback_pointer(i64 %v0, i64 0, i64 %v1)
  store i64 %v5, ptr %gcs1
  %v6 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v6 to i64
  %v7 = icmp ne i64 %c0, 0
  br i1 %v7, label %b2, label %b1
b1:
  %v8 = call i64 @dartforge_ffi_callback_pointer(i64 %v0, i64 1, i64 %v2)
  store i64 %v8, ptr %gcs2
  %v9 = load i8, ptr %ctx, align 8
  %c1 = zext i8 %v9 to i64
  %v10 = icmp ne i64 %c1, 0
  br i1 %v10, label %b4, label %b3
b2:
  %gcvolta2 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2, ptr %ctxtopo, align 8
  ret i64 0
b3:
  %v11 = call i64 @df.box_int(i64 %v3)
  store i64 %v11, ptr %gcs3
  %ca12_0 = getelementptr [4 x i64], ptr %cargs12, i64 0, i64 0
  store i64 %v5, ptr %ca12_0
  %ca12_1 = getelementptr [4 x i64], ptr %cargs12, i64 0, i64 1
  store i64 %v8, ptr %ca12_1
  %ca12_2 = getelementptr [4 x i64], ptr %cargs12, i64 0, i64 2
  store i64 %v11, ptr %ca12_2
  %cat12 = getelementptr [4 x i64], ptr %cargs12, i64 0, i64 3
  store i64 0, ptr %cat12
  %cc12 = call i64 @dartforge_closure_entry(i64 %v4)
  %cz12 = icmp eq i64 %cc12, 0
  %cq12 = select i1 %cz12, i64 ptrtoint (ptr @df_clo_invalido to i64), i64 %cc12
  %cf12 = inttoptr i64 %cq12 to ptr
  %v12 = call i64 %cf12(i64 %v4, ptr %cargs12, ptr @df.arr.5)
  store i64 %v12, ptr %gcs0
  %v13 = load i8, ptr %ctx, align 8
  %c2 = zext i8 %v13 to i64
  %v14 = icmp ne i64 %c2, 0
  br i1 %v14, label %b6, label %b5
b4:
  %gcvolta4 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta4, ptr %ctxtopo, align 8
  ret i64 0
b5:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  store i64 0, ptr %gcs3
  %v15 = call i64 @df.unbox_int(i64 %v12)
  %v16 = load i8, ptr %ctx, align 8
  %c3 = zext i8 %v16 to i64
  %v17 = icmp ne i64 %c3, 0
  br i1 %v17, label %b8, label %b7
b6:
  %gcvolta6 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta6, ptr %ctxtopo, align 8
  ret i64 0
b7:
  %gcvolta7 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta7, ptr %ctxtopo, align 8
  ret i64 %v15
b8:
  %gcvolta8 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta8, ptr %ctxtopo, align 8
  ret i64 0
}

define i64 @df.ffi.a_l$ent(i64 %v0, ptr %v1, ptr %v2) nounwind {
b0:
  %gcq = alloca { ptr, i64, [2 x i64] }, align 8
  %pilhaq = alloca i8, align 1
  %ctx = call ptr @dartforge_context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load ptr, ptr %pilhalp, align 8
  %pilhab = icmp ult ptr %pilhaq, %pilhal
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
  %li3 = getelementptr i64, ptr %v2, i64 0
  %v3 = load i64, ptr %li3
  %li4 = getelementptr i64, ptr %v2, i64 1
  %v4 = load i64, ptr %li4
  %v5 = xor i64 %v3, 1
  %v6 = or i64 %v5, %v4
  %v7 = icmp eq i64 %v6, 0
  br i1 %v7, label %b1, label %b2
b1:
  %li9 = getelementptr i64, ptr %v2, i64 0
  %v9 = load i64, ptr %li9
  %li10 = getelementptr i64, ptr %v1, i64 0
  %v10 = load i64, ptr %li10
  store i64 %v10, ptr %gcs1
  %v11 = call i64 @dartforge_ffi_closure_address(i64 %v0)
  %v12 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v12 to i64
  %v13 = icmp ne i64 %c0, 0
  br i1 %v13, label %b4, label %b3
b2:
  store i64 0, ptr %gcs0
  call void @dartforge_nsm_call()
  %gcvolta2 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2, ptr %ctxtopo, align 8
  ret i64 0
b3:
  store i64 0, ptr %gcs0
  %v14 = call i64 @df.unbox_int(i64 %v10)
  %v15 = load i8, ptr %ctx, align 8
  %c1 = zext i8 %v15 to i64
  %v16 = icmp ne i64 %c1, 0
  br i1 %v16, label %b6, label %b5
b4:
  %gcvolta4 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta4, ptr %ctxtopo, align 8
  ret i64 0
b5:
  store i64 0, ptr %gcs1
  %fn17 = inttoptr i64 %v11 to ptr
  %nr17 = call i8 %fn17(i64 %v14)
  %v17 = sext i8 %nr17 to i64
  %v18 = call i64 @df.box_int(i64 %v17)
  %gcvolta5 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta5, ptr %ctxtopo, align 8
  ret i64 %v18
b6:
  %gcvolta6 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta6, ptr %ctxtopo, align 8
  ret i64 0
}

define i64 @df.ffi.a_l$cb(i64 %v0, i64 %v1) nounwind {
b0:
  %cargs4 = alloca [2 x i64]
  %gcq = alloca { ptr, i64, [2 x i64] }, align 8
  %pilhaq = alloca i8, align 1
  %ctx = call ptr @dartforge_context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load ptr, ptr %pilhalp, align 8
  %pilhab = icmp ult ptr %pilhaq, %pilhal
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
  %v2 = call i64 @dartforge_ffi_callback_closure(i64 %v0)
  store i64 %v2, ptr %gcs0
  %v3 = call i64 @df.box_int(i64 %v1)
  store i64 %v3, ptr %gcs1
  %ca4_0 = getelementptr [2 x i64], ptr %cargs4, i64 0, i64 0
  store i64 %v3, ptr %ca4_0
  %cat4 = getelementptr [2 x i64], ptr %cargs4, i64 0, i64 1
  store i64 0, ptr %cat4
  %cc4 = call i64 @dartforge_closure_entry(i64 %v2)
  %cz4 = icmp eq i64 %cc4, 0
  %cq4 = select i1 %cz4, i64 ptrtoint (ptr @df_clo_invalido to i64), i64 %cc4
  %cf4 = inttoptr i64 %cq4 to ptr
  %v4 = call i64 %cf4(i64 %v2, ptr %cargs4, ptr @df.arr.1)
  store i64 %v4, ptr %gcs0
  %v5 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v5 to i64
  %v6 = icmp ne i64 %c0, 0
  br i1 %v6, label %b2, label %b1
b1:
  store i64 0, ptr %gcs1
  %v7 = call i64 @df.unbox_int(i64 %v4)
  %v8 = load i8, ptr %ctx, align 8
  %c1 = zext i8 %v8 to i64
  %v9 = icmp ne i64 %c1, 0
  br i1 %v9, label %b4, label %b3
b2:
  %gcvolta2 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2, ptr %ctxtopo, align 8
  ret i64 0
b3:
  %gcvolta3 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta3, ptr %ctxtopo, align 8
  ret i64 %v7
b4:
  %gcvolta4 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta4, ptr %ctxtopo, align 8
  ret i64 0
}

define i64 @df.ffi.a_$ent(i64 %v0, ptr %v1, ptr %v2) nounwind {
b0:
  %gcq = alloca { ptr, i64, [1 x i64] }, align 8
  %pilhaq = alloca i8, align 1
  %ctx = call ptr @dartforge_context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load ptr, ptr %pilhalp, align 8
  %pilhab = icmp ult ptr %pilhaq, %pilhal
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
  %li3 = getelementptr i64, ptr %v2, i64 0
  %v3 = load i64, ptr %li3
  %li4 = getelementptr i64, ptr %v2, i64 1
  %v4 = load i64, ptr %li4
  %v5 = xor i64 %v3, 0
  %v6 = or i64 %v5, %v4
  %v7 = icmp eq i64 %v6, 0
  br i1 %v7, label %b1, label %b2
b1:
  %li9 = getelementptr i64, ptr %v2, i64 0
  %v9 = load i64, ptr %li9
  %v10 = call i64 @dartforge_ffi_closure_address(i64 %v0)
  %v11 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v11 to i64
  %v12 = icmp ne i64 %c0, 0
  br i1 %v12, label %b4, label %b3
b2:
  store i64 0, ptr %gcs0
  call void @dartforge_nsm_call()
  %gcvolta2 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2, ptr %ctxtopo, align 8
  ret i64 0
b3:
  store i64 0, ptr %gcs0
  %fn13 = inttoptr i64 %v10 to ptr
  %nr13 = call i8 %fn13()
  %v13 = sext i8 %nr13 to i64
  %v14 = call i64 @df.box_int(i64 %v13)
  %gcvolta3 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta3, ptr %ctxtopo, align 8
  ret i64 %v14
b4:
  %gcvolta4 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta4, ptr %ctxtopo, align 8
  ret i64 0
}

define i64 @df.ffi.a_$cb(i64 %v0) nounwind {
b0:
  %cargs2 = alloca [1 x i64]
  %gcq = alloca { ptr, i64, [1 x i64] }, align 8
  %pilhaq = alloca i8, align 1
  %ctx = call ptr @dartforge_context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load ptr, ptr %pilhalp, align 8
  %pilhab = icmp ult ptr %pilhaq, %pilhal
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
  %v1 = call i64 @dartforge_ffi_callback_closure(i64 %v0)
  store i64 %v1, ptr %gcs0
  %cat2 = getelementptr [1 x i64], ptr %cargs2, i64 0, i64 0
  store i64 0, ptr %cat2
  %cc2 = call i64 @dartforge_closure_entry(i64 %v1)
  %cz2 = icmp eq i64 %cc2, 0
  %cq2 = select i1 %cz2, i64 ptrtoint (ptr @df_clo_invalido to i64), i64 %cc2
  %cf2 = inttoptr i64 %cq2 to ptr
  %v2 = call i64 %cf2(i64 %v1, ptr %cargs2, ptr @df.arr.0)
  store i64 %v2, ptr %gcs0
  %v3 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v3 to i64
  %v4 = icmp ne i64 %c0, 0
  br i1 %v4, label %b2, label %b1
b1:
  %v5 = call i64 @df.unbox_int(i64 %v2)
  %v6 = load i8, ptr %ctx, align 8
  %c1 = zext i8 %v6 to i64
  %v7 = icmp ne i64 %c1, 0
  br i1 %v7, label %b4, label %b3
b2:
  %gcvolta2 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2, ptr %ctxtopo, align 8
  ret i64 0
b3:
  %gcvolta3 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta3, ptr %ctxtopo, align 8
  ret i64 %v5
b4:
  %gcvolta4 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta4, ptr %ctxtopo, align 8
  ret i64 0
}

define i64 @df.ffi.s_$ent(i64 %v0, ptr %v1, ptr %v2) nounwind {
b0:
  %gcq = alloca { ptr, i64, [1 x i64] }, align 8
  %pilhaq = alloca i8, align 1
  %ctx = call ptr @dartforge_context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load ptr, ptr %pilhalp, align 8
  %pilhab = icmp ult ptr %pilhaq, %pilhal
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
  %li3 = getelementptr i64, ptr %v2, i64 0
  %v3 = load i64, ptr %li3
  %li4 = getelementptr i64, ptr %v2, i64 1
  %v4 = load i64, ptr %li4
  %v5 = xor i64 %v3, 0
  %v6 = or i64 %v5, %v4
  %v7 = icmp eq i64 %v6, 0
  br i1 %v7, label %b1, label %b2
b1:
  %li9 = getelementptr i64, ptr %v2, i64 0
  %v9 = load i64, ptr %li9
  %v10 = call i64 @dartforge_ffi_closure_address(i64 %v0)
  %v11 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v11 to i64
  %v12 = icmp ne i64 %c0, 0
  br i1 %v12, label %b4, label %b3
b2:
  store i64 0, ptr %gcs0
  call void @dartforge_nsm_call()
  %gcvolta2 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2, ptr %ctxtopo, align 8
  ret i64 0
b3:
  store i64 0, ptr %gcs0
  %fn13 = inttoptr i64 %v10 to ptr
  %nr13 = call i16 %fn13()
  %v13 = sext i16 %nr13 to i64
  %v14 = call i64 @df.box_int(i64 %v13)
  %gcvolta3 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta3, ptr %ctxtopo, align 8
  ret i64 %v14
b4:
  %gcvolta4 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta4, ptr %ctxtopo, align 8
  ret i64 0
}

define i64 @df.ffi.s_$cb(i64 %v0) nounwind {
b0:
  %cargs2 = alloca [1 x i64]
  %gcq = alloca { ptr, i64, [1 x i64] }, align 8
  %pilhaq = alloca i8, align 1
  %ctx = call ptr @dartforge_context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load ptr, ptr %pilhalp, align 8
  %pilhab = icmp ult ptr %pilhaq, %pilhal
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
  %v1 = call i64 @dartforge_ffi_callback_closure(i64 %v0)
  store i64 %v1, ptr %gcs0
  %cat2 = getelementptr [1 x i64], ptr %cargs2, i64 0, i64 0
  store i64 0, ptr %cat2
  %cc2 = call i64 @dartforge_closure_entry(i64 %v1)
  %cz2 = icmp eq i64 %cc2, 0
  %cq2 = select i1 %cz2, i64 ptrtoint (ptr @df_clo_invalido to i64), i64 %cc2
  %cf2 = inttoptr i64 %cq2 to ptr
  %v2 = call i64 %cf2(i64 %v1, ptr %cargs2, ptr @df.arr.0)
  store i64 %v2, ptr %gcs0
  %v3 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v3 to i64
  %v4 = icmp ne i64 %c0, 0
  br i1 %v4, label %b2, label %b1
b1:
  %v5 = call i64 @df.unbox_int(i64 %v2)
  %v6 = load i8, ptr %ctx, align 8
  %c1 = zext i8 %v6 to i64
  %v7 = icmp ne i64 %c1, 0
  br i1 %v7, label %b4, label %b3
b2:
  %gcvolta2 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2, ptr %ctxtopo, align 8
  ret i64 0
b3:
  %gcvolta3 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta3, ptr %ctxtopo, align 8
  ret i64 %v5
b4:
  %gcvolta4 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta4, ptr %ctxtopo, align 8
  ret i64 0
}

define i64 @df.ffi.i_$ent(i64 %v0, ptr %v1, ptr %v2) nounwind {
b0:
  %gcq = alloca { ptr, i64, [1 x i64] }, align 8
  %pilhaq = alloca i8, align 1
  %ctx = call ptr @dartforge_context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load ptr, ptr %pilhalp, align 8
  %pilhab = icmp ult ptr %pilhaq, %pilhal
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
  %li3 = getelementptr i64, ptr %v2, i64 0
  %v3 = load i64, ptr %li3
  %li4 = getelementptr i64, ptr %v2, i64 1
  %v4 = load i64, ptr %li4
  %v5 = xor i64 %v3, 0
  %v6 = or i64 %v5, %v4
  %v7 = icmp eq i64 %v6, 0
  br i1 %v7, label %b1, label %b2
b1:
  %li9 = getelementptr i64, ptr %v2, i64 0
  %v9 = load i64, ptr %li9
  %v10 = call i64 @dartforge_ffi_closure_address(i64 %v0)
  %v11 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v11 to i64
  %v12 = icmp ne i64 %c0, 0
  br i1 %v12, label %b4, label %b3
b2:
  store i64 0, ptr %gcs0
  call void @dartforge_nsm_call()
  %gcvolta2 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2, ptr %ctxtopo, align 8
  ret i64 0
b3:
  store i64 0, ptr %gcs0
  %fn13 = inttoptr i64 %v10 to ptr
  %nr13 = call i32 %fn13()
  %v13 = sext i32 %nr13 to i64
  %v14 = call i64 @df.box_int(i64 %v13)
  %gcvolta3 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta3, ptr %ctxtopo, align 8
  ret i64 %v14
b4:
  %gcvolta4 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta4, ptr %ctxtopo, align 8
  ret i64 0
}

define i64 @df.ffi.i_$cb(i64 %v0) nounwind {
b0:
  %cargs2 = alloca [1 x i64]
  %gcq = alloca { ptr, i64, [1 x i64] }, align 8
  %pilhaq = alloca i8, align 1
  %ctx = call ptr @dartforge_context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load ptr, ptr %pilhalp, align 8
  %pilhab = icmp ult ptr %pilhaq, %pilhal
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
  %v1 = call i64 @dartforge_ffi_callback_closure(i64 %v0)
  store i64 %v1, ptr %gcs0
  %cat2 = getelementptr [1 x i64], ptr %cargs2, i64 0, i64 0
  store i64 0, ptr %cat2
  %cc2 = call i64 @dartforge_closure_entry(i64 %v1)
  %cz2 = icmp eq i64 %cc2, 0
  %cq2 = select i1 %cz2, i64 ptrtoint (ptr @df_clo_invalido to i64), i64 %cc2
  %cf2 = inttoptr i64 %cq2 to ptr
  %v2 = call i64 %cf2(i64 %v1, ptr %cargs2, ptr @df.arr.0)
  store i64 %v2, ptr %gcs0
  %v3 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v3 to i64
  %v4 = icmp ne i64 %c0, 0
  br i1 %v4, label %b2, label %b1
b1:
  %v5 = call i64 @df.unbox_int(i64 %v2)
  %v6 = load i8, ptr %ctx, align 8
  %c1 = zext i8 %v6 to i64
  %v7 = icmp ne i64 %c1, 0
  br i1 %v7, label %b4, label %b3
b2:
  %gcvolta2 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2, ptr %ctxtopo, align 8
  ret i64 0
b3:
  %gcvolta3 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta3, ptr %ctxtopo, align 8
  ret i64 %v5
b4:
  %gcvolta4 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta4, ptr %ctxtopo, align 8
  ret i64 0
}

define i64 @df.ffi.l_$ent(i64 %v0, ptr %v1, ptr %v2) nounwind {
b0:
  %gcq = alloca { ptr, i64, [1 x i64] }, align 8
  %pilhaq = alloca i8, align 1
  %ctx = call ptr @dartforge_context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load ptr, ptr %pilhalp, align 8
  %pilhab = icmp ult ptr %pilhaq, %pilhal
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
  %li3 = getelementptr i64, ptr %v2, i64 0
  %v3 = load i64, ptr %li3
  %li4 = getelementptr i64, ptr %v2, i64 1
  %v4 = load i64, ptr %li4
  %v5 = xor i64 %v3, 0
  %v6 = or i64 %v5, %v4
  %v7 = icmp eq i64 %v6, 0
  br i1 %v7, label %b1, label %b2
b1:
  %li9 = getelementptr i64, ptr %v2, i64 0
  %v9 = load i64, ptr %li9
  %v10 = call i64 @dartforge_ffi_closure_address(i64 %v0)
  %v11 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v11 to i64
  %v12 = icmp ne i64 %c0, 0
  br i1 %v12, label %b4, label %b3
b2:
  store i64 0, ptr %gcs0
  call void @dartforge_nsm_call()
  %gcvolta2 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2, ptr %ctxtopo, align 8
  ret i64 0
b3:
  store i64 0, ptr %gcs0
  %fn13 = inttoptr i64 %v10 to ptr
  %v13 = call i64 %fn13()
  %v14 = call i64 @df.box_int(i64 %v13)
  %gcvolta3 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta3, ptr %ctxtopo, align 8
  ret i64 %v14
b4:
  %gcvolta4 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta4, ptr %ctxtopo, align 8
  ret i64 0
}

define i64 @df.ffi.l_$cb(i64 %v0) nounwind {
b0:
  %cargs2 = alloca [1 x i64]
  %gcq = alloca { ptr, i64, [1 x i64] }, align 8
  %pilhaq = alloca i8, align 1
  %ctx = call ptr @dartforge_context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load ptr, ptr %pilhalp, align 8
  %pilhab = icmp ult ptr %pilhaq, %pilhal
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
  %v1 = call i64 @dartforge_ffi_callback_closure(i64 %v0)
  store i64 %v1, ptr %gcs0
  %cat2 = getelementptr [1 x i64], ptr %cargs2, i64 0, i64 0
  store i64 0, ptr %cat2
  %cc2 = call i64 @dartforge_closure_entry(i64 %v1)
  %cz2 = icmp eq i64 %cc2, 0
  %cq2 = select i1 %cz2, i64 ptrtoint (ptr @df_clo_invalido to i64), i64 %cc2
  %cf2 = inttoptr i64 %cq2 to ptr
  %v2 = call i64 %cf2(i64 %v1, ptr %cargs2, ptr @df.arr.0)
  store i64 %v2, ptr %gcs0
  %v3 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v3 to i64
  %v4 = icmp ne i64 %c0, 0
  br i1 %v4, label %b2, label %b1
b1:
  %v5 = call i64 @df.unbox_int(i64 %v2)
  %v6 = load i8, ptr %ctx, align 8
  %c1 = zext i8 %v6 to i64
  %v7 = icmp ne i64 %c1, 0
  br i1 %v7, label %b4, label %b3
b2:
  %gcvolta2 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2, ptr %ctxtopo, align 8
  ret i64 0
b3:
  %gcvolta3 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta3, ptr %ctxtopo, align 8
  ret i64 %v5
b4:
  %gcvolta4 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta4, ptr %ctxtopo, align 8
  ret i64 0
}

define i64 @df.ffi.h_$ent(i64 %v0, ptr %v1, ptr %v2) nounwind {
b0:
  %gcq = alloca { ptr, i64, [1 x i64] }, align 8
  %pilhaq = alloca i8, align 1
  %ctx = call ptr @dartforge_context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load ptr, ptr %pilhalp, align 8
  %pilhab = icmp ult ptr %pilhaq, %pilhal
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
  %li3 = getelementptr i64, ptr %v2, i64 0
  %v3 = load i64, ptr %li3
  %li4 = getelementptr i64, ptr %v2, i64 1
  %v4 = load i64, ptr %li4
  %v5 = xor i64 %v3, 0
  %v6 = or i64 %v5, %v4
  %v7 = icmp eq i64 %v6, 0
  br i1 %v7, label %b1, label %b2
b1:
  %li9 = getelementptr i64, ptr %v2, i64 0
  %v9 = load i64, ptr %li9
  %v10 = call i64 @dartforge_ffi_closure_address(i64 %v0)
  %v11 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v11 to i64
  %v12 = icmp ne i64 %c0, 0
  br i1 %v12, label %b4, label %b3
b2:
  store i64 0, ptr %gcs0
  call void @dartforge_nsm_call()
  %gcvolta2 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2, ptr %ctxtopo, align 8
  ret i64 0
b3:
  store i64 0, ptr %gcs0
  %fn13 = inttoptr i64 %v10 to ptr
  %nr13 = call i8 %fn13()
  %v13 = zext i8 %nr13 to i64
  %v14 = call i64 @df.box_int(i64 %v13)
  %gcvolta3 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta3, ptr %ctxtopo, align 8
  ret i64 %v14
b4:
  %gcvolta4 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta4, ptr %ctxtopo, align 8
  ret i64 0
}

define i64 @df.ffi.h_$cb(i64 %v0) nounwind {
b0:
  %cargs2 = alloca [1 x i64]
  %gcq = alloca { ptr, i64, [1 x i64] }, align 8
  %pilhaq = alloca i8, align 1
  %ctx = call ptr @dartforge_context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load ptr, ptr %pilhalp, align 8
  %pilhab = icmp ult ptr %pilhaq, %pilhal
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
  %v1 = call i64 @dartforge_ffi_callback_closure(i64 %v0)
  store i64 %v1, ptr %gcs0
  %cat2 = getelementptr [1 x i64], ptr %cargs2, i64 0, i64 0
  store i64 0, ptr %cat2
  %cc2 = call i64 @dartforge_closure_entry(i64 %v1)
  %cz2 = icmp eq i64 %cc2, 0
  %cq2 = select i1 %cz2, i64 ptrtoint (ptr @df_clo_invalido to i64), i64 %cc2
  %cf2 = inttoptr i64 %cq2 to ptr
  %v2 = call i64 %cf2(i64 %v1, ptr %cargs2, ptr @df.arr.0)
  store i64 %v2, ptr %gcs0
  %v3 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v3 to i64
  %v4 = icmp ne i64 %c0, 0
  br i1 %v4, label %b2, label %b1
b1:
  %v5 = call i64 @df.unbox_int(i64 %v2)
  %v6 = load i8, ptr %ctx, align 8
  %c1 = zext i8 %v6 to i64
  %v7 = icmp ne i64 %c1, 0
  br i1 %v7, label %b4, label %b3
b2:
  %gcvolta2 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2, ptr %ctxtopo, align 8
  ret i64 0
b3:
  %gcvolta3 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta3, ptr %ctxtopo, align 8
  ret i64 %v5
b4:
  %gcvolta4 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta4, ptr %ctxtopo, align 8
  ret i64 0
}

define i64 @df.ffi.t_$ent(i64 %v0, ptr %v1, ptr %v2) nounwind {
b0:
  %gcq = alloca { ptr, i64, [1 x i64] }, align 8
  %pilhaq = alloca i8, align 1
  %ctx = call ptr @dartforge_context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load ptr, ptr %pilhalp, align 8
  %pilhab = icmp ult ptr %pilhaq, %pilhal
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
  %li3 = getelementptr i64, ptr %v2, i64 0
  %v3 = load i64, ptr %li3
  %li4 = getelementptr i64, ptr %v2, i64 1
  %v4 = load i64, ptr %li4
  %v5 = xor i64 %v3, 0
  %v6 = or i64 %v5, %v4
  %v7 = icmp eq i64 %v6, 0
  br i1 %v7, label %b1, label %b2
b1:
  %li9 = getelementptr i64, ptr %v2, i64 0
  %v9 = load i64, ptr %li9
  %v10 = call i64 @dartforge_ffi_closure_address(i64 %v0)
  %v11 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v11 to i64
  %v12 = icmp ne i64 %c0, 0
  br i1 %v12, label %b4, label %b3
b2:
  store i64 0, ptr %gcs0
  call void @dartforge_nsm_call()
  %gcvolta2 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2, ptr %ctxtopo, align 8
  ret i64 0
b3:
  store i64 0, ptr %gcs0
  %fn13 = inttoptr i64 %v10 to ptr
  %nr13 = call i16 %fn13()
  %v13 = zext i16 %nr13 to i64
  %v14 = call i64 @df.box_int(i64 %v13)
  %gcvolta3 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta3, ptr %ctxtopo, align 8
  ret i64 %v14
b4:
  %gcvolta4 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta4, ptr %ctxtopo, align 8
  ret i64 0
}

define i64 @df.ffi.t_$cb(i64 %v0) nounwind {
b0:
  %cargs2 = alloca [1 x i64]
  %gcq = alloca { ptr, i64, [1 x i64] }, align 8
  %pilhaq = alloca i8, align 1
  %ctx = call ptr @dartforge_context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load ptr, ptr %pilhalp, align 8
  %pilhab = icmp ult ptr %pilhaq, %pilhal
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
  %v1 = call i64 @dartforge_ffi_callback_closure(i64 %v0)
  store i64 %v1, ptr %gcs0
  %cat2 = getelementptr [1 x i64], ptr %cargs2, i64 0, i64 0
  store i64 0, ptr %cat2
  %cc2 = call i64 @dartforge_closure_entry(i64 %v1)
  %cz2 = icmp eq i64 %cc2, 0
  %cq2 = select i1 %cz2, i64 ptrtoint (ptr @df_clo_invalido to i64), i64 %cc2
  %cf2 = inttoptr i64 %cq2 to ptr
  %v2 = call i64 %cf2(i64 %v1, ptr %cargs2, ptr @df.arr.0)
  store i64 %v2, ptr %gcs0
  %v3 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v3 to i64
  %v4 = icmp ne i64 %c0, 0
  br i1 %v4, label %b2, label %b1
b1:
  %v5 = call i64 @df.unbox_int(i64 %v2)
  %v6 = load i8, ptr %ctx, align 8
  %c1 = zext i8 %v6 to i64
  %v7 = icmp ne i64 %c1, 0
  br i1 %v7, label %b4, label %b3
b2:
  %gcvolta2 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2, ptr %ctxtopo, align 8
  ret i64 0
b3:
  %gcvolta3 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta3, ptr %ctxtopo, align 8
  ret i64 %v5
b4:
  %gcvolta4 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta4, ptr %ctxtopo, align 8
  ret i64 0
}

define i64 @df.ffi.j_$ent(i64 %v0, ptr %v1, ptr %v2) nounwind {
b0:
  %gcq = alloca { ptr, i64, [1 x i64] }, align 8
  %pilhaq = alloca i8, align 1
  %ctx = call ptr @dartforge_context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load ptr, ptr %pilhalp, align 8
  %pilhab = icmp ult ptr %pilhaq, %pilhal
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
  %li3 = getelementptr i64, ptr %v2, i64 0
  %v3 = load i64, ptr %li3
  %li4 = getelementptr i64, ptr %v2, i64 1
  %v4 = load i64, ptr %li4
  %v5 = xor i64 %v3, 0
  %v6 = or i64 %v5, %v4
  %v7 = icmp eq i64 %v6, 0
  br i1 %v7, label %b1, label %b2
b1:
  %li9 = getelementptr i64, ptr %v2, i64 0
  %v9 = load i64, ptr %li9
  %v10 = call i64 @dartforge_ffi_closure_address(i64 %v0)
  %v11 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v11 to i64
  %v12 = icmp ne i64 %c0, 0
  br i1 %v12, label %b4, label %b3
b2:
  store i64 0, ptr %gcs0
  call void @dartforge_nsm_call()
  %gcvolta2 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2, ptr %ctxtopo, align 8
  ret i64 0
b3:
  store i64 0, ptr %gcs0
  %fn13 = inttoptr i64 %v10 to ptr
  %nr13 = call i32 %fn13()
  %v13 = zext i32 %nr13 to i64
  %v14 = call i64 @df.box_int(i64 %v13)
  %gcvolta3 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta3, ptr %ctxtopo, align 8
  ret i64 %v14
b4:
  %gcvolta4 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta4, ptr %ctxtopo, align 8
  ret i64 0
}

define i64 @df.ffi.j_$cb(i64 %v0) nounwind {
b0:
  %cargs2 = alloca [1 x i64]
  %gcq = alloca { ptr, i64, [1 x i64] }, align 8
  %pilhaq = alloca i8, align 1
  %ctx = call ptr @dartforge_context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load ptr, ptr %pilhalp, align 8
  %pilhab = icmp ult ptr %pilhaq, %pilhal
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
  %v1 = call i64 @dartforge_ffi_callback_closure(i64 %v0)
  store i64 %v1, ptr %gcs0
  %cat2 = getelementptr [1 x i64], ptr %cargs2, i64 0, i64 0
  store i64 0, ptr %cat2
  %cc2 = call i64 @dartforge_closure_entry(i64 %v1)
  %cz2 = icmp eq i64 %cc2, 0
  %cq2 = select i1 %cz2, i64 ptrtoint (ptr @df_clo_invalido to i64), i64 %cc2
  %cf2 = inttoptr i64 %cq2 to ptr
  %v2 = call i64 %cf2(i64 %v1, ptr %cargs2, ptr @df.arr.0)
  store i64 %v2, ptr %gcs0
  %v3 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v3 to i64
  %v4 = icmp ne i64 %c0, 0
  br i1 %v4, label %b2, label %b1
b1:
  %v5 = call i64 @df.unbox_int(i64 %v2)
  %v6 = load i8, ptr %ctx, align 8
  %c1 = zext i8 %v6 to i64
  %v7 = icmp ne i64 %c1, 0
  br i1 %v7, label %b4, label %b3
b2:
  %gcvolta2 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2, ptr %ctxtopo, align 8
  ret i64 0
b3:
  %gcvolta3 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta3, ptr %ctxtopo, align 8
  ret i64 %v5
b4:
  %gcvolta4 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta4, ptr %ctxtopo, align 8
  ret i64 0
}

define i64 @df.ffi.m_$ent(i64 %v0, ptr %v1, ptr %v2) nounwind {
b0:
  %gcq = alloca { ptr, i64, [1 x i64] }, align 8
  %pilhaq = alloca i8, align 1
  %ctx = call ptr @dartforge_context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load ptr, ptr %pilhalp, align 8
  %pilhab = icmp ult ptr %pilhaq, %pilhal
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
  %li3 = getelementptr i64, ptr %v2, i64 0
  %v3 = load i64, ptr %li3
  %li4 = getelementptr i64, ptr %v2, i64 1
  %v4 = load i64, ptr %li4
  %v5 = xor i64 %v3, 0
  %v6 = or i64 %v5, %v4
  %v7 = icmp eq i64 %v6, 0
  br i1 %v7, label %b1, label %b2
b1:
  %li9 = getelementptr i64, ptr %v2, i64 0
  %v9 = load i64, ptr %li9
  %v10 = call i64 @dartforge_ffi_closure_address(i64 %v0)
  %v11 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v11 to i64
  %v12 = icmp ne i64 %c0, 0
  br i1 %v12, label %b4, label %b3
b2:
  store i64 0, ptr %gcs0
  call void @dartforge_nsm_call()
  %gcvolta2 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2, ptr %ctxtopo, align 8
  ret i64 0
b3:
  store i64 0, ptr %gcs0
  %fn13 = inttoptr i64 %v10 to ptr
  %v13 = call i64 %fn13()
  %v14 = call i64 @df.box_int(i64 %v13)
  %gcvolta3 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta3, ptr %ctxtopo, align 8
  ret i64 %v14
b4:
  %gcvolta4 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta4, ptr %ctxtopo, align 8
  ret i64 0
}

define i64 @df.ffi.m_$cb(i64 %v0) nounwind {
b0:
  %cargs2 = alloca [1 x i64]
  %gcq = alloca { ptr, i64, [1 x i64] }, align 8
  %pilhaq = alloca i8, align 1
  %ctx = call ptr @dartforge_context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load ptr, ptr %pilhalp, align 8
  %pilhab = icmp ult ptr %pilhaq, %pilhal
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
  %v1 = call i64 @dartforge_ffi_callback_closure(i64 %v0)
  store i64 %v1, ptr %gcs0
  %cat2 = getelementptr [1 x i64], ptr %cargs2, i64 0, i64 0
  store i64 0, ptr %cat2
  %cc2 = call i64 @dartforge_closure_entry(i64 %v1)
  %cz2 = icmp eq i64 %cc2, 0
  %cq2 = select i1 %cz2, i64 ptrtoint (ptr @df_clo_invalido to i64), i64 %cc2
  %cf2 = inttoptr i64 %cq2 to ptr
  %v2 = call i64 %cf2(i64 %v1, ptr %cargs2, ptr @df.arr.0)
  store i64 %v2, ptr %gcs0
  %v3 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v3 to i64
  %v4 = icmp ne i64 %c0, 0
  br i1 %v4, label %b2, label %b1
b1:
  %v5 = call i64 @df.unbox_int(i64 %v2)
  %v6 = load i8, ptr %ctx, align 8
  %c1 = zext i8 %v6 to i64
  %v7 = icmp ne i64 %c1, 0
  br i1 %v7, label %b4, label %b3
b2:
  %gcvolta2 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2, ptr %ctxtopo, align 8
  ret i64 0
b3:
  %gcvolta3 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta3, ptr %ctxtopo, align 8
  ret i64 %v5
b4:
  %gcvolta4 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta4, ptr %ctxtopo, align 8
  ret i64 0
}

define i64 @df.ffi.f_$ent(i64 %v0, ptr %v1, ptr %v2) nounwind {
b0:
  %gcq = alloca { ptr, i64, [1 x i64] }, align 8
  %pilhaq = alloca i8, align 1
  %ctx = call ptr @dartforge_context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load ptr, ptr %pilhalp, align 8
  %pilhab = icmp ult ptr %pilhaq, %pilhal
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
  %li3 = getelementptr i64, ptr %v2, i64 0
  %v3 = load i64, ptr %li3
  %li4 = getelementptr i64, ptr %v2, i64 1
  %v4 = load i64, ptr %li4
  %v5 = xor i64 %v3, 0
  %v6 = or i64 %v5, %v4
  %v7 = icmp eq i64 %v6, 0
  br i1 %v7, label %b1, label %b2
b1:
  %li9 = getelementptr i64, ptr %v2, i64 0
  %v9 = load i64, ptr %li9
  %v10 = call i64 @dartforge_ffi_closure_address(i64 %v0)
  %v11 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v11 to i64
  %v12 = icmp ne i64 %c0, 0
  br i1 %v12, label %b4, label %b3
b2:
  store i64 0, ptr %gcs0
  call void @dartforge_nsm_call()
  %gcvolta2 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2, ptr %ctxtopo, align 8
  ret i64 0
b3:
  store i64 0, ptr %gcs0
  %fn13 = inttoptr i64 %v10 to ptr
  %nr13 = call float %fn13()
  %v13 = fpext float %nr13 to double
  %v14 = call i64 @df.box_double(double %v13)
  %gcvolta3 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta3, ptr %ctxtopo, align 8
  ret i64 %v14
b4:
  %gcvolta4 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta4, ptr %ctxtopo, align 8
  ret i64 0
}

define double @df.ffi.f_$cb(i64 %v0) nounwind {
b0:
  %cargs2 = alloca [1 x i64]
  %gcq = alloca { ptr, i64, [1 x i64] }, align 8
  %pilhaq = alloca i8, align 1
  %ctx = call ptr @dartforge_context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load ptr, ptr %pilhalp, align 8
  %pilhab = icmp ult ptr %pilhaq, %pilhal
  %pilhax = call i1 @llvm.expect.i1(i1 %pilhab, i1 false)
  br i1 %pilhax, label %pilha.estouro, label %pilha.ok
pilha.estouro:
  call void @dartforge_stack_overflow()
  ret double 0.0
pilha.ok:
  store { ptr, i64, [1 x i64] } { ptr null, i64 4611686018427387905, [1 x i64] zeroinitializer }, ptr %gcq
  %gcs0 = getelementptr inbounds { ptr, i64, [1 x i64] }, ptr %gcq, i64 0, i32 2, i64 0
  %gcant = load ptr, ptr %ctxtopo, align 8
  store ptr %gcant, ptr %gcq, align 8
  store ptr %gcq, ptr %ctxtopo, align 8
  %v1 = call i64 @dartforge_ffi_callback_closure(i64 %v0)
  store i64 %v1, ptr %gcs0
  %cat2 = getelementptr [1 x i64], ptr %cargs2, i64 0, i64 0
  store i64 0, ptr %cat2
  %cc2 = call i64 @dartforge_closure_entry(i64 %v1)
  %cz2 = icmp eq i64 %cc2, 0
  %cq2 = select i1 %cz2, i64 ptrtoint (ptr @df_clo_invalido to i64), i64 %cc2
  %cf2 = inttoptr i64 %cq2 to ptr
  %v2 = call i64 %cf2(i64 %v1, ptr %cargs2, ptr @df.arr.0)
  store i64 %v2, ptr %gcs0
  %v3 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v3 to i64
  %v4 = icmp ne i64 %c0, 0
  br i1 %v4, label %b2, label %b1
b1:
  %v5 = call double @df.unbox_double(i64 %v2)
  %v6 = load i8, ptr %ctx, align 8
  %c1 = zext i8 %v6 to i64
  %v7 = icmp ne i64 %c1, 0
  br i1 %v7, label %b4, label %b3
b2:
  %gcvolta2 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2, ptr %ctxtopo, align 8
  ret double 0x0000000000000000
b3:
  %gcvolta3 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta3, ptr %ctxtopo, align 8
  ret double %v5
b4:
  %gcvolta4 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta4, ptr %ctxtopo, align 8
  ret double 0x0000000000000000
}

define i64 @df.ffi.d_$ent(i64 %v0, ptr %v1, ptr %v2) nounwind {
b0:
  %gcq = alloca { ptr, i64, [1 x i64] }, align 8
  %pilhaq = alloca i8, align 1
  %ctx = call ptr @dartforge_context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load ptr, ptr %pilhalp, align 8
  %pilhab = icmp ult ptr %pilhaq, %pilhal
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
  %li3 = getelementptr i64, ptr %v2, i64 0
  %v3 = load i64, ptr %li3
  %li4 = getelementptr i64, ptr %v2, i64 1
  %v4 = load i64, ptr %li4
  %v5 = xor i64 %v3, 0
  %v6 = or i64 %v5, %v4
  %v7 = icmp eq i64 %v6, 0
  br i1 %v7, label %b1, label %b2
b1:
  %li9 = getelementptr i64, ptr %v2, i64 0
  %v9 = load i64, ptr %li9
  %v10 = call i64 @dartforge_ffi_closure_address(i64 %v0)
  %v11 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v11 to i64
  %v12 = icmp ne i64 %c0, 0
  br i1 %v12, label %b4, label %b3
b2:
  store i64 0, ptr %gcs0
  call void @dartforge_nsm_call()
  %gcvolta2 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2, ptr %ctxtopo, align 8
  ret i64 0
b3:
  store i64 0, ptr %gcs0
  %fn13 = inttoptr i64 %v10 to ptr
  %v13 = call double %fn13()
  %v14 = call i64 @df.box_double(double %v13)
  %gcvolta3 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta3, ptr %ctxtopo, align 8
  ret i64 %v14
b4:
  %gcvolta4 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta4, ptr %ctxtopo, align 8
  ret i64 0
}

define double @df.ffi.d_$cb(i64 %v0) nounwind {
b0:
  %cargs2 = alloca [1 x i64]
  %gcq = alloca { ptr, i64, [1 x i64] }, align 8
  %pilhaq = alloca i8, align 1
  %ctx = call ptr @dartforge_context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load ptr, ptr %pilhalp, align 8
  %pilhab = icmp ult ptr %pilhaq, %pilhal
  %pilhax = call i1 @llvm.expect.i1(i1 %pilhab, i1 false)
  br i1 %pilhax, label %pilha.estouro, label %pilha.ok
pilha.estouro:
  call void @dartforge_stack_overflow()
  ret double 0.0
pilha.ok:
  store { ptr, i64, [1 x i64] } { ptr null, i64 4611686018427387905, [1 x i64] zeroinitializer }, ptr %gcq
  %gcs0 = getelementptr inbounds { ptr, i64, [1 x i64] }, ptr %gcq, i64 0, i32 2, i64 0
  %gcant = load ptr, ptr %ctxtopo, align 8
  store ptr %gcant, ptr %gcq, align 8
  store ptr %gcq, ptr %ctxtopo, align 8
  %v1 = call i64 @dartforge_ffi_callback_closure(i64 %v0)
  store i64 %v1, ptr %gcs0
  %cat2 = getelementptr [1 x i64], ptr %cargs2, i64 0, i64 0
  store i64 0, ptr %cat2
  %cc2 = call i64 @dartforge_closure_entry(i64 %v1)
  %cz2 = icmp eq i64 %cc2, 0
  %cq2 = select i1 %cz2, i64 ptrtoint (ptr @df_clo_invalido to i64), i64 %cc2
  %cf2 = inttoptr i64 %cq2 to ptr
  %v2 = call i64 %cf2(i64 %v1, ptr %cargs2, ptr @df.arr.0)
  store i64 %v2, ptr %gcs0
  %v3 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v3 to i64
  %v4 = icmp ne i64 %c0, 0
  br i1 %v4, label %b2, label %b1
b1:
  %v5 = call double @df.unbox_double(i64 %v2)
  %v6 = load i8, ptr %ctx, align 8
  %c1 = zext i8 %v6 to i64
  %v7 = icmp ne i64 %c1, 0
  br i1 %v7, label %b4, label %b3
b2:
  %gcvolta2 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2, ptr %ctxtopo, align 8
  ret double 0x0000000000000000
b3:
  %gcvolta3 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta3, ptr %ctxtopo, align 8
  ret double %v5
b4:
  %gcvolta4 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta4, ptr %ctxtopo, align 8
  ret double 0x0000000000000000
}

define i64 @df.ffi.b_$ent(i64 %v0, ptr %v1, ptr %v2) nounwind {
b0:
  %gcq = alloca { ptr, i64, [1 x i64] }, align 8
  %pilhaq = alloca i8, align 1
  %ctx = call ptr @dartforge_context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load ptr, ptr %pilhalp, align 8
  %pilhab = icmp ult ptr %pilhaq, %pilhal
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
  %li3 = getelementptr i64, ptr %v2, i64 0
  %v3 = load i64, ptr %li3
  %li4 = getelementptr i64, ptr %v2, i64 1
  %v4 = load i64, ptr %li4
  %v5 = xor i64 %v3, 0
  %v6 = or i64 %v5, %v4
  %v7 = icmp eq i64 %v6, 0
  br i1 %v7, label %b1, label %b2
b1:
  %li9 = getelementptr i64, ptr %v2, i64 0
  %v9 = load i64, ptr %li9
  %v10 = call i64 @dartforge_ffi_closure_address(i64 %v0)
  %v11 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v11 to i64
  %v12 = icmp ne i64 %c0, 0
  br i1 %v12, label %b4, label %b3
b2:
  store i64 0, ptr %gcs0
  call void @dartforge_nsm_call()
  %gcvolta2 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2, ptr %ctxtopo, align 8
  ret i64 0
b3:
  store i64 0, ptr %gcs0
  %fn13 = inttoptr i64 %v10 to ptr
  %v13 = call i1 %fn13()
  %v14 = call i64 @df.box_bool(i1 %v13)
  %gcvolta3 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta3, ptr %ctxtopo, align 8
  ret i64 %v14
b4:
  %gcvolta4 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta4, ptr %ctxtopo, align 8
  ret i64 0
}

define i1 @df.ffi.b_$cb(i64 %v0) nounwind {
b0:
  %cargs2 = alloca [1 x i64]
  %gcq = alloca { ptr, i64, [1 x i64] }, align 8
  %pilhaq = alloca i8, align 1
  %ctx = call ptr @dartforge_context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load ptr, ptr %pilhalp, align 8
  %pilhab = icmp ult ptr %pilhaq, %pilhal
  %pilhax = call i1 @llvm.expect.i1(i1 %pilhab, i1 false)
  br i1 %pilhax, label %pilha.estouro, label %pilha.ok
pilha.estouro:
  call void @dartforge_stack_overflow()
  ret i1 false
pilha.ok:
  store { ptr, i64, [1 x i64] } { ptr null, i64 4611686018427387905, [1 x i64] zeroinitializer }, ptr %gcq
  %gcs0 = getelementptr inbounds { ptr, i64, [1 x i64] }, ptr %gcq, i64 0, i32 2, i64 0
  %gcant = load ptr, ptr %ctxtopo, align 8
  store ptr %gcant, ptr %gcq, align 8
  store ptr %gcq, ptr %ctxtopo, align 8
  %v1 = call i64 @dartforge_ffi_callback_closure(i64 %v0)
  store i64 %v1, ptr %gcs0
  %cat2 = getelementptr [1 x i64], ptr %cargs2, i64 0, i64 0
  store i64 0, ptr %cat2
  %cc2 = call i64 @dartforge_closure_entry(i64 %v1)
  %cz2 = icmp eq i64 %cc2, 0
  %cq2 = select i1 %cz2, i64 ptrtoint (ptr @df_clo_invalido to i64), i64 %cc2
  %cf2 = inttoptr i64 %cq2 to ptr
  %v2 = call i64 %cf2(i64 %v1, ptr %cargs2, ptr @df.arr.0)
  store i64 %v2, ptr %gcs0
  %v3 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v3 to i64
  %v4 = icmp ne i64 %c0, 0
  br i1 %v4, label %b2, label %b1
b1:
  %v5 = call i1 @df.unbox_bool(i64 %v2)
  %v6 = load i8, ptr %ctx, align 8
  %c1 = zext i8 %v6 to i64
  %v7 = icmp ne i64 %c1, 0
  br i1 %v7, label %b4, label %b3
b2:
  %gcvolta2 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2, ptr %ctxtopo, align 8
  ret i1 false
b3:
  %gcvolta3 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta3, ptr %ctxtopo, align 8
  ret i1 %v5
b4:
  %gcvolta4 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta4, ptr %ctxtopo, align 8
  ret i1 false
}

define i64 @df.ffi.l_HHl$ent(i64 %v0, ptr %v1, ptr %v2) nounwind {
b0:
  %gcq = alloca { ptr, i64, [4 x i64] }, align 8
  %pilhaq = alloca i8, align 1
  %ctx = call ptr @dartforge_context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load ptr, ptr %pilhalp, align 8
  %pilhab = icmp ult ptr %pilhaq, %pilhal
  %pilhax = call i1 @llvm.expect.i1(i1 %pilhab, i1 false)
  br i1 %pilhax, label %pilha.estouro, label %pilha.ok
pilha.estouro:
  call void @dartforge_stack_overflow()
  ret i64 0
pilha.ok:
  store { ptr, i64, [4 x i64] } { ptr null, i64 4611686018427387908, [4 x i64] zeroinitializer }, ptr %gcq
  %gcs0 = getelementptr inbounds { ptr, i64, [4 x i64] }, ptr %gcq, i64 0, i32 2, i64 0
  %gcs1 = getelementptr inbounds { ptr, i64, [4 x i64] }, ptr %gcq, i64 0, i32 2, i64 1
  %gcs2 = getelementptr inbounds { ptr, i64, [4 x i64] }, ptr %gcq, i64 0, i32 2, i64 2
  %gcs3 = getelementptr inbounds { ptr, i64, [4 x i64] }, ptr %gcq, i64 0, i32 2, i64 3
  %gcant = load ptr, ptr %ctxtopo, align 8
  store ptr %gcant, ptr %gcq, align 8
  store ptr %gcq, ptr %ctxtopo, align 8
  store i64 %v0, ptr %gcs0
  %li3 = getelementptr i64, ptr %v2, i64 0
  %v3 = load i64, ptr %li3
  %li4 = getelementptr i64, ptr %v2, i64 1
  %v4 = load i64, ptr %li4
  %v5 = xor i64 %v3, 3
  %v6 = or i64 %v5, %v4
  %v7 = icmp eq i64 %v6, 0
  br i1 %v7, label %b1, label %b2
b1:
  %li9 = getelementptr i64, ptr %v2, i64 0
  %v9 = load i64, ptr %li9
  %li10 = getelementptr i64, ptr %v1, i64 0
  %v10 = load i64, ptr %li10
  store i64 %v10, ptr %gcs1
  %li11 = getelementptr i64, ptr %v1, i64 1
  %v11 = load i64, ptr %li11
  store i64 %v11, ptr %gcs2
  %li12 = getelementptr i64, ptr %v1, i64 2
  %v12 = load i64, ptr %li12
  store i64 %v12, ptr %gcs3
  %v13 = call i64 @dartforge_ffi_closure_address(i64 %v0)
  %v14 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v14 to i64
  %v15 = icmp ne i64 %c0, 0
  br i1 %v15, label %b4, label %b3
b2:
  store i64 0, ptr %gcs0
  call void @dartforge_nsm_call()
  %gcvolta2 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2, ptr %ctxtopo, align 8
  ret i64 0
b3:
  store i64 0, ptr %gcs0
  %v16 = call i64 @df.unbox_int(i64 %v12)
  %v17 = load i8, ptr %ctx, align 8
  %c1 = zext i8 %v17 to i64
  %v18 = icmp ne i64 %c1, 0
  br i1 %v18, label %b6, label %b5
b4:
  %gcvolta4 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta4, ptr %ctxtopo, align 8
  ret i64 0
b5:
  %v19 = call i64 @dartforge_ffi_handles_open()
  %v20 = call i64 @dartforge_ffi_handle_new(i64 %v10)
  %v21 = call i64 @dartforge_ffi_handle_new(i64 %v11)
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  store i64 0, ptr %gcs3
  %fn22 = inttoptr i64 %v13 to ptr
  %na22_0 = inttoptr i64 %v20 to ptr
  %na22_1 = inttoptr i64 %v21 to ptr
  %v22 = call i64 %fn22(ptr %na22_0, ptr %na22_1, i64 %v16)
  call void @dartforge_ffi_handles_close(i64 %v19)
  %v24 = call i64 @df.box_int(i64 %v22)
  %gcvolta5 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta5, ptr %ctxtopo, align 8
  ret i64 %v24
b6:
  %gcvolta6 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta6, ptr %ctxtopo, align 8
  ret i64 0
}

define i64 @df.ffi.l_HHl$cb(i64 %v0, i64 %v1, i64 %v2, i64 %v3) nounwind {
b0:
  %cargs12 = alloca [4 x i64]
  %gcq = alloca { ptr, i64, [4 x i64] }, align 8
  %pilhaq = alloca i8, align 1
  %ctx = call ptr @dartforge_context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load ptr, ptr %pilhalp, align 8
  %pilhab = icmp ult ptr %pilhaq, %pilhal
  %pilhax = call i1 @llvm.expect.i1(i1 %pilhab, i1 false)
  br i1 %pilhax, label %pilha.estouro, label %pilha.ok
pilha.estouro:
  call void @dartforge_stack_overflow()
  ret i64 0
pilha.ok:
  store { ptr, i64, [4 x i64] } { ptr null, i64 4611686018427387908, [4 x i64] zeroinitializer }, ptr %gcq
  %gcs0 = getelementptr inbounds { ptr, i64, [4 x i64] }, ptr %gcq, i64 0, i32 2, i64 0
  %gcs1 = getelementptr inbounds { ptr, i64, [4 x i64] }, ptr %gcq, i64 0, i32 2, i64 1
  %gcs2 = getelementptr inbounds { ptr, i64, [4 x i64] }, ptr %gcq, i64 0, i32 2, i64 2
  %gcs3 = getelementptr inbounds { ptr, i64, [4 x i64] }, ptr %gcq, i64 0, i32 2, i64 3
  %gcant = load ptr, ptr %ctxtopo, align 8
  store ptr %gcant, ptr %gcq, align 8
  store ptr %gcq, ptr %ctxtopo, align 8
  %v4 = call i64 @dartforge_ffi_callback_closure(i64 %v0)
  store i64 %v4, ptr %gcs0
  %v5 = call i64 @dartforge_ffi_handle_object(i64 %v1)
  store i64 %v5, ptr %gcs1
  %v6 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v6 to i64
  %v7 = icmp ne i64 %c0, 0
  br i1 %v7, label %b2, label %b1
b1:
  %v8 = call i64 @dartforge_ffi_handle_object(i64 %v2)
  store i64 %v8, ptr %gcs2
  %v9 = load i8, ptr %ctx, align 8
  %c1 = zext i8 %v9 to i64
  %v10 = icmp ne i64 %c1, 0
  br i1 %v10, label %b4, label %b3
b2:
  %gcvolta2 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2, ptr %ctxtopo, align 8
  ret i64 0
b3:
  %v11 = call i64 @df.box_int(i64 %v3)
  store i64 %v11, ptr %gcs3
  %ca12_0 = getelementptr [4 x i64], ptr %cargs12, i64 0, i64 0
  store i64 %v5, ptr %ca12_0
  %ca12_1 = getelementptr [4 x i64], ptr %cargs12, i64 0, i64 1
  store i64 %v8, ptr %ca12_1
  %ca12_2 = getelementptr [4 x i64], ptr %cargs12, i64 0, i64 2
  store i64 %v11, ptr %ca12_2
  %cat12 = getelementptr [4 x i64], ptr %cargs12, i64 0, i64 3
  store i64 0, ptr %cat12
  %cc12 = call i64 @dartforge_closure_entry(i64 %v4)
  %cz12 = icmp eq i64 %cc12, 0
  %cq12 = select i1 %cz12, i64 ptrtoint (ptr @df_clo_invalido to i64), i64 %cc12
  %cf12 = inttoptr i64 %cq12 to ptr
  %v12 = call i64 %cf12(i64 %v4, ptr %cargs12, ptr @df.arr.5)
  store i64 %v12, ptr %gcs0
  %v13 = load i8, ptr %ctx, align 8
  %c2 = zext i8 %v13 to i64
  %v14 = icmp ne i64 %c2, 0
  br i1 %v14, label %b6, label %b5
b4:
  %gcvolta4 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta4, ptr %ctxtopo, align 8
  ret i64 0
b5:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  store i64 0, ptr %gcs3
  %v15 = call i64 @df.unbox_int(i64 %v12)
  %v16 = load i8, ptr %ctx, align 8
  %c3 = zext i8 %v16 to i64
  %v17 = icmp ne i64 %c3, 0
  br i1 %v17, label %b8, label %b7
b6:
  %gcvolta6 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta6, ptr %ctxtopo, align 8
  ret i64 0
b7:
  %gcvolta7 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta7, ptr %ctxtopo, align 8
  ret i64 %v15
b8:
  %gcvolta8 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta8, ptr %ctxtopo, align 8
  ret i64 0
}

define i64 @dartforge_dispatch_toString(i64 %obj) nounwind {
b0:
  %a = alloca [1 x i64]
  store i64 0, ptr %a
  %area = call ptr @df.get_area()
  %ic = getelementptr i64, ptr %area, i64 24
  %f = call ptr @df.selector(ptr %ic, i64 %obj, i64 -3445551396850951156, ptr @df.seln.3, i64 10)
  %r = call i64 %f(i64 %obj, ptr %a, ptr @df.arr.0)
  ret i64 %r
}

@df.mt.$type$d = private unnamed_addr constant { i64, i64, i64, ptr, i64, ptr, i64, ptr, i64, ptr, i64, ptr, i64, ptr, i64, ptr, i64, ptr, i64, ptr, i64, ptr, i64, ptr, i64, ptr, i64, ptr, i64, ptr, i64, ptr, i64, ptr, i64, ptr, i64, ptr, i64, ptr, i64, ptr, i64, ptr, i64, ptr } { i64 1073741569, i64 22, i64 -9163797671095228218, ptr @df.$type.$3d$3d$c, i64 -3445551396850951156, ptr @df.$type.toString$c, i64 -1301464018035317077, ptr @df.dart$3acore.Object.noSuchMethod$tc, i64 -1174056763059767872, ptr @df.dart$3acore.Object._identityHashCode$c, i64 -86854735692782949, ptr @df.dart$3acore.Object._simpleInstanceOf$c, i64 358566821361314253, ptr @df.dart$3acore.Object.noSuchMethod$g, i64 1599743725080214626, ptr @df.dart$3acore.Object.$3d$3d$g, i64 1733328245926997691, ptr @df.dart$3acore.Object._simpleInstanceOfTrue$c, i64 1912180515176846064, ptr @df.dart$3acore.Object.toString$g, i64 2960095418482094780, ptr @df.dart$3acore.Object.$3d$3d$tc, i64 3023026857678717220, ptr @df.dart$3acore.Object.runtimeType$c, i64 3599887816169301665, ptr @df.dart$3acore.Object.noSuchMethod$c, i64 3904686890611376661, ptr @df.dart$3acore.Object._instanceOf$c, i64 4031450617872922849, ptr @df.dart$3acore.Object.hashCode$g, i64 4315172646098853535, ptr @df.dart$3acore.Object._simpleInstanceOf$g, i64 5306504273795186908, ptr @df.dart$3acore.Object._identityHashCode$g, i64 5334229838662542733, ptr @df.dart$3acore.Object.hashCode$c, i64 7093520068988870176, ptr @df.dart$3acore.Object.runtimeType$g, i64 7539980059721418247, ptr @df.dart$3acore.Object._simpleInstanceOfTrue$g, i64 7818775865403665178, ptr @df.dart$3acore.Object._simpleInstanceOfFalse$c, i64 8227416856165915390, ptr @df.dart$3acore.Object._simpleInstanceOfFalse$g, i64 8998535859395059057, ptr @df.dart$3acore.Object._instanceOf$g }
define ptr @df.mt.$type() nounwind {
b0:
  ret ptr @df.mt.$type$d
}
@df.argname.0 = private unnamed_addr constant [4 x i8] c"mode"
define void @df.register.program() nounwind {
b0:
  call void @dartforge_register_class_name(i64 0, ptr @.str.8, i64 6)
  call void @dartforge_register_class_name(i64 1073741568, ptr @.str.9, i64 11)
  call void @dartforge_register_class_name(i64 1073741569, ptr @.str.10, i64 5)
  call void @dartforge_register_subclass(i64 1073741569, i64 621)
  call void @dartforge_register_table(i64 1073741569, ptr @df.mt.$type)
  call void @dartforge_register_argument_name(ptr @df.argname.0, i64 4)
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
  call void @dartforge_rti_init()
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
  call void @dartforge_ffi_register_trampoline(ptr @df.ffi.key.0, i64 4, ptr @"df.ffi.v_lp$ent")
  call void @dartforge_ffi_register_trampoline(ptr @df.ffi.key.1, i64 3, ptr @"df.ffi.v_p$ent")
  call void @dartforge_ffi_register_trampoline(ptr @df.ffi.key.2, i64 2, ptr @"df.ffi.p_$ent")
  call void @dartforge_ffi_register_trampoline(ptr @df.ffi.key.3, i64 4, ptr @"df.ffi.a_lp$ent")
  call void @dartforge_ffi_register_trampoline(ptr @df.ffi.key.4, i64 5, ptr @"df.ffi.l_ppa$ent")
  call void @dartforge_ffi_register_trampoline(ptr @df.ffi.key.5, i64 3, ptr @"df.ffi.a_l$ent")
  call void @dartforge_ffi_register_trampoline(ptr @df.ffi.key.6, i64 2, ptr @"df.ffi.a_$ent")
  call void @dartforge_ffi_register_trampoline(ptr @df.ffi.key.7, i64 2, ptr @"df.ffi.s_$ent")
  call void @dartforge_ffi_register_trampoline(ptr @df.ffi.key.8, i64 2, ptr @"df.ffi.i_$ent")
  call void @dartforge_ffi_register_trampoline(ptr @df.ffi.key.9, i64 2, ptr @"df.ffi.l_$ent")
  call void @dartforge_ffi_register_trampoline(ptr @df.ffi.key.10, i64 2, ptr @"df.ffi.h_$ent")
  call void @dartforge_ffi_register_trampoline(ptr @df.ffi.key.11, i64 2, ptr @"df.ffi.t_$ent")
  call void @dartforge_ffi_register_trampoline(ptr @df.ffi.key.12, i64 2, ptr @"df.ffi.j_$ent")
  call void @dartforge_ffi_register_trampoline(ptr @df.ffi.key.13, i64 2, ptr @"df.ffi.m_$ent")
  call void @dartforge_ffi_register_trampoline(ptr @df.ffi.key.14, i64 2, ptr @"df.ffi.f_$ent")
  call void @dartforge_ffi_register_trampoline(ptr @df.ffi.key.15, i64 2, ptr @"df.ffi.d_$ent")
  call void @dartforge_ffi_register_trampoline(ptr @df.ffi.key.16, i64 2, ptr @"df.ffi.b_$ent")
  call void @dartforge_ffi_register_trampoline(ptr @df.ffi.key.17, i64 5, ptr @"df.ffi.l_HHl$ent")
  call void @dartforge_ffi_register_callback(ptr @df.ffi.cbkey.0, i64 4, ptr @df.ffi.cbentry.0)
  call void @dartforge_ffi_register_callback(ptr @df.ffi.cbkey.1, i64 3, ptr @df.ffi.cbentry.1)
  call void @dartforge_ffi_register_callback(ptr @df.ffi.cbkey.2, i64 2, ptr @df.ffi.cbentry.2)
  call void @dartforge_ffi_register_callback(ptr @df.ffi.cbkey.3, i64 4, ptr @df.ffi.cbentry.3)
  call void @dartforge_ffi_register_callback(ptr @df.ffi.cbkey.4, i64 5, ptr @df.ffi.cbentry.4)
  call void @dartforge_ffi_register_callback(ptr @df.ffi.cbkey.5, i64 3, ptr @df.ffi.cbentry.5)
  call void @dartforge_ffi_register_callback(ptr @df.ffi.cbkey.6, i64 2, ptr @df.ffi.cbentry.6)
  call void @dartforge_ffi_register_callback(ptr @df.ffi.cbkey.7, i64 2, ptr @df.ffi.cbentry.7)
  call void @dartforge_ffi_register_callback(ptr @df.ffi.cbkey.8, i64 2, ptr @df.ffi.cbentry.8)
  call void @dartforge_ffi_register_callback(ptr @df.ffi.cbkey.9, i64 2, ptr @df.ffi.cbentry.9)
  call void @dartforge_ffi_register_callback(ptr @df.ffi.cbkey.10, i64 2, ptr @df.ffi.cbentry.10)
  call void @dartforge_ffi_register_callback(ptr @df.ffi.cbkey.11, i64 2, ptr @df.ffi.cbentry.11)
  call void @dartforge_ffi_register_callback(ptr @df.ffi.cbkey.12, i64 2, ptr @df.ffi.cbentry.12)
  call void @dartforge_ffi_register_callback(ptr @df.ffi.cbkey.13, i64 2, ptr @df.ffi.cbentry.13)
  call void @dartforge_ffi_register_callback(ptr @df.ffi.cbkey.14, i64 2, ptr @df.ffi.cbentry.14)
  call void @dartforge_ffi_register_callback(ptr @df.ffi.cbkey.15, i64 2, ptr @df.ffi.cbentry.15)
  call void @dartforge_ffi_register_callback(ptr @df.ffi.cbkey.16, i64 2, ptr @df.ffi.cbentry.16)
  call void @dartforge_ffi_register_callback(ptr @df.ffi.cbkey.17, i64 5, ptr @df.ffi.cbentry.17)
  call void @dartforge_prepare_embedder()
  ret void
}

@df.ffi.key.0 = private unnamed_addr constant [4 x i8] c"v_lp"
@df.ffi.key.1 = private unnamed_addr constant [3 x i8] c"v_p"
@df.ffi.key.2 = private unnamed_addr constant [2 x i8] c"p_"
@df.ffi.key.3 = private unnamed_addr constant [4 x i8] c"a_lp"
@df.ffi.key.4 = private unnamed_addr constant [5 x i8] c"l_ppa"
@df.ffi.key.5 = private unnamed_addr constant [3 x i8] c"a_l"
@df.ffi.key.6 = private unnamed_addr constant [2 x i8] c"a_"
@df.ffi.key.7 = private unnamed_addr constant [2 x i8] c"s_"
@df.ffi.key.8 = private unnamed_addr constant [2 x i8] c"i_"
@df.ffi.key.9 = private unnamed_addr constant [2 x i8] c"l_"
@df.ffi.key.10 = private unnamed_addr constant [2 x i8] c"h_"
@df.ffi.key.11 = private unnamed_addr constant [2 x i8] c"t_"
@df.ffi.key.12 = private unnamed_addr constant [2 x i8] c"j_"
@df.ffi.key.13 = private unnamed_addr constant [2 x i8] c"m_"
@df.ffi.key.14 = private unnamed_addr constant [2 x i8] c"f_"
@df.ffi.key.15 = private unnamed_addr constant [2 x i8] c"d_"
@df.ffi.key.16 = private unnamed_addr constant [2 x i8] c"b_"
@df.ffi.key.17 = private unnamed_addr constant [5 x i8] c"l_HHl"
@df.ffi.cbkey.0 = private unnamed_addr constant [4 x i8] c"v_lp"
define internal void @df.ffi.cbentry.0(ptr nest %ctx, i64 %a0, ptr %a1) nounwind {
entrada:
  %modo = call i64 @dartforge_ffi_callback_enter(ptr %ctx)
  %is_closed_callback = icmp eq i64 %modo, 2
  br i1 %is_closed_callback, label %closed_callback, label %active_callback
closed_callback:
  ret void
active_callback:
  %saida = alloca i64
  %buf = alloca [2 x i64]
  %e_ouvinte = icmp ne i64 %modo, 0
  br i1 %e_ouvinte, label %ouvinte, label %local
ouvinte:
  %o0 = add i64 %a0, 0
  %g0 = getelementptr [2 x i64], ptr %buf, i64 0, i64 0
  store i64 %o0, ptr %g0
  %o1 = ptrtoint ptr %a1 to i64
  %g1 = getelementptr [2 x i64], ptr %buf, i64 0, i64 1
  store i64 %o1, ptr %g1
  %is_sync_callback = icmp eq i64 %modo, 3
  br i1 %is_sync_callback, label %foreign_sync, label %listener_post
foreign_sync:
  %sync_bits = call i64 @dartforge_ffi_callback_sync(ptr %ctx, ptr %buf, i64 2)
  ret void
listener_post:
  call void @dartforge_ffi_callback_post(ptr %ctx, ptr %buf, i64 2)
  ret void
local:
  %c = ptrtoint ptr %ctx to i64
  %d1 = ptrtoint ptr %a1 to i64
  call void @df.ffi.v_lp$cb(i64 %c, i64 %a0, i64 %d1)
  %x = call i8 @dartforge_ffi_callback_exit(ptr %ctx, ptr %saida)
  ret void
}

@df.ffi.cbkey.1 = private unnamed_addr constant [3 x i8] c"v_p"
define internal void @df.ffi.cbentry.1(ptr nest %ctx, ptr %a0) nounwind {
entrada:
  %modo = call i64 @dartforge_ffi_callback_enter(ptr %ctx)
  %is_closed_callback = icmp eq i64 %modo, 2
  br i1 %is_closed_callback, label %closed_callback, label %active_callback
closed_callback:
  ret void
active_callback:
  %saida = alloca i64
  %buf = alloca [1 x i64]
  %e_ouvinte = icmp ne i64 %modo, 0
  br i1 %e_ouvinte, label %ouvinte, label %local
ouvinte:
  %o0 = ptrtoint ptr %a0 to i64
  %g0 = getelementptr [1 x i64], ptr %buf, i64 0, i64 0
  store i64 %o0, ptr %g0
  %is_sync_callback = icmp eq i64 %modo, 3
  br i1 %is_sync_callback, label %foreign_sync, label %listener_post
foreign_sync:
  %sync_bits = call i64 @dartforge_ffi_callback_sync(ptr %ctx, ptr %buf, i64 1)
  ret void
listener_post:
  call void @dartforge_ffi_callback_post(ptr %ctx, ptr %buf, i64 1)
  ret void
local:
  %c = ptrtoint ptr %ctx to i64
  %d0 = ptrtoint ptr %a0 to i64
  call void @df.ffi.v_p$cb(i64 %c, i64 %d0)
  %x = call i8 @dartforge_ffi_callback_exit(ptr %ctx, ptr %saida)
  ret void
}

@df.ffi.cbkey.2 = private unnamed_addr constant [2 x i8] c"p_"
define internal ptr @df.ffi.cbentry.2(ptr nest %ctx) nounwind {
entrada:
  %modo = call i64 @dartforge_ffi_callback_enter(ptr %ctx)
  %is_closed_callback = icmp eq i64 %modo, 2
  br i1 %is_closed_callback, label %closed_callback, label %active_callback
closed_callback:
  %late_bits = call i64 @dartforge_ffi_callback_late_return(ptr %ctx)
  %late_value = inttoptr i64 %late_bits to ptr
  ret ptr %late_value
active_callback:
  %saida = alloca i64
  %buf = alloca [1 x i64]
  %e_ouvinte = icmp ne i64 %modo, 0
  br i1 %e_ouvinte, label %ouvinte, label %local
ouvinte:
  %is_sync_callback = icmp eq i64 %modo, 3
  br i1 %is_sync_callback, label %foreign_sync, label %listener_post
foreign_sync:
  %sync_bits = call i64 @dartforge_ffi_callback_sync(ptr %ctx, ptr %buf, i64 0)
  %sync_value = inttoptr i64 %sync_bits to ptr
  ret ptr %sync_value
listener_post:
  call void @dartforge_ffi_callback_post(ptr %ctx, ptr %buf, i64 0)
  ret ptr null
local:
  %c = ptrtoint ptr %ctx to i64
  %r = call i64 @df.ffi.p_$cb(i64 %c)
  %x = call i8 @dartforge_ffi_callback_exit(ptr %ctx, ptr %saida)
  %e_excecao = icmp ne i8 %x, 0
  br i1 %e_excecao, label %excecao, label %normal
excecao:
  %e = load i64, ptr %saida
  %ev = inttoptr i64 %e to ptr
  ret ptr %ev
normal:
  %nv = inttoptr i64 %r to ptr
  ret ptr %nv
}

@df.ffi.cbkey.3 = private unnamed_addr constant [4 x i8] c"a_lp"
define internal signext i8 @df.ffi.cbentry.3(ptr nest %ctx, i64 %a0, ptr %a1) nounwind {
entrada:
  %modo = call i64 @dartforge_ffi_callback_enter(ptr %ctx)
  %is_closed_callback = icmp eq i64 %modo, 2
  br i1 %is_closed_callback, label %closed_callback, label %active_callback
closed_callback:
  %late_bits = call i64 @dartforge_ffi_callback_late_return(ptr %ctx)
  %late_value = trunc i64 %late_bits to i8
  ret i8 %late_value
active_callback:
  %saida = alloca i64
  %buf = alloca [2 x i64]
  %e_ouvinte = icmp ne i64 %modo, 0
  br i1 %e_ouvinte, label %ouvinte, label %local
ouvinte:
  %o0 = add i64 %a0, 0
  %g0 = getelementptr [2 x i64], ptr %buf, i64 0, i64 0
  store i64 %o0, ptr %g0
  %o1 = ptrtoint ptr %a1 to i64
  %g1 = getelementptr [2 x i64], ptr %buf, i64 0, i64 1
  store i64 %o1, ptr %g1
  %is_sync_callback = icmp eq i64 %modo, 3
  br i1 %is_sync_callback, label %foreign_sync, label %listener_post
foreign_sync:
  %sync_bits = call i64 @dartforge_ffi_callback_sync(ptr %ctx, ptr %buf, i64 2)
  %sync_value = trunc i64 %sync_bits to i8
  ret i8 %sync_value
listener_post:
  call void @dartforge_ffi_callback_post(ptr %ctx, ptr %buf, i64 2)
  ret i8 0
local:
  %c = ptrtoint ptr %ctx to i64
  %d1 = ptrtoint ptr %a1 to i64
  %r = call i64 @df.ffi.a_lp$cb(i64 %c, i64 %a0, i64 %d1)
  %x = call i8 @dartforge_ffi_callback_exit(ptr %ctx, ptr %saida)
  %e_excecao = icmp ne i8 %x, 0
  br i1 %e_excecao, label %excecao, label %normal
excecao:
  %e = load i64, ptr %saida
  %ev = trunc i64 %e to i8
  ret i8 %ev
normal:
  %nv = trunc i64 %r to i8
  ret i8 %nv
}

@df.ffi.cbkey.4 = private unnamed_addr constant [5 x i8] c"l_ppa"
define internal i64 @df.ffi.cbentry.4(ptr nest %ctx, ptr %a0, ptr %a1, i8 signext %a2) nounwind {
entrada:
  %modo = call i64 @dartforge_ffi_callback_enter(ptr %ctx)
  %is_closed_callback = icmp eq i64 %modo, 2
  br i1 %is_closed_callback, label %closed_callback, label %active_callback
closed_callback:
  %late_bits = call i64 @dartforge_ffi_callback_late_return(ptr %ctx)
  %late_value = add i64 %late_bits, 0
  ret i64 %late_value
active_callback:
  %saida = alloca i64
  %buf = alloca [3 x i64]
  %e_ouvinte = icmp ne i64 %modo, 0
  br i1 %e_ouvinte, label %ouvinte, label %local
ouvinte:
  %o0 = ptrtoint ptr %a0 to i64
  %g0 = getelementptr [3 x i64], ptr %buf, i64 0, i64 0
  store i64 %o0, ptr %g0
  %o1 = ptrtoint ptr %a1 to i64
  %g1 = getelementptr [3 x i64], ptr %buf, i64 0, i64 1
  store i64 %o1, ptr %g1
  %o2 = sext i8 %a2 to i64
  %g2 = getelementptr [3 x i64], ptr %buf, i64 0, i64 2
  store i64 %o2, ptr %g2
  %is_sync_callback = icmp eq i64 %modo, 3
  br i1 %is_sync_callback, label %foreign_sync, label %listener_post
foreign_sync:
  %sync_bits = call i64 @dartforge_ffi_callback_sync(ptr %ctx, ptr %buf, i64 3)
  %sync_value = add i64 %sync_bits, 0
  ret i64 %sync_value
listener_post:
  call void @dartforge_ffi_callback_post(ptr %ctx, ptr %buf, i64 3)
  ret i64 0
local:
  %c = ptrtoint ptr %ctx to i64
  %d0 = ptrtoint ptr %a0 to i64
  %d1 = ptrtoint ptr %a1 to i64
  %d2 = sext i8 %a2 to i64
  %r = call i64 @df.ffi.l_ppa$cb(i64 %c, i64 %d0, i64 %d1, i64 %d2)
  %x = call i8 @dartforge_ffi_callback_exit(ptr %ctx, ptr %saida)
  %e_excecao = icmp ne i8 %x, 0
  br i1 %e_excecao, label %excecao, label %normal
excecao:
  %e = load i64, ptr %saida
  %ev = add i64 %e, 0
  ret i64 %ev
normal:
  ret i64 %r
}

@df.ffi.cbkey.5 = private unnamed_addr constant [3 x i8] c"a_l"
define internal signext i8 @df.ffi.cbentry.5(ptr nest %ctx, i64 %a0) nounwind {
entrada:
  %modo = call i64 @dartforge_ffi_callback_enter(ptr %ctx)
  %is_closed_callback = icmp eq i64 %modo, 2
  br i1 %is_closed_callback, label %closed_callback, label %active_callback
closed_callback:
  %late_bits = call i64 @dartforge_ffi_callback_late_return(ptr %ctx)
  %late_value = trunc i64 %late_bits to i8
  ret i8 %late_value
active_callback:
  %saida = alloca i64
  %buf = alloca [1 x i64]
  %e_ouvinte = icmp ne i64 %modo, 0
  br i1 %e_ouvinte, label %ouvinte, label %local
ouvinte:
  %o0 = add i64 %a0, 0
  %g0 = getelementptr [1 x i64], ptr %buf, i64 0, i64 0
  store i64 %o0, ptr %g0
  %is_sync_callback = icmp eq i64 %modo, 3
  br i1 %is_sync_callback, label %foreign_sync, label %listener_post
foreign_sync:
  %sync_bits = call i64 @dartforge_ffi_callback_sync(ptr %ctx, ptr %buf, i64 1)
  %sync_value = trunc i64 %sync_bits to i8
  ret i8 %sync_value
listener_post:
  call void @dartforge_ffi_callback_post(ptr %ctx, ptr %buf, i64 1)
  ret i8 0
local:
  %c = ptrtoint ptr %ctx to i64
  %r = call i64 @df.ffi.a_l$cb(i64 %c, i64 %a0)
  %x = call i8 @dartforge_ffi_callback_exit(ptr %ctx, ptr %saida)
  %e_excecao = icmp ne i8 %x, 0
  br i1 %e_excecao, label %excecao, label %normal
excecao:
  %e = load i64, ptr %saida
  %ev = trunc i64 %e to i8
  ret i8 %ev
normal:
  %nv = trunc i64 %r to i8
  ret i8 %nv
}

@df.ffi.cbkey.6 = private unnamed_addr constant [2 x i8] c"a_"
define internal signext i8 @df.ffi.cbentry.6(ptr nest %ctx) nounwind {
entrada:
  %modo = call i64 @dartforge_ffi_callback_enter(ptr %ctx)
  %is_closed_callback = icmp eq i64 %modo, 2
  br i1 %is_closed_callback, label %closed_callback, label %active_callback
closed_callback:
  %late_bits = call i64 @dartforge_ffi_callback_late_return(ptr %ctx)
  %late_value = trunc i64 %late_bits to i8
  ret i8 %late_value
active_callback:
  %saida = alloca i64
  %buf = alloca [1 x i64]
  %e_ouvinte = icmp ne i64 %modo, 0
  br i1 %e_ouvinte, label %ouvinte, label %local
ouvinte:
  %is_sync_callback = icmp eq i64 %modo, 3
  br i1 %is_sync_callback, label %foreign_sync, label %listener_post
foreign_sync:
  %sync_bits = call i64 @dartforge_ffi_callback_sync(ptr %ctx, ptr %buf, i64 0)
  %sync_value = trunc i64 %sync_bits to i8
  ret i8 %sync_value
listener_post:
  call void @dartforge_ffi_callback_post(ptr %ctx, ptr %buf, i64 0)
  ret i8 0
local:
  %c = ptrtoint ptr %ctx to i64
  %r = call i64 @df.ffi.a_$cb(i64 %c)
  %x = call i8 @dartforge_ffi_callback_exit(ptr %ctx, ptr %saida)
  %e_excecao = icmp ne i8 %x, 0
  br i1 %e_excecao, label %excecao, label %normal
excecao:
  %e = load i64, ptr %saida
  %ev = trunc i64 %e to i8
  ret i8 %ev
normal:
  %nv = trunc i64 %r to i8
  ret i8 %nv
}

@df.ffi.cbkey.7 = private unnamed_addr constant [2 x i8] c"s_"
define internal signext i16 @df.ffi.cbentry.7(ptr nest %ctx) nounwind {
entrada:
  %modo = call i64 @dartforge_ffi_callback_enter(ptr %ctx)
  %is_closed_callback = icmp eq i64 %modo, 2
  br i1 %is_closed_callback, label %closed_callback, label %active_callback
closed_callback:
  %late_bits = call i64 @dartforge_ffi_callback_late_return(ptr %ctx)
  %late_value = trunc i64 %late_bits to i16
  ret i16 %late_value
active_callback:
  %saida = alloca i64
  %buf = alloca [1 x i64]
  %e_ouvinte = icmp ne i64 %modo, 0
  br i1 %e_ouvinte, label %ouvinte, label %local
ouvinte:
  %is_sync_callback = icmp eq i64 %modo, 3
  br i1 %is_sync_callback, label %foreign_sync, label %listener_post
foreign_sync:
  %sync_bits = call i64 @dartforge_ffi_callback_sync(ptr %ctx, ptr %buf, i64 0)
  %sync_value = trunc i64 %sync_bits to i16
  ret i16 %sync_value
listener_post:
  call void @dartforge_ffi_callback_post(ptr %ctx, ptr %buf, i64 0)
  ret i16 0
local:
  %c = ptrtoint ptr %ctx to i64
  %r = call i64 @df.ffi.s_$cb(i64 %c)
  %x = call i8 @dartforge_ffi_callback_exit(ptr %ctx, ptr %saida)
  %e_excecao = icmp ne i8 %x, 0
  br i1 %e_excecao, label %excecao, label %normal
excecao:
  %e = load i64, ptr %saida
  %ev = trunc i64 %e to i16
  ret i16 %ev
normal:
  %nv = trunc i64 %r to i16
  ret i16 %nv
}

@df.ffi.cbkey.8 = private unnamed_addr constant [2 x i8] c"i_"
define internal i32 @df.ffi.cbentry.8(ptr nest %ctx) nounwind {
entrada:
  %modo = call i64 @dartforge_ffi_callback_enter(ptr %ctx)
  %is_closed_callback = icmp eq i64 %modo, 2
  br i1 %is_closed_callback, label %closed_callback, label %active_callback
closed_callback:
  %late_bits = call i64 @dartforge_ffi_callback_late_return(ptr %ctx)
  %late_value = trunc i64 %late_bits to i32
  ret i32 %late_value
active_callback:
  %saida = alloca i64
  %buf = alloca [1 x i64]
  %e_ouvinte = icmp ne i64 %modo, 0
  br i1 %e_ouvinte, label %ouvinte, label %local
ouvinte:
  %is_sync_callback = icmp eq i64 %modo, 3
  br i1 %is_sync_callback, label %foreign_sync, label %listener_post
foreign_sync:
  %sync_bits = call i64 @dartforge_ffi_callback_sync(ptr %ctx, ptr %buf, i64 0)
  %sync_value = trunc i64 %sync_bits to i32
  ret i32 %sync_value
listener_post:
  call void @dartforge_ffi_callback_post(ptr %ctx, ptr %buf, i64 0)
  ret i32 0
local:
  %c = ptrtoint ptr %ctx to i64
  %r = call i64 @df.ffi.i_$cb(i64 %c)
  %x = call i8 @dartforge_ffi_callback_exit(ptr %ctx, ptr %saida)
  %e_excecao = icmp ne i8 %x, 0
  br i1 %e_excecao, label %excecao, label %normal
excecao:
  %e = load i64, ptr %saida
  %ev = trunc i64 %e to i32
  ret i32 %ev
normal:
  %nv = trunc i64 %r to i32
  ret i32 %nv
}

@df.ffi.cbkey.9 = private unnamed_addr constant [2 x i8] c"l_"
define internal i64 @df.ffi.cbentry.9(ptr nest %ctx) nounwind {
entrada:
  %modo = call i64 @dartforge_ffi_callback_enter(ptr %ctx)
  %is_closed_callback = icmp eq i64 %modo, 2
  br i1 %is_closed_callback, label %closed_callback, label %active_callback
closed_callback:
  %late_bits = call i64 @dartforge_ffi_callback_late_return(ptr %ctx)
  %late_value = add i64 %late_bits, 0
  ret i64 %late_value
active_callback:
  %saida = alloca i64
  %buf = alloca [1 x i64]
  %e_ouvinte = icmp ne i64 %modo, 0
  br i1 %e_ouvinte, label %ouvinte, label %local
ouvinte:
  %is_sync_callback = icmp eq i64 %modo, 3
  br i1 %is_sync_callback, label %foreign_sync, label %listener_post
foreign_sync:
  %sync_bits = call i64 @dartforge_ffi_callback_sync(ptr %ctx, ptr %buf, i64 0)
  %sync_value = add i64 %sync_bits, 0
  ret i64 %sync_value
listener_post:
  call void @dartforge_ffi_callback_post(ptr %ctx, ptr %buf, i64 0)
  ret i64 0
local:
  %c = ptrtoint ptr %ctx to i64
  %r = call i64 @df.ffi.l_$cb(i64 %c)
  %x = call i8 @dartforge_ffi_callback_exit(ptr %ctx, ptr %saida)
  %e_excecao = icmp ne i8 %x, 0
  br i1 %e_excecao, label %excecao, label %normal
excecao:
  %e = load i64, ptr %saida
  %ev = add i64 %e, 0
  ret i64 %ev
normal:
  ret i64 %r
}

@df.ffi.cbkey.10 = private unnamed_addr constant [2 x i8] c"h_"
define internal zeroext i8 @df.ffi.cbentry.10(ptr nest %ctx) nounwind {
entrada:
  %modo = call i64 @dartforge_ffi_callback_enter(ptr %ctx)
  %is_closed_callback = icmp eq i64 %modo, 2
  br i1 %is_closed_callback, label %closed_callback, label %active_callback
closed_callback:
  %late_bits = call i64 @dartforge_ffi_callback_late_return(ptr %ctx)
  %late_value = trunc i64 %late_bits to i8
  ret i8 %late_value
active_callback:
  %saida = alloca i64
  %buf = alloca [1 x i64]
  %e_ouvinte = icmp ne i64 %modo, 0
  br i1 %e_ouvinte, label %ouvinte, label %local
ouvinte:
  %is_sync_callback = icmp eq i64 %modo, 3
  br i1 %is_sync_callback, label %foreign_sync, label %listener_post
foreign_sync:
  %sync_bits = call i64 @dartforge_ffi_callback_sync(ptr %ctx, ptr %buf, i64 0)
  %sync_value = trunc i64 %sync_bits to i8
  ret i8 %sync_value
listener_post:
  call void @dartforge_ffi_callback_post(ptr %ctx, ptr %buf, i64 0)
  ret i8 0
local:
  %c = ptrtoint ptr %ctx to i64
  %r = call i64 @df.ffi.h_$cb(i64 %c)
  %x = call i8 @dartforge_ffi_callback_exit(ptr %ctx, ptr %saida)
  %e_excecao = icmp ne i8 %x, 0
  br i1 %e_excecao, label %excecao, label %normal
excecao:
  %e = load i64, ptr %saida
  %ev = trunc i64 %e to i8
  ret i8 %ev
normal:
  %nv = trunc i64 %r to i8
  ret i8 %nv
}

@df.ffi.cbkey.11 = private unnamed_addr constant [2 x i8] c"t_"
define internal zeroext i16 @df.ffi.cbentry.11(ptr nest %ctx) nounwind {
entrada:
  %modo = call i64 @dartforge_ffi_callback_enter(ptr %ctx)
  %is_closed_callback = icmp eq i64 %modo, 2
  br i1 %is_closed_callback, label %closed_callback, label %active_callback
closed_callback:
  %late_bits = call i64 @dartforge_ffi_callback_late_return(ptr %ctx)
  %late_value = trunc i64 %late_bits to i16
  ret i16 %late_value
active_callback:
  %saida = alloca i64
  %buf = alloca [1 x i64]
  %e_ouvinte = icmp ne i64 %modo, 0
  br i1 %e_ouvinte, label %ouvinte, label %local
ouvinte:
  %is_sync_callback = icmp eq i64 %modo, 3
  br i1 %is_sync_callback, label %foreign_sync, label %listener_post
foreign_sync:
  %sync_bits = call i64 @dartforge_ffi_callback_sync(ptr %ctx, ptr %buf, i64 0)
  %sync_value = trunc i64 %sync_bits to i16
  ret i16 %sync_value
listener_post:
  call void @dartforge_ffi_callback_post(ptr %ctx, ptr %buf, i64 0)
  ret i16 0
local:
  %c = ptrtoint ptr %ctx to i64
  %r = call i64 @df.ffi.t_$cb(i64 %c)
  %x = call i8 @dartforge_ffi_callback_exit(ptr %ctx, ptr %saida)
  %e_excecao = icmp ne i8 %x, 0
  br i1 %e_excecao, label %excecao, label %normal
excecao:
  %e = load i64, ptr %saida
  %ev = trunc i64 %e to i16
  ret i16 %ev
normal:
  %nv = trunc i64 %r to i16
  ret i16 %nv
}

@df.ffi.cbkey.12 = private unnamed_addr constant [2 x i8] c"j_"
define internal i32 @df.ffi.cbentry.12(ptr nest %ctx) nounwind {
entrada:
  %modo = call i64 @dartforge_ffi_callback_enter(ptr %ctx)
  %is_closed_callback = icmp eq i64 %modo, 2
  br i1 %is_closed_callback, label %closed_callback, label %active_callback
closed_callback:
  %late_bits = call i64 @dartforge_ffi_callback_late_return(ptr %ctx)
  %late_value = trunc i64 %late_bits to i32
  ret i32 %late_value
active_callback:
  %saida = alloca i64
  %buf = alloca [1 x i64]
  %e_ouvinte = icmp ne i64 %modo, 0
  br i1 %e_ouvinte, label %ouvinte, label %local
ouvinte:
  %is_sync_callback = icmp eq i64 %modo, 3
  br i1 %is_sync_callback, label %foreign_sync, label %listener_post
foreign_sync:
  %sync_bits = call i64 @dartforge_ffi_callback_sync(ptr %ctx, ptr %buf, i64 0)
  %sync_value = trunc i64 %sync_bits to i32
  ret i32 %sync_value
listener_post:
  call void @dartforge_ffi_callback_post(ptr %ctx, ptr %buf, i64 0)
  ret i32 0
local:
  %c = ptrtoint ptr %ctx to i64
  %r = call i64 @df.ffi.j_$cb(i64 %c)
  %x = call i8 @dartforge_ffi_callback_exit(ptr %ctx, ptr %saida)
  %e_excecao = icmp ne i8 %x, 0
  br i1 %e_excecao, label %excecao, label %normal
excecao:
  %e = load i64, ptr %saida
  %ev = trunc i64 %e to i32
  ret i32 %ev
normal:
  %nv = trunc i64 %r to i32
  ret i32 %nv
}

@df.ffi.cbkey.13 = private unnamed_addr constant [2 x i8] c"m_"
define internal i64 @df.ffi.cbentry.13(ptr nest %ctx) nounwind {
entrada:
  %modo = call i64 @dartforge_ffi_callback_enter(ptr %ctx)
  %is_closed_callback = icmp eq i64 %modo, 2
  br i1 %is_closed_callback, label %closed_callback, label %active_callback
closed_callback:
  %late_bits = call i64 @dartforge_ffi_callback_late_return(ptr %ctx)
  %late_value = add i64 %late_bits, 0
  ret i64 %late_value
active_callback:
  %saida = alloca i64
  %buf = alloca [1 x i64]
  %e_ouvinte = icmp ne i64 %modo, 0
  br i1 %e_ouvinte, label %ouvinte, label %local
ouvinte:
  %is_sync_callback = icmp eq i64 %modo, 3
  br i1 %is_sync_callback, label %foreign_sync, label %listener_post
foreign_sync:
  %sync_bits = call i64 @dartforge_ffi_callback_sync(ptr %ctx, ptr %buf, i64 0)
  %sync_value = add i64 %sync_bits, 0
  ret i64 %sync_value
listener_post:
  call void @dartforge_ffi_callback_post(ptr %ctx, ptr %buf, i64 0)
  ret i64 0
local:
  %c = ptrtoint ptr %ctx to i64
  %r = call i64 @df.ffi.m_$cb(i64 %c)
  %x = call i8 @dartforge_ffi_callback_exit(ptr %ctx, ptr %saida)
  %e_excecao = icmp ne i8 %x, 0
  br i1 %e_excecao, label %excecao, label %normal
excecao:
  %e = load i64, ptr %saida
  %ev = add i64 %e, 0
  ret i64 %ev
normal:
  ret i64 %r
}

@df.ffi.cbkey.14 = private unnamed_addr constant [2 x i8] c"f_"
define internal float @df.ffi.cbentry.14(ptr nest %ctx) nounwind {
entrada:
  %modo = call i64 @dartforge_ffi_callback_enter(ptr %ctx)
  %is_closed_callback = icmp eq i64 %modo, 2
  br i1 %is_closed_callback, label %closed_callback, label %active_callback
closed_callback:
  %late_bits = call i64 @dartforge_ffi_callback_late_return(ptr %ctx)
  %late_double = bitcast i64 %late_bits to double
  %late_value = fptrunc double %late_double to float
  ret float %late_value
active_callback:
  %saida = alloca i64
  %buf = alloca [1 x i64]
  %e_ouvinte = icmp ne i64 %modo, 0
  br i1 %e_ouvinte, label %ouvinte, label %local
ouvinte:
  %is_sync_callback = icmp eq i64 %modo, 3
  br i1 %is_sync_callback, label %foreign_sync, label %listener_post
foreign_sync:
  %sync_bits = call i64 @dartforge_ffi_callback_sync(ptr %ctx, ptr %buf, i64 0)
  %sync_double = bitcast i64 %sync_bits to double
  %sync_value = fptrunc double %sync_double to float
  ret float %sync_value
listener_post:
  call void @dartforge_ffi_callback_post(ptr %ctx, ptr %buf, i64 0)
  ret float 0.0
local:
  %c = ptrtoint ptr %ctx to i64
  %r = call double @df.ffi.f_$cb(i64 %c)
  %x = call i8 @dartforge_ffi_callback_exit(ptr %ctx, ptr %saida)
  %e_excecao = icmp ne i8 %x, 0
  br i1 %e_excecao, label %excecao, label %normal
excecao:
  %e = load i64, ptr %saida
  %ed = bitcast i64 %e to double
  %ev = fptrunc double %ed to float
  ret float %ev
normal:
  %nv = fptrunc double %r to float
  ret float %nv
}

@df.ffi.cbkey.15 = private unnamed_addr constant [2 x i8] c"d_"
define internal double @df.ffi.cbentry.15(ptr nest %ctx) nounwind {
entrada:
  %modo = call i64 @dartforge_ffi_callback_enter(ptr %ctx)
  %is_closed_callback = icmp eq i64 %modo, 2
  br i1 %is_closed_callback, label %closed_callback, label %active_callback
closed_callback:
  %late_bits = call i64 @dartforge_ffi_callback_late_return(ptr %ctx)
  %late_value = bitcast i64 %late_bits to double
  ret double %late_value
active_callback:
  %saida = alloca i64
  %buf = alloca [1 x i64]
  %e_ouvinte = icmp ne i64 %modo, 0
  br i1 %e_ouvinte, label %ouvinte, label %local
ouvinte:
  %is_sync_callback = icmp eq i64 %modo, 3
  br i1 %is_sync_callback, label %foreign_sync, label %listener_post
foreign_sync:
  %sync_bits = call i64 @dartforge_ffi_callback_sync(ptr %ctx, ptr %buf, i64 0)
  %sync_value = bitcast i64 %sync_bits to double
  ret double %sync_value
listener_post:
  call void @dartforge_ffi_callback_post(ptr %ctx, ptr %buf, i64 0)
  ret double 0.0
local:
  %c = ptrtoint ptr %ctx to i64
  %r = call double @df.ffi.d_$cb(i64 %c)
  %x = call i8 @dartforge_ffi_callback_exit(ptr %ctx, ptr %saida)
  %e_excecao = icmp ne i8 %x, 0
  br i1 %e_excecao, label %excecao, label %normal
excecao:
  %e = load i64, ptr %saida
  %ev = bitcast i64 %e to double
  ret double %ev
normal:
  ret double %r
}

@df.ffi.cbkey.16 = private unnamed_addr constant [2 x i8] c"b_"
define internal zeroext i1 @df.ffi.cbentry.16(ptr nest %ctx) nounwind {
entrada:
  %modo = call i64 @dartforge_ffi_callback_enter(ptr %ctx)
  %is_closed_callback = icmp eq i64 %modo, 2
  br i1 %is_closed_callback, label %closed_callback, label %active_callback
closed_callback:
  %late_bits = call i64 @dartforge_ffi_callback_late_return(ptr %ctx)
  %late_value = trunc i64 %late_bits to i1
  ret i1 %late_value
active_callback:
  %saida = alloca i64
  %buf = alloca [1 x i64]
  %e_ouvinte = icmp ne i64 %modo, 0
  br i1 %e_ouvinte, label %ouvinte, label %local
ouvinte:
  %is_sync_callback = icmp eq i64 %modo, 3
  br i1 %is_sync_callback, label %foreign_sync, label %listener_post
foreign_sync:
  %sync_bits = call i64 @dartforge_ffi_callback_sync(ptr %ctx, ptr %buf, i64 0)
  %sync_value = trunc i64 %sync_bits to i1
  ret i1 %sync_value
listener_post:
  call void @dartforge_ffi_callback_post(ptr %ctx, ptr %buf, i64 0)
  ret i1 0
local:
  %c = ptrtoint ptr %ctx to i64
  %r = call i1 @df.ffi.b_$cb(i64 %c)
  %x = call i8 @dartforge_ffi_callback_exit(ptr %ctx, ptr %saida)
  %e_excecao = icmp ne i8 %x, 0
  br i1 %e_excecao, label %excecao, label %normal
excecao:
  %e = load i64, ptr %saida
  %ev = trunc i64 %e to i1
  ret i1 %ev
normal:
  ret i1 %r
}

@df.ffi.cbkey.17 = private unnamed_addr constant [5 x i8] c"l_HHl"
define internal i64 @df.ffi.cbentry.17(ptr nest %ctx, ptr %a0, ptr %a1, i64 %a2) nounwind {
entrada:
  %modo = call i64 @dartforge_ffi_callback_enter(ptr %ctx)
  %is_closed_callback = icmp eq i64 %modo, 2
  br i1 %is_closed_callback, label %closed_callback, label %active_callback
closed_callback:
  %late_bits = call i64 @dartforge_ffi_callback_late_return(ptr %ctx)
  %late_value = add i64 %late_bits, 0
  ret i64 %late_value
active_callback:
  %saida = alloca i64
  %buf = alloca [3 x i64]
  %e_ouvinte = icmp ne i64 %modo, 0
  br i1 %e_ouvinte, label %ouvinte, label %local
ouvinte:
  %o0 = ptrtoint ptr %a0 to i64
  %g0 = getelementptr [3 x i64], ptr %buf, i64 0, i64 0
  store i64 %o0, ptr %g0
  %o1 = ptrtoint ptr %a1 to i64
  %g1 = getelementptr [3 x i64], ptr %buf, i64 0, i64 1
  store i64 %o1, ptr %g1
  %o2 = add i64 %a2, 0
  %g2 = getelementptr [3 x i64], ptr %buf, i64 0, i64 2
  store i64 %o2, ptr %g2
  %is_sync_callback = icmp eq i64 %modo, 3
  br i1 %is_sync_callback, label %foreign_sync, label %listener_post
foreign_sync:
  %sync_bits = call i64 @dartforge_ffi_callback_sync(ptr %ctx, ptr %buf, i64 3)
  %sync_value = add i64 %sync_bits, 0
  ret i64 %sync_value
listener_post:
  call void @dartforge_ffi_callback_post(ptr %ctx, ptr %buf, i64 3)
  ret i64 0
local:
  %c = ptrtoint ptr %ctx to i64
  %d0 = ptrtoint ptr %a0 to i64
  %d1 = ptrtoint ptr %a1 to i64
  %r = call i64 @df.ffi.l_HHl$cb(i64 %c, i64 %d0, i64 %d1, i64 %a2)
  %x = call i8 @dartforge_ffi_callback_exit(ptr %ctx, ptr %saida)
  %e_excecao = icmp ne i8 %x, 0
  br i1 %e_excecao, label %excecao, label %normal
excecao:
  %e = load i64, ptr %saida
  %ev = add i64 %e, 0
  ret i64 %ev
normal:
  ret i64 %r
}

define void @dartforge_entry() nounwind {
  call void @dartforge_register_isolates(ptr @df.prepare_isolate, ptr @dartforge_call_dart0)
  call void @df.prepare_isolate()
  call void @dart_main()
  call void @dartforge_event_loop(ptr @dartforge_call_dart0)
  ret void
}

define i32 @main() nounwind {
  %r = call i32 @dartforge_init(ptr @dartforge_entry, ptr @dartforge_dispatch_toString)
  ret i32 %r
}

@df.seln.0 = private unnamed_addr constant [24 x i8] c"g:millisecondsSinceEpoch"
@df.seln.1 = private unnamed_addr constant [20 x i8] c"tc:writeAsStringSync"
@df.seln.2 = private unnamed_addr constant [13 x i8] c"tc:existsSync"
@df.seln.3 = private unnamed_addr constant [10 x i8] c"c:toString"

@df.area = private unnamed_addr constant [50 x i64] [i64 -7318119598941579387, i64 48, i64 -7122046887823521421, i64 2008984066716003743, i64 -7846682197585810633, i64 361698977638662171, i64 -2211443304049680192, i64 -2211442204538051981, i64 -1587278040669626517, i64 -1587279140181254728, i64 -963112777289572842, i64 -963111677777944631, i64 -338947513909519167, i64 -338948613421147378, i64 -4326468268956853396, i64 -4326467169445225185, i64 -3702303005576799721, i64 -3702304105088427932, i64 -3078137742196746046, i64 -3078136642685117835, i64 -2453972478816692371, i64 -2453973578328320582, i64 -1829807215436638696, i64 -1829806115925010485, i64 -1330385971727080951, i64 -1330387071238709162, i64 -457777237921703800, i64 -457776138410075589, i64 8836240676947637268, i64 8836241776459265479, i64 -4374652114954472769, i64 -4374653214466100980, i64 -3750486851574419094, i64 -3750485752062790883, i64 -3126321588194365419, i64 -3126322687705993630, i64 6339579623427422568, i64 6339580722939050779, i64 6963744886807476243, i64 6963743787295848032, i64 7587910150187529918, i64 7587911249699158129, i64 8212075413567583593, i64 8212074314055955382, i64 8454604588334595772, i64 8454605687846223983, i64 9078769851714649447, i64 9078768752203021236, i64 -4140364399009050659, i64 -4140365498520678870]
define void @df.prepare_area() nounwind {
  %a = call ptr @dartforge_globals_area(ptr @df.area)
  ret void
}
@df.area_id.program = internal global i64 0, align 8
define internal ptr @df.get_area() alwaysinline nounwind {
%ctx = call ptr @dartforge_context()
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
declare i64 @df.dart$3aasync.._asyncAwait(i64, i64)
declare i64 @df.dart$3aasync.._asyncRethrow(i64, i64, i64)
declare i64 @df.dart$3aasync.._asyncReturn(i64, i64)
declare i64 @df.dart$3aasync.._asyncStartSync(i64, i64)
declare i64 @df.dart$3aasync.._makeAsyncAwaitCompleter(i64)
declare i64 @df.dart$3aasync.._wrapBody(i64)
declare i64 @df.dart$3aasync.Future.new$3adelayed(i64, i64, i64)
declare void @df.dart$3acore.DateTime.new$3anow(i64)
declare void @df.dart$3acore.Duration.new(i64, i64, i64, i64, i64, i64, i64)
declare i64 @df.dart$3acore.Object.$3d$3d$g(i64, ptr, ptr)
declare i64 @df.dart$3acore.Object.$3d$3d$tc(i64, ptr, ptr)
declare i64 @df.dart$3acore.Object._identityHashCode$c(i64, ptr, ptr)
declare i64 @df.dart$3acore.Object._identityHashCode$g(i64, ptr, ptr)
declare i64 @df.dart$3acore.Object._instanceOf$c(i64, ptr, ptr)
declare i64 @df.dart$3acore.Object._instanceOf$g(i64, ptr, ptr)
declare i64 @df.dart$3acore.Object._simpleInstanceOf$c(i64, ptr, ptr)
declare i64 @df.dart$3acore.Object._simpleInstanceOf$g(i64, ptr, ptr)
declare i64 @df.dart$3acore.Object._simpleInstanceOfFalse$c(i64, ptr, ptr)
declare i64 @df.dart$3acore.Object._simpleInstanceOfFalse$g(i64, ptr, ptr)
declare i64 @df.dart$3acore.Object._simpleInstanceOfTrue$c(i64, ptr, ptr)
declare i64 @df.dart$3acore.Object._simpleInstanceOfTrue$g(i64, ptr, ptr)
declare i64 @df.dart$3acore.Object.hashCode$c(i64, ptr, ptr)
declare i64 @df.dart$3acore.Object.hashCode$g(i64, ptr, ptr)
declare i64 @df.dart$3acore.Object.noSuchMethod$c(i64, ptr, ptr)
declare i64 @df.dart$3acore.Object.noSuchMethod$g(i64, ptr, ptr)
declare i64 @df.dart$3acore.Object.noSuchMethod$tc(i64, ptr, ptr)
declare i64 @df.dart$3acore.Object.runtimeType$c(i64, ptr, ptr)
declare i64 @df.dart$3acore.Object.runtimeType$g(i64, ptr, ptr)
declare i64 @df.dart$3acore.Object.toString$g(i64, ptr, ptr)
declare i64 @df.dart$3aio.File.new(i64)
declare i64 @df.dart$3aio.FileMode.append()
declare ptr @df.mt.dart$3acore.DateTime()
declare ptr @df.mt.dart$3acore.Duration()
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


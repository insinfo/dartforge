; DartForge contract provenance dae26ed2165021ec578a751b71a520cf594c11c3b560003e06bbde049298599f
; df.cone-class 0
; df.cone-class 1024
; df.cone-class 1025
; df.cone-class 1026
; df.cone-class 1027
; df.cone-parent 1024 0
; df.cone-parent 1024 602
; df.cone-parent 1025 0
; df.cone-parent 1025 602
; df.cone-parent 1025 1024
; df.cone-parent 1026 0
; df.cone-parent 1026 602
; df.cone-parent 1026 1024
; df.cone-parent 1027 0
; df.cone-parent 1027 602
; df.cone-parent 1027 1024
; df.cone-parent 1073741569 621
; df.class 1024 main$2edart Backend
; df.class 1027 main$2edart WindowsBackend
; df.class 1026 main$2edart MacBackend
; df.class 1025 main$2edart LinuxBackend
; df.fields 1024 0 
; df.fields 1027 0 
; df.fields 1026 0 
; df.fields 1025 0 
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
define internal ptr @df.body(i64 %o, ptr %ctx) alwaysinline nounwind {
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
@.str.0 = private unnamed_addr constant [13 x i8] c"win32 backend"
@.str.1 = private unnamed_addr constant [13 x i8] c"macos backend"
@.str.2 = private unnamed_addr constant [13 x i8] c"linux backend"
@.str.3 = private unnamed_addr constant [5 x i8] c"macos"
@.str.4 = private unnamed_addr constant [7 x i8] c"windows"
@.str.5 = private unnamed_addr constant [5 x i8] c"linux"
@.str.6 = private unnamed_addr constant [14 x i8] c"F\3C0\3BC1027\3B0\3B\3B\3E"
@.str.7 = private unnamed_addr constant [14 x i8] c"F\3C0\3BC1026\3B0\3B\3B\3E"
@.str.8 = private unnamed_addr constant [14 x i8] c"F\3C0\3BC1025\3B0\3B\3B\3E"
@.str.9 = private unnamed_addr constant [20 x i8] c"C597\3CF\3C0\3BC1024\3B0\3B\3B\3E\3E"
@.str.10 = private unnamed_addr constant [1 x i8] c"\2F"
@.str.11 = private unnamed_addr constant [15 x i8] c"posix separator"
@.str.12 = private unnamed_addr constant [17 x i8] c"windows separator"
@.str.13 = private unnamed_addr constant [6 x i8] c"Object"
@.str.14 = private unnamed_addr constant [7 x i8] c"Backend"
@.str.15 = private unnamed_addr constant [14 x i8] c"WindowsBackend"
@.str.16 = private unnamed_addr constant [10 x i8] c"MacBackend"
@.str.17 = private unnamed_addr constant [12 x i8] c"LinuxBackend"
@.str.18 = private unnamed_addr constant [11 x i8] c"_AsyncFrame"
@.str.19 = private unnamed_addr constant [5 x i8] c"_Type"

; VTable of class Object (id 0)
; VTable of class Backend (id 1024)
; VTable of class WindowsBackend (id 1027)
; VTable of class MacBackend (id 1026)
; VTable of class LinuxBackend (id 1025)
; VTable of class _AsyncFrame (id 1073741568)
; VTable of class _Type (id 1073741569)
; Closures: invalid entry and constant vectors
define internal i64 @df_clo_invalido(i64 %c, ptr %a, ptr %d) nounwind {
b0:
  ret i64 0
}
@df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892 = linkonce_odr hidden unnamed_addr constant [2 x i64] [i64 0, i64 0], align 8
@df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da = linkonce_odr hidden unnamed_addr constant [2 x i64] [i64 1, i64 0], align 8
@df.const.i64.3079.881c74f717bc41c6a12fbe6ffdcbfb8c7e587b27e7f448af650591eb1a83a01f = linkonce_odr hidden unnamed_addr constant [3079 x i64] [i64 411618295301120, i64 -5252157194895566216, i64 8649146247807510670, i64 3229913158401429197, i64 5823957813885055579, i64 -7734112834750329666, i64 -7580918016866327652, i64 -4503590212799566854, i64 -9058948542785170772, i64 -436215690504191712, i64 8029388995328011772, i64 4212949187444210111, i64 7291696587903578111, i64 -3616610623884232962, i64 1262120048382210946, i64 -1861747933826820312, i64 4917076469554800603, i64 -6602841096474915585, i64 7906604606936508671, i64 6163917276658838660, i64 2980016446738099348, i64 1120954939964213977, i64 7082405167427936191, i64 8238717890043455535, i64 -8793057957841657872, i64 4906598963786280908, i64 -2418335702598506960, i64 1224098457401642909, i64 4142256087068791140, i64 4483831151489346543, i64 -1729787978704351037, i64 -6194071320993333432, i64 1919839592253313617, i64 6211953478132618517, i64 -81118809799085654, i64 1772809909706930292, i64 -1297427784624778667, i64 637271669006464237, i64 -2221124492125785128, i64 3407797743376543944, i64 -6002588623340854664, i64 -6504378603737732144, i64 -7310700279561279325, i64 -2972175186931789467, i64 -8416431272356116904, i64 5015156400676595290, i64 -7015903734554564819, i64 5292669361104518911, i64 8072738987485817691, i64 242139539496247341, i64 6306051530319728984, i64 4063837925791461327, i64 1703021965765407439, i64 -7523149720726719678, i64 -3109404735178840745, i64 -9182184956637369116, i64 9182546584146207920, i64 -1663088292205759625, i64 5241600041480608827, i64 6186226253668512554, i64 9069815911867436224, i64 3224467647122832858, i64 -8234385361111395724, i64 301523419428552723, i64 -4954404447936651682, i64 -675768749376308443, i64 -6062989742738811081, i64 7045882766693284149, i64 -7297049927859412959, i64 -2983439942327513891, i64 6262142234978943212, i64 5818225838077735034, i64 -7767862496180361670, i64 7652828848303773470, i64 -4230738597087143540, i64 2534978320121070449, i64 4725508186782630983, i64 7285118929994832426, i64 -3185133173860335061, i64 -6015824330380142356, i64 2378372242475994667, i64 6582414158826949629, i64 4006712149372226451, i64 1289479438327839147, i64 4696363337818684432, i64 7418322344613301552, i64 1184007873365325721, i64 2267403874117281522, i64 1336164938005773847, i64 8446966553355234475, i64 297272129385616282, i64 1199986007251789572, i64 -4999902324492004532, i64 -8033450286206299243, i64 4559214087088852819, i64 4002232412795150364, i64 -902497596953364049, i64 8239107865574324109, i64 -7220127161673740283, i64 -7815168022964019609, i64 6686873317401199643, i64 4717296410014250776, i64 4528214675354294131, i64 7505920868078848228, i64 -1871847455837376346, i64 7958110397795642414, i64 -4193788738761428464, i64 5867967212164059659, i64 -1957109456962251021, i64 7670178341609144891, i64 -3983551481373495424, i64 993542250654188988, i64 -8273563066452222398, i64 -5069721009142526341, i64 2080499997945338443, i64 5995581044466070675, i64 -7533656936839623656, i64 2379843501296905814, i64 2187846411397798263, i64 -5557887078398862090, i64 9056148081622068710, i64 1446174182459460878, i64 -4852553273296813476, i64 2816396546406213221, i64 -4127006639759292361, i64 -7803690494709031524, i64 -1285431091728036126, i64 2841063200793339116, i64 2090800882250522053, i64 4762860255537269151, i64 3995315412340568814, i64 -8167068538586477905, i64 -9003581793195590060, i64 -7754962170478370876, i64 6808416785364148801, i64 7285022997591783636, i64 -3924820940976728956, i64 -4016725695164796778, i64 -6288820811579696738, i64 -691074503442803473, i64 7118672544123545228, i64 -9211531859655170419, i64 -7666836952241595924, i64 2367945082793685181, i64 2263114554270207844, i64 -3145356087857043631, i64 -8421154349082384439, i64 -3775586209961273294, i64 2891421189757181722, i64 7096178281036684420, i64 -5384655653508568355, i64 9055970921576215222, i64 1227663036673196886, i64 -8258294172653964660, i64 -8914840038559919665, i64 -4403358403685585243, i64 -5777801791639200437, i64 -2986662792123242709, i64 1744457580856768371, i64 4480270039080538213, i64 3492387325525185671, i64 -5706245143774572990, i64 -5410514205203732576, i64 5315083186482180859, i64 -2496488111299990760, i64 6534966440898681520, i64 -5702301837507579683, i64 -1488969737164838239, i64 974724527203653149, i64 -5561830326000951974, i64 8239174182604682376, i64 8767749342315949849, i64 -5955876721776808616, i64 -2848054693825423239, i64 3751604526660481190, i64 5099145039063993215, i64 3703725176599083437, i64 -4060315470088791247, i64 6245446320493527029, i64 7802581652367387720, i64 5211628251745372243, i64 -3818899616724134228, i64 629702148395206922, i64 265277848902273456, i64 2317490672146452088, i64 -3059709045520018215, i64 -8522378906532738630, i64 -7250727823897675051, i64 -5676970525978172818, i64 3690141169529428908, i64 -8451050163363792485, i64 -7113989673581725873, i64 6005159034894439254, i64 6240954801125021340, i64 2905761413996960851, i64 7938063960436115736, i64 73908711369952210, i64 -891954157875006557, i64 -3572244613315661947, i64 -7262499453522031206, i64 673108815673483958, i64 -3688733046868317778, i64 7413089341333621990, i64 -3652442055916489882, i64 -5914760609039861337, i64 8172589236935436054, i64 -5914979219132798086, i64 2907205118977169174, i64 3714157917439002776, i64 5879566081024115066, i64 -5711224269762725029, i64 8621885703756484716, i64 -4190746300517540771, i64 -1484191860073833163, i64 1724291268381884890, i64 -3054463679196120121, i64 -5013408751023618197, i64 2379423939292733398, i64 4087701993372243225, i64 6765404614429960637, i64 -800696687463383681, i64 -2426994891792512090, i64 5072069280280768884, i64 2680577804567458589, i64 5401321365873819303, i64 -7697831690930063643, i64 4107466184174434779, i64 3873811375296763055, i64 6138515636159629317, i64 -2008375766191785724, i64 1908916260040029140, i64 952969626721577518, i64 -6425622861735472230, i64 -2201850043145602784, i64 5635187198390469274, i64 8219184153759604355, i64 3116297026708568778, i64 6083739726284841822, i64 -5384683122000982805, i64 -7794785153027313458, i64 -4378939163460021156, i64 2360113482697377077, i64 2419996793918731180, i64 6275566217159506071, i64 -6256031399038447077, i64 6526527065201280805, i64 2060014994088966258, i64 5445056920482600238, i64 6212841422515718506, i64 -512975851882865548, i64 3621952693825523033, i64 8141018725108864405, i64 -2949950843501236903, i64 -4979358399615001068, i64 4516364346968600826, i64 7173489592478926896, i64 8715018828684445357, i64 1348639122729695347, i64 -3433578950570466319, i64 -5338303723711162985, i64 -2033738191815187514, i64 -4996774818949642597, i64 -1011802546025664198, i64 6315450173404803500, i64 -6949306223142003161, i64 -4864311827184990469, i64 8767593143061576213, i64 -2973351199065612312, i64 -4861403265921270648, i64 -8910727536183572266, i64 7462775534287259696, i64 2547316608210028685, i64 -1331257671511069454, i64 -6130088037019040182, i64 3918105908906215782, i64 -2973327008846653814, i64 8979850812630203550, i64 -6122590234644537645, i64 2002392538460901147, i64 -7001275264899368259, i64 -653018969282961988, i64 -5548985582981342775, i64 -956071484650970522, i64 -4950033528911643814, i64 8970799608548252402, i64 6299195225432436435, i64 -5492162880702007906, i64 3905868765641434486, i64 -8615802494310139020, i64 -3420822579127933627, i64 -7504448781847914534, i64 -2706777884850660753, i64 -5403999573085857897, i64 4363617842587457569, i64 -2087436914073378965, i64 -7773712960318215871, i64 -7495627591905135034, i64 -4843802413256302387, i64 2120484700399701686, i64 -4842262967138468109, i64 -8163203620708708682, i64 -3516372745621885063, i64 2439720582834526634, i64 -8004536719475065540, i64 8749631243277038643, i64 -8471386558427712579, i64 6803788665856972699, i64 -9071568989652424593, i64 -7291561254752307593, i64 5798597584134956955, i64 -5213752685809915174, i64 3703646152723003438, i64 -2396831480418427416, i64 -3753896577554582791, i64 8859604447152224967, i64 3431153130324347915, i64 6478343074285810975, i64 -6612563954786241575, i64 -6769627085305296985, i64 -2455644633157281409, i64 -4826359630337167568, i64 3997775914274590174, i64 1998490865292116620, i64 4400674866225648614, i64 -6242638834924877525, i64 2192504716233604345, i64 -3491075260776935428, i64 -5982799718443168406, i64 3867253493924246742, i64 -1656590296655999091, i64 -5114869610514785613, i64 6199605250251402106, i64 -4456816466650152147, i64 2233861309951972852, i64 6531558826498277544, i64 2475472098956037154, i64 -8637862671943531212, i64 -7590482043875574869, i64 -5842588631734935360, i64 -7499898393825426554, i64 -3735530470567264671, i64 -880056619505142433, i64 7025429243728698396, i64 -7467014725923001514, i64 7695582201230912213, i64 -2212098670463674379, i64 9175539303862416140, i64 -4381891615450884735, i64 5951442572599010543, i64 6395059993097845747, i64 -3050975960525240803, i64 1804424030555091041, i64 -6807376297650726260, i64 7867896119303443177, i64 443793732200306805, i64 6895119424474866043, i64 -6002232388320108491, i64 -6100825952439244483, i64 8842070414828331207, i64 -8998564420353176308, i64 -3071237153162286163, i64 7384037065501118954, i64 3433024982427381933, i64 7012162909084732950, i64 3141386085330112660, i64 -4290420113700381184, i64 8221673524963430559, i64 1056868915928047605, i64 -2034807384403239107, i64 -4933886957710520377, i64 -4048755719594838209, i64 -613912317941448817, i64 -7283750547040093175, i64 5965895745289498288, i64 5936793971783562342, i64 922405448689661736, i64 -2450018149611043303, i64 -6763300057277941285, i64 702819415619137678, i64 -3598535549002468882, i64 3207786329891392921, i64 3228172279035151973, i64 -3721774793906624922, i64 -6964824440882322740, i64 -7954231940847164585, i64 1549068407829140769, i64 6988835637090182786, i64 -2450631459300977427, i64 4319930568050372857, i64 5712364558656578681, i64 -7347603342228165617, i64 -6754552240082767335, i64 2108886636461092295, i64 -7452000855840228703, i64 -4724488602651255045, i64 8897488838348857127, i64 3714118636314790672, i64 -3803092851354289997, i64 1026191941183631134, i64 4444475194308277665, i64 -3777517946869498162, i64 -5572391336847804661, i64 9197424581415856886, i64 7316157010069290606, i64 -5298799728662110610, i64 -5775825778064440829, i64 5913761662603452713, i64 2250264807747662475, i64 3188553700801138767, i64 7003616692918401331, i64 -9055788939290599361, i64 -7632168444314795550, i64 7626634569527375769, i64 1677139067924393494, i64 135209176127248898, i64 932835440916985879, i64 -7056852226314921527, i64 -212798429989995533, i64 -6599800902050313220, i64 7376383351895645494, i64 3775782908072873652, i64 7235257707762131762, i64 9001362053581640006, i64 -465062351941219191, i64 3838753990971806999, i64 -3418306967194918145, i64 259811847163805498, i64 -653369494751731521, i64 -8084196557433603074, i64 7218836689391859767, i64 2067123273097130792, i64 6300183003256332729, i64 2611932905504096835, i64 -331941486717567107, i64 -5704951471707031711, i64 8441518635800014021, i64 -6584371156192345463, i64 8968360869861045162, i64 3878069392772989782, i64 6679059825595705623, i64 8064887815700321059, i64 6236032554373028463, i64 -5780691999148919706, i64 -1444590764867307165, i64 -3859202644172650773, i64 903977547074627212, i64 -3672404779579571039, i64 -4699864728615630003, i64 -823617171014339635, i64 9099315440695295061, i64 7563401276632394529, i64 -8600311286301248850, i64 2808036294149415051, i64 1072406338109404503, i64 -2768250563152110774, i64 1930668111891115185, i64 -6801790992886877497, i64 -8416873758319598576, i64 7429722980189135668, i64 8527945147183510314, i64 -6241406796192600916, i64 7346437354030752996, i64 1300896419982840755, i64 -5088419394065067324, i64 1816272200232029452, i64 -4079463503265844169, i64 -2853907897138302774, i64 -6741531348188417835, i64 -3508280338685412115, i64 -8151401094687774870, i64 -1446807252429258434, i64 8731138737896152409, i64 8000857221942884612, i64 -8902190308083719125, i64 8831950738171793146, i64 8633356599900277235, i64 -442933536526448777, i64 7729977670888038577, i64 -7260105445471571697, i64 -9022357714826382612, i64 5143853842500425516, i64 -3454029732023139734, i64 1757549650373067014, i64 -6819820645084164750, i64 -4658381404655808907, i64 487507882545387768, i64 -3628215125590638057, i64 -7441752216511058145, i64 5072698632238300737, i64 3183798363553958131, i64 5540699215824453024, i64 -5958171154197360886, i64 -5385923808392948715, i64 7674834884257833039, i64 -2316401591456190433, i64 -7521787154170765345, i64 -7169756911742977057, i64 -8850428253362020401, i64 -591151179766048911, i64 -7122912917956073500, i64 -6230920376429621889, i64 1098815715403928898, i64 -3502009298866979944, i64 -2459042364686660974, i64 5701591689563426796, i64 -9093703116290407461, i64 8701363491995258469, i64 -1316760961631618178, i64 5405332557044438647, i64 -716187981198205952, i64 7336398493805312737, i64 3261084581840355706, i64 8655064501530816051, i64 7900346852971045790, i64 4571601383701853383, i64 -1171022796990757524, i64 -2593670767460631991, i64 -3644389081291006122, i64 -4245830460757772714, i64 6920397499524737901, i64 -3520709511582337220, i64 -8237064926325230024, i64 -2240181906470747739, i64 -8919657854570295586, i64 255686578410078894, i64 -5584418159702942681, i64 -2155945829763633580, i64 3727800643387701392, i64 4658291482440963606, i64 5981577498209478550, i64 2992289797138608171, i64 -7257539727116539143, i64 8009166588815750146, i64 -316040352841081656, i64 1702688985302140998, i64 -7383598962930063676, i64 -1737271692164516920, i64 -8315413166988575253, i64 -8835458124782409109, i64 -5329701771316207126, i64 5538434983841188409, i64 -3628540460205173046, i64 -3619865713746954307, i64 2331346030667150110, i64 -510615478630193739, i64 -7369981577771912962, i64 8317415898249469020, i64 8779339984478772584, i64 -2196847440083190509, i64 7796246755337578570, i64 2955826324045361449, i64 -580348268007748042, i64 2404034500344258821, i64 -3171810040152489318, i64 762041657499636831, i64 -2197358740108799708, i64 5656608234759417290, i64 -7063780555189267750, i64 -1632496146077322882, i64 1831209798204383424, i64 2937965032992959189, i64 -988411188792327808, i64 -8492430844865007698, i64 5077221592269525928, i64 -2469160454751085019, i64 7396222090828578247, i64 9178434345001821958, i64 9065740491637660305, i64 725198383534844635, i64 7095580137774342176, i64 7216877586974545956, i64 369385994130465952, i64 4903437053120091207, i64 665945636025554961, i64 -7813100695878318442, i64 -1388137354433213787, i64 76796645909128345, i64 1130421550263172522, i64 426062757747742512, i64 -3177324407873119278, i64 509457553055215342, i64 -3314463430261142925, i64 -4408892138779705134, i64 -2369897740669523249, i64 -761044128858399681, i64 3839297001297541870, i64 -5030209965489863077, i64 -7920373062559604042, i64 690902875868178074, i64 -869487299513765897, i64 2035591725258082159, i64 6450484059055904974, i64 -4689072860280212736, i64 -3699109555336480427, i64 -3189727484923928141, i64 7107314348771580935, i64 -6736781083097382892, i64 -6643696437769102538, i64 -329648067856114881, i64 -6200703861570715060, i64 4808057025669998657, i64 -5850781378531839851, i64 6676106566010493347, i64 1035634221266606753, i64 -4340344721592279177, i64 -7125268144208808223, i64 773430459215467053, i64 2072770340923679317, i64 4156574404801016116, i64 -3575919888860981678, i64 -9123611505481561593, i64 8790335873143379652, i64 -8117402882332128073, i64 5031612574135561937, i64 -5367039775436444751, i64 -907403260222663431, i64 1266910217373262633, i64 -2454603838263089516, i64 1172939051948389109, i64 7344124170916116779, i64 -1854883492804635821, i64 9052909960870524764, i64 -5886322118018418340, i64 618473245255999615, i64 1885112757318767085, i64 5812593167169760610, i64 5767056056126890277, i64 -7672515133939702586, i64 -6190775591731051754, i64 4657259928960711365, i64 1273111686903013751, i64 -7311172343648150029, i64 3754918498957506990, i64 568797637498290657, i64 3090631001735442635, i64 -3740431074255725740, i64 1379618311870096858, i64 7371833081787932752, i64 -1581521203156829645, i64 1473153217600037031, i64 -4244862905251246117, i64 6028141466552648897, i64 -7919565415713003110, i64 -143622189904858834, i64 1764028336568689706, i64 -1882387471305024677, i64 -576118246773984915, i64 -783564781353950307, i64 -3221923949838760276, i64 8928918578767123127, i64 4117786134848747529, i64 -3349452448278408649, i64 3982369123963442247, i64 2845207822506919256, i64 -328462072217233001, i64 -6962600487200241699, i64 -3722020912708496114, i64 -5339514118493651200, i64 -36138425199847872, i64 -9133375328781614193, i64 -1741624861726976260, i64 4259691425046965918, i64 -6504729628936192525, i64 -4703863924771613112, i64 3118228317654244932, i64 -1162537943583298732, i64 -5419365443439982649, i64 1051743201037320443, i64 8300861998238662823, i64 5810028375951414040, i64 -3170135994400521313, i64 3274623470454950081, i64 4259700737317215596, i64 -4487159306919498831, i64 -3103662460121123451, i64 -3421467614284967443, i64 -4123431056125922234, i64 319269525520837713, i64 5225304993234551917, i64 4071001820203109664, i64 4662677511443461846, i64 381411878519875818, i64 -3135344828658187554, i64 -4548624143593722472, i64 8001372928967264030, i64 -3829874842468112521, i64 -2246879992529642878, i64 84495289717001413, i64 -1318145401588082966, i64 -4119775404891793516, i64 7138720582060614774, i64 354390613076005167, i64 6474825763146786694, i64 5312612547598778042, i64 3846325334996700174, i64 -4177102083145760964, i64 6779571882044490232, i64 -6575517531668720591, i64 -7311480949156231656, i64 -3613958872606636335, i64 -3310810710913622592, i64 2643849390295063322, i64 -2646069922572805271, i64 -8791767442407280225, i64 7472884437485718583, i64 8494447586529971299, i64 7887877314600027859, i64 3877973076706811404, i64 2747220068369657182, i64 -9166770351061912459, i64 -2854547452544463083, i64 1310195315488903473, i64 3558507137333089980, i64 -1516176967053432954, i64 825562247100857357, i64 -4500553803364693961, i64 4825238712603812235, i64 -8003006512911541140, i64 4912190332534482978, i64 -2198111359892746755, i64 -1715479740124836191, i64 8422572060568270604, i64 -6202926988033248593, i64 1694256328077939502, i64 -4028483094860440297, i64 7390058543274452900, i64 8777311132941004550, i64 -4434543861390313345, i64 -7621484391400837702, i64 -2601627980918572007, i64 4102184912845079034, i64 -6264660963595103175, i64 -6215789579720207396, i64 -2926152358051590041, i64 -583220838338706644, i64 4722977491043968051, i64 -5515516331309782937, i64 -1166996396914357237, i64 6756481240378040319, i64 -7810220002207656153, i64 8924512726553781088, i64 2164377721452388996, i64 -1601562873415670533, i64 -7597680847104417951, i64 -1596816534829363359, i64 -8173981326132713758, i64 -6569844766267284589, i64 537472302454317328, i64 -3300134851243576511, i64 6225051827616434127, i64 4872395883285059110, i64 5281553740995737055, i64 -4730992467919828354, i64 2621074394583795609, i64 -8883151300977841579, i64 7730094789829266520, i64 -5918428629097316571, i64 3287292712824790023, i64 -1662895120060251830, i64 -860552345332627356, i64 6467466349649951331, i64 6465758259346703381, i64 -3615255108889017508, i64 -8415164949972139166, i64 -5101915778420901619, i64 7271602906382118448, i64 1939348825340902120, i64 -6839496259874763273, i64 -8448511608840751592, i64 -5018016778018835119, i64 6247718640196546368, i64 3017742960549278114, i64 7843341678581845726, i64 -1955876159271774887, i64 6663586498245164741, i64 -1701161177030185527, i64 3359597336602541856, i64 2093191265314480848, i64 -8016832209651277951, i64 4219385715552241917, i64 8007222284897614443, i64 -4971733739093890187, i64 -5910067440704996328, i64 -4792848306869798776, i64 -8288390042069707210, i64 8407722947748628076, i64 -6822772053728536177, i64 -5282099020824520994, i64 2150588332706220064, i64 6264188034242482536, i64 -1297185932698250205, i64 -2342343244381491454, i64 8045311504634402791, i64 -6239000324620716083, i64 -5177351178589421534, i64 -7804493350395640973, i64 -1764276746800977267, i64 4064460757342793430, i64 999127441455163732, i64 -6792673281826830533, i64 6044087311638048867, i64 -1350478119639787995, i64 3718359519666299542, i64 641976466727536417, i64 -6235590465893325636, i64 -3469575544160745667, i64 3733752698033612344, i64 466701501191132140, i64 6280124104192460239, i64 -2691445665877645081, i64 -2761138547513622778, i64 133527804419590001, i64 6090759364735117899, i64 4853754980721090603, i64 5513036516426606153, i64 4301164108377320344, i64 -4866533302882948464, i64 3368382859862119089, i64 -8624726904079963469, i64 5484906632223715756, i64 -6038450444837450837, i64 1237868837875426574, i64 -8080680090601308829, i64 3326874593697890147, i64 620564387069614680, i64 8100891223722430126, i64 7657542181442021783, i64 5785072809558692834, i64 -4800854868264772033, i64 1375188490219045504, i64 9167384972877641816, i64 -4555009451111529543, i64 -1297641535926303319, i64 769418294715621593, i64 9184771986286685379, i64 6928993321339254495, i64 -4017783022259783511, i64 1434246465695717065, i64 3260708394574716716, i64 2285327772790726022, i64 5762407139821731807, i64 8006940816063839227, i64 2519903010738376559, i64 1545763426525538212, i64 1538305081093043656, i64 -7936039038161247502, i64 -2757405021445568436, i64 7649335219947822719, i64 -5740212634655299625, i64 -8431741268929036423, i64 1807875397459671687, i64 -2040885857779605855, i64 6142127990168201268, i64 -2854817902188644899, i64 5023305039272528471, i64 731807532948029170, i64 6078140005855032042, i64 -8591020954698445582, i64 -5559233345964630443, i64 -922295471357038849, i64 -7912406724461146812, i64 2470917371530023330, i64 5394103302672357924, i64 -6961110166221898499, i64 7289972649432880296, i64 4937143445769725269, i64 -1009174170708846427, i64 3108452874820233187, i64 383844289756612149, i64 5860846447780713787, i64 6616395579729950372, i64 4957550425016450210, i64 -6379513811858181808, i64 5766059387676711942, i64 -5742856800716087068, i64 7588801424633245782, i64 -4040808790615593898, i64 -4658815779583716145, i64 -7480566095740200787, i64 -1087296570737533119, i64 -6877621406107698337, i64 -1592020173609166639, i64 -8853257012707997243, i64 2366682062302846502, i64 5782495577100548473, i64 -4200831797005817085, i64 -8369229577756114901, i64 -4471828653597772478, i64 3598761123849714384, i64 899725465333740937, i64 2805248512755080058, i64 -356387051716073575, i64 1943926799920475546, i64 -7308640165146866267, i64 3801701582205585781, i64 2646160758055604956, i64 2071986179298470991, i64 -6343784621382965253, i64 -1964131566938780467, i64 7335693152807806849, i64 -5708935865476209459, i64 -163982971845289657, i64 5995837959399384712, i64 -3124147108547297764, i64 1971911563701759697, i64 -6753576773056507595, i64 -8071656078889493985, i64 -7277446211274179765, i64 923305570088532361, i64 -8278760450271895430, i64 -1741624861813728836, i64 -8290451247235786978, i64 -2829211826040772881, i64 2806966463285513036, i64 3181407295877259771, i64 5689747590297205170, i64 7396956488697567539, i64 -4360230194487712325, i64 6618179519734282930, i64 -1239019166159641395, i64 -713313280640925342, i64 4768902309928842079, i64 8816557441536256343, i64 -5984595789352242047, i64 -9038804946099144379, i64 734933549325969461, i64 3461764821555970570, i64 3429074751070656655, i64 5887446730725082838, i64 3710955375392223922, i64 298314434126965049, i64 7150814651564350315, i64 -6745327120018151822, i64 -6171677388019537667, i64 -7059096132510934909, i64 -2040140741942760230, i64 7870946684342729885, i64 -5515794257810498522, i64 -4870609964311720941, i64 -7951914121430959777, i64 -7214851429126823926, i64 -8127883191608456035, i64 -7577560854739862254, i64 -1145628919027932867, i64 -8689529533571461551, i64 -6487262032087678892, i64 3131959401526625293, i64 -6468996499937829381, i64 -8261850577118127683, i64 -5729937530139515976, i64 8451988002475078696, i64 5337932782117083264, i64 -7056066251131287970, i64 7992079002578682283, i64 -8387473900581332097, i64 -1591825153344888038, i64 5996289524252077814, i64 7975986533751214737, i64 7409460669075492608, i64 2876331351942228422, i64 5040240718642517542, i64 -1839579480964696734, i64 -8710001281071275040, i64 -6295411107854241445, i64 -74773178658485583, i64 -1757224867171863630, i64 6252713623002728903, i64 1078040264632432219, i64 4783067338024178783, i64 -1216648754464691023, i64 2060885951251447326, i64 -475470398368952771, i64 2786278680409711301, i64 4494442481348795568, i64 7580536577673072382, i64 1189813139947999850, i64 -8108170433147923139, i64 -3909082974271762825, i64 -7468062527660248379, i64 6824930835109407702, i64 2088060002597208676, i64 -2688329081835310811, i64 -4831233411660820101, i64 -7238934644525245438, i64 -3575308568369912143, i64 -5354091689999486437, i64 3473845379611912886, i64 8295242334326800435, i64 -7188424313181549350, i64 -3769841576863235668, i64 24919879194143529, i64 -2154884086513103791, i64 -5977356512841868937, i64 -7281209221068856243, i64 -4707863300191963900, i64 -7238035622993703650, i64 1622261948596840380, i64 -1511991300013675608, i64 -9107812251488039662, i64 -6189381983492518586, i64 6872431236045167228, i64 -6872989186544784705, i64 -8180112740760629005, i64 6466759139793406769, i64 8604685575204552383, i64 -6882678967689547014, i64 -1763527272337983516, i64 -7783592501315010134, i64 -5672788023395873380, i64 3037311222716747078, i64 7389928410174927986, i64 5308790811158335872, i64 2608924873129204877, i64 -2615819070174744407, i64 -3217111147388090989, i64 -2783447378209195780, i64 6494532148195936805, i64 -8253874759512319557, i64 1635668611356525442, i64 -1332719437365606353, i64 -4074995105529920922, i64 1554206645206364882, i64 -2734774275740614350, i64 -8776976837406002901, i64 -475414705410148956, i64 6295843311409245922, i64 -3136548588008717773, i64 -8090460627346433337, i64 7531137148194289605, i64 -6198245827624849379, i64 -1053131577447834699, i64 6016868105718609453, i64 -7154602729429211731, i64 -4983187405824712854, i64 646238435237107212, i64 3332493952945576111, i64 -5155639727099183541, i64 6618605958991301598, i64 -3470029217545362748, i64 -2835939782651175888, i64 -441955466586436577, i64 -6254068791881460710, i64 -146916843779131816, i64 9108695134106261501, i64 -1449379210244637856, i64 -2635959901984343972, i64 3218680875603954821, i64 5014713872912330739, i64 7625715636556509103, i64 -5417799260745503895, i64 -4212359286454744627, i64 3526148799256896919, i64 5809163955748331147, i64 6387570646754654646, i64 -2577523743396793354, i64 8770446620949511312, i64 7114328814758285688, i64 6692120407934855001, i64 1900303264524952751, i64 4447443980185771365, i64 -4085046581654009825, i64 -1939538943343838117, i64 -7073653432643132641, i64 -3156378916872571187, i64 -8470092637692699748, i64 6608169258774018153, i64 -3402838697912115224, i64 -1053125039397227812, i64 8472596343475150165, i64 -6341952936487411216, i64 6528097794064074376, i64 -1266187112237445462, i64 6474318492880024146, i64 4849434119088497354, i64 6531562421118935554, i64 -28454914729006799, i64 -2116397955472200365, i64 -3905227484666218766, i64 -451447724755563038, i64 6424361786596879672, i64 298121526296264539, i64 -608935830081438351, i64 -5557774454353571882, i64 -1686567643744201661, i64 -473101392951953491, i64 7997628266224230926, i64 -5878061752923915398, i64 -4187563822561547192, i64 -7575644077921692605, i64 -2498810341536203420, i64 -5226249311279259726, i64 -1288278128104177760, i64 7933355767452304135, i64 -4843714913045324111, i64 -6352354960639875684, i64 9006311915804867897, i64 -2959705221539841800, i64 -7873112211416806997, i64 8884411921977201744, i64 4588047882262222592, i64 8614893401172357455, i64 8985073100872373720, i64 -4182119503464395003, i64 -3482126262544140546, i64 3205012168769410685, i64 -5769886348538034338, i64 7041049767904032151, i64 -4102258768271903766, i64 692893531799319558, i64 -3175186359885052125, i64 6195757284519044491, i64 -5877060397390905409, i64 -6856498134843058299, i64 7583382309167176468, i64 -358765788107919252, i64 3126506954960501048, i64 -4068891675828274856, i64 5100912914261116182, i64 -8047743582510470011, i64 -8190169258459585759, i64 -18324366137171746, i64 -8162737399558768261, i64 8314723432928521717, i64 -3316816530047007189, i64 3846475533220836838, i64 -6854815900573727877, i64 5596555102837419878, i64 3939476334775069373, i64 2616380331293415781, i64 -1226109001898132310, i64 1831876196710404090, i64 4401779790945007828, i64 2459027561574967939, i64 -6803724625044168389, i64 -2247854992835658678, i64 -8151600240650410572, i64 7630617681625726789, i64 3196016854212117972, i64 1997462624146085435, i64 -1545982770069999214, i64 -8770405579667203101, i64 1746407825776586872, i64 6189136443416601640, i64 -1405870715184314328, i64 5763348684520652003, i64 -835952471903809163, i64 3115982254561748994, i64 1557498546071365574, i64 -8444622763936533589, i64 -5898639834547595691, i64 -5684210560321683523, i64 5099492011130130052, i64 -4952051027547723287, i64 580639217812116498, i64 -378278907213049489, i64 5848416067548707337, i64 386511481827343947, i64 -6192124894925919933, i64 -4036394620878194299, i64 -1970111766343723638, i64 -7703344309253024672, i64 -6703285588016329756, i64 3427895034011286674, i64 -4559785325079769850, i64 461102306925598352, i64 8472451990675861826, i64 5958676725944584904, i64 -6981268222523695681, i64 6533789669216443310, i64 2298887370699884715, i64 -8656547534192255458, i64 -5112573382145124703, i64 -2120212284328594944, i64 -651475011282639465, i64 -574930459107358527, i64 7616276320906882357, i64 -951448910167402582, i64 6595729813444676021, i64 2968385372159760128, i64 -4553402546349242135, i64 -1902101619916906700, i64 8033424057940526100, i64 3624684392597048726, i64 1579792131177941655, i64 -8858027340663820020, i64 -2441066650584938324, i64 5230866804514315831, i64 -2994307391133149653, i64 -1465721871410223018, i64 -7823432900862595141, i64 -6083551379145467498, i64 1614954649234049033, i64 3412216834977093347, i64 2831217888207995983, i64 -6840539975316063736, i64 -7051175328638483018, i64 -2568612300536420165, i64 4264281194635737254, i64 -5369480517236664019, i64 -5564855586489330061, i64 -1391728235144965545, i64 -7020536230144675591, i64 2381904425476840778, i64 -4143724920734520940, i64 5717621629743072777, i64 7228493937092594606, i64 -727235355110332905, i64 199505381898024393, i64 -8804145008171900279, i64 -3505726406328120599, i64 8383784112520731924, i64 -5517233358904313250, i64 -8437297890475119146, i64 -5804139530628876940, i64 7618908283086775970, i64 -7162922721126978215, i64 1631813123298481998, i64 493340447849715016, i64 4377331807157456743, i64 266372560086564510, i64 7219275804218572157, i64 -6803978992311387111, i64 -8518279477972618465, i64 7827698579182959842, i64 -2624942529446229932, i64 839226293870352816, i64 5659659307348695738, i64 -3136766087047342772, i64 1685907376126851521, i64 -4633495784830978268, i64 4730639447226169471, i64 8374756022276496118, i64 4311563130833123896, i64 -6712284340525304409, i64 2649388762678247444, i64 -8538310155010184073, i64 2396364938278619588, i64 -910417365601049854, i64 -742409191421859265, i64 5309034204350510434, i64 -9209543179507428859, i64 7097000278682722077, i64 798025032279792483, i64 -8901425907835155932, i64 6099150974573314724, i64 764633391542632814, i64 746846383566886213, i64 -8738329237367153339, i64 773641293236603052, i64 -6836651269475921659, i64 -4967097622344565678, i64 1668373323829485304, i64 -4095186932788646201, i64 -3573068406824068637, i64 -6322304716859390679, i64 2429713381619808294, i64 4693796151243096414, i64 -2570765252476197150, i64 1452009904384803988, i64 -1525303361708699030, i64 -6399097666586343455, i64 6667340173988626789, i64 -4992253340090832914, i64 -2154040868419952798, i64 -5601632238027366126, i64 8191795055480894980, i64 -7554622676739263849, i64 3535810282536536848, i64 6478166048674188740, i64 1935981371172659766, i64 1142313891693783889, i64 -1606817379078390905, i64 5726949722955069034, i64 -1462531535975987521, i64 -7615456335392796699, i64 -6295560383009396582, i64 5724866132536871234, i64 -7604869286872709452, i64 -2543562903792999813, i64 6300483862807147781, i64 972143448293235131, i64 4116684830764544248, i64 9201823208363348475, i64 -3996564544381906764, i64 673786263220062553, i64 6466616395995634117, i64 1982899794789119753, i64 -152607860274118918, i64 2166356962488089342, i64 -2530814572239691547, i64 -3972862893503686884, i64 6841217199838462861, i64 6359496362470946098, i64 4437633855297898906, i64 5663602214072983354, i64 7904593125194709988, i64 5160242407317209806, i64 -2544328279959349084, i64 -7117135660472290872, i64 5167106969787981424, i64 -8166114066018435029, i64 -1279390930285754968, i64 -7953981094357795636, i64 2560164880895546331, i64 2380377269367589947, i64 6744220671026808102, i64 3822394938667052866, i64 -2962283867153387402, i64 212154028614379275, i64 -5146374738465298992, i64 5810239906015627827, i64 -6682627959823654572, i64 646369422307598660, i64 -7890652042008568998, i64 5076211920009391667, i64 -6217597083455028966, i64 4099677195654505583, i64 -3059068646365841437, i64 4855218633215954383, i64 6112028557345171090, i64 -1710217707258968285, i64 -4031218390533974048, i64 -6941579200965248146, i64 9127171343558109859, i64 8442237103293049486, i64 7233961920355174996, i64 -8541932205095012999, i64 -926346682142329367, i64 -6176609049635680533, i64 -6259692713169743343, i64 -6592026778969421754, i64 1302008994686640410, i64 7188208075152858547, i64 3118940815700039065, i64 -697998664219220336, i64 7639993329487606931, i64 -9104771558652441177, i64 3983233722332962522, i64 -5887131158871845444, i64 -6191036417405091154, i64 7698690932169208089, i64 6144796151496980841, i64 -8264587377409395511, i64 1570538443608607093, i64 -5027656851845187877, i64 -2794787892332114420, i64 8894756867168069463, i64 -9185955350103759942, i64 -5986449530195676424, i64 -3431222972333846305, i64 -5703314380819479663, i64 5293992358054580385, i64 -864426291710744492, i64 1807494764958384298, i64 -1591331087548213856, i64 1317893768569898828, i64 8786778302968993139, i64 3242510367559942092, i64 -1606355519531045604, i64 243589390828538513, i64 -1778739550941585529, i64 -1900170149109262426, i64 -5817759522616193576, i64 7764647590953925380, i64 4856177290327957593, i64 775603429772917048, i64 8270120556044572597, i64 -4492273412425755122, i64 -2764506306487146052, i64 755636461133432801, i64 -5672728847592469338, i64 4718583499293660262, i64 -3535999961085989624, i64 2090982738888145584, i64 -1780805271779244702, i64 -7918310063069427898, i64 -5876189607064977749, i64 -6289867750962821219, i64 966792772160796594, i64 4977649656856271535, i64 8798926769010620072, i64 7188759799603724859, i64 -8938897392949035945, i64 -1776080402247041238, i64 -951831652067015137, i64 5959996982663828928, i64 5700016627770598816, i64 8330871590376920931, i64 4416796387808003177, i64 -5553561622296420839, i64 6217700155070399901, i64 6038748043670819437, i64 8056435520767098013, i64 -915874974532928857, i64 -7876274763545728406, i64 -9044224976515785184, i64 5737471429422079474, i64 786333055576784379, i64 234339226458437795, i64 -6730964532446136332, i64 8856266670235031642, i64 -7264767040197335782, i64 38610180045767204, i64 1824711300453793313, i64 6781038018157632355, i64 2674328417413827977, i64 -5543711215617047638, i64 8410284655830148608, i64 6175528078191399601, i64 -1757373178999139816, i64 2560187048620690181, i64 5168364828070501647, i64 8526047993815547819, i64 3888649555291644858, i64 -2676365566352279093, i64 1186835762546819023, i64 98934079855497822, i64 -3923217147778539818, i64 8170121284541251926, i64 -8015102359639544342, i64 5812965362073893400, i64 1018746297968010221, i64 -178878731269932361, i64 810771100651103830, i64 6955954274594491665, i64 -4873274582663003019, i64 2210276197533662745, i64 1636294694091006581, i64 -8628374322418299298, i64 -7900962797858058099, i64 2714065935412374615, i64 -3859993767016831799, i64 106453606671476224, i64 4477147119963977844, i64 -6237021391531269556, i64 -3073261616024236715, i64 4736699272715662853, i64 5814290878342814228, i64 -2839562528131087524, i64 1047996652514783121, i64 -2302481213074466523, i64 -767996352047966581, i64 -3032640621024924323, i64 -8327562154973480390, i64 -6488789385478429808, i64 -1283231329308103043, i64 7175924079481943818, i64 346474089313697729, i64 9172998606750781823, i64 7676160195175305011, i64 -4866019041062444775, i64 -5573056330595181940, i64 -37154806839503498, i64 797467143732876869, i64 7672036240940173856, i64 6607487045429817320, i64 4407728236321639341, i64 -5041708022311323047, i64 -8421327322056579908, i64 6394671430073774322, i64 3477108526074906274, i64 -2817078265491605468, i64 4170899883165079417, i64 7511189311544450894, i64 -7372631484329251241, i64 -4685525821012903508, i64 2089988585363663241, i64 895901749950210533, i64 2618989823215141549, i64 4696088593407703367, i64 -2911192114113362765, i64 -2654002041656520086, i64 -5845746830830525978, i64 3947555808313002789, i64 8686225060208603829, i64 -5548858070646000590, i64 8166782117778859146, i64 2753366589593801704, i64 -1419641203344830324, i64 -337041725656077559, i64 -6527202560449822764, i64 5520663950826067224, i64 5279089335531699475, i64 6745427536098837638, i64 -6124638822053931483, i64 -6707336239249292405, i64 -6044315214933490470, i64 -8109706517687222596, i64 -7445774878482080681, i64 -8578022763673354326, i64 -6617335770561283837, i64 2625361891810579527, i64 6743610902226398326, i64 3111395213917014809, i64 -7577490693561335527, i64 188140871690135072, i64 7659768324125647416, i64 6727799749190601197, i64 -5920590216306247913, i64 -5462590320488235334, i64 -419991866554625138, i64 7226540811774714376, i64 -1999372111876232401, i64 1606565054428604844, i64 5177209628569422270, i64 -2767604536938952600, i64 -3693051291931126451, i64 6022328098035482057, i64 3196101330042414957, i64 6913897742685147747, i64 6869542925939078737, i64 8467600985321055462, i64 1051913316746831913, i64 -8696591412826991473, i64 -5497450492456960560, i64 -5868825272825357336, i64 9090053332140526613, i64 -5683984437274278547, i64 471798804696134203, i64 -1098921535082416687, i64 -5457810914556836231, i64 7035986272502638594, i64 -4032180076980871425, i64 2243219996573439119, i64 1290814321557616990, i64 8489103714977385778, i64 2690230476718573279, i64 5037709699623747963, i64 8378252571862079092, i64 -2777033811498109034, i64 6151695296840542300, i64 1631256919036235070, i64 8185118562982863977, i64 6660150761251263689, i64 6897669004939495454, i64 7990325974701682986, i64 -390782067339164641, i64 7512312362929526161, i64 6431988083190195221, i64 -3801514031729771934, i64 -4700206660664504526, i64 -3209999140482917306, i64 4984123614302962984, i64 -7668466239674338520, i64 9043086756697048098, i64 1592392831550734392, i64 -535879648512378765, i64 -8361745495048040484, i64 7806698206953813286, i64 -8783120666700521516, i64 -656514994992627599, i64 8873452305345716113, i64 -2035303466376648407, i64 6822195174584454811, i64 -3032596917152382220, i64 -1796488191913784430, i64 841988726091910645, i64 8831008935895310665, i64 -3038162321717210493, i64 2275320041933482386, i64 2963328730245946866, i64 -1270840112510175457, i64 -7515859685757466617, i64 -506963961732872558, i64 8766238866795737676, i64 -4333731061630623916, i64 -5120130268797906575, i64 -5159500155212796688, i64 -6078082918247323744, i64 9201553038079760084, i64 3406585781038601406, i64 1089864455203616971, i64 7185903785399070896, i64 8041883340556067516, i64 -1216288974650488956, i64 -1462950974722114991, i64 1576592401157418792, i64 5336392398867018650, i64 8633197132029819199, i64 -8297604237678006712, i64 3783474072388584095, i64 -8887696994637325083, i64 -231482653929331647, i64 4716553221136681428, i64 -1058072173967148712, i64 -2395989221226590401, i64 5224462447914838977, i64 2676560536037129772, i64 3270814087350862848, i64 -6349465643469591753, i64 -6539897188088239748, i64 -1216009832603468227, i64 -8443300917279938726, i64 9219754822665272263, i64 -4293087293623210638, i64 -4675402794737606248, i64 -4068386360183163918, i64 -4127909557246154597, i64 -3233257995622989901, i64 -7112669089903511766, i64 3741551676513631206, i64 -5322479804232780402, i64 -1325869547388178922, i64 2764627209998430390, i64 1525679750774606045, i64 -7495065799127283746, i64 5245443005888330683, i64 -5269935347625123393, i64 8486671896087107374, i64 -4528405038024915876, i64 6831895866980179875, i64 -7226282970411783695, i64 -3446565788139142860, i64 3991956845312335501, i64 -6729145354192001545, i64 -8364994328509364954, i64 5869722518902216017, i64 3356029090547364011, i64 4572974780908977532, i64 -6680827772427668490, i64 2653842462594730394, i64 8822118976808626555, i64 -63174023696128123, i64 331511395772203124, i64 8808016766694972658, i64 -7910874962754078925, i64 -454217145878330998, i64 -6914484423130965596, i64 1571015676170389457, i64 401105184795479450, i64 3867195507718601475, i64 2938102275462953133, i64 8497494673163048489, i64 7147088083135175044, i64 7147087463543967278, i64 934767844756786734, i64 -8164204313453617485, i64 -8201227370583071960, i64 -2752405084604243160, i64 -5981577340871873581, i64 -4605386715217563147, i64 1751737339083736996, i64 2951386502334337643, i64 299293038704460154, i64 -4810529545566979409, i64 2463998115134483631, i64 -6225372885627661427, i64 3274603031918196034, i64 -7621736443451607982, i64 -827839636066988763, i64 -8746080890123808488, i64 -511483558305947731, i64 -182539887405357750, i64 8861838542682184711, i64 -5677582349715454372, i64 4032948587626480706, i64 -1928298406319009861, i64 2544199423206925582, i64 8761324537814579454, i64 6520890890669423145, i64 -2432977866168667175, i64 1671927697301064302, i64 -2069387142457782466, i64 5682379487165517703, i64 -7112226391129584380, i64 -5449144742284799831, i64 6345671078638432286, i64 3294091435836313108, i64 -303456042415364083, i64 1660587699676879960, i64 4375168152103258993, i64 7562082458015673851, i64 -5092072745652416812, i64 1865546489553145999, i64 -1423356657495455862, i64 1388162858945466171, i64 6328027347950226425, i64 -6093626876033304887, i64 -3150640729134791698, i64 8192767605254294350, i64 -8465516771185692852, i64 8543188976815802984, i64 3295997749146323146, i64 2937373226433206843, i64 -3426522224273840418, i64 5328956600456656050, i64 1705838823877212862, i64 -4375572925200131555, i64 6555968351934030293, i64 4402857626073167272, i64 6958829498482487626, i64 7104026898879112359, i64 8771848887365031654, i64 -5631835042562259376, i64 -9089394990908467289, i64 58688411039063902, i64 -3025341611979671373, i64 3377242115919214728, i64 308328873985947571, i64 767257526828973179, i64 7467118978778710613, i64 -8646132436087639144, i64 -6854751831023650710, i64 -4913294781061407766, i64 3197501327160978390, i64 1635751258941875605, i64 4412777063054613480, i64 287038366605032786, i64 5155233038701826709, i64 7274647785325617607, i64 -4679217764017688331, i64 4257865634916499622, i64 -7127324276564143925, i64 8294602170935595380, i64 2138570493396396994, i64 841347405236061527, i64 273203194066123963, i64 5084952881839495269, i64 -417579798973123819, i64 4420820359734555113, i64 2287147315090506736, i64 -4142519641211795542, i64 4296308174600411087, i64 7793325155544589153, i64 4820804492612440508, i64 46751937982708809, i64 -8684063110806407363, i64 -2610394711754082388, i64 -1122986324477336309, i64 -1118753030865505819, i64 -4474051221137328261, i64 -2098922803731414133, i64 4466877486015754602, i64 -8667328964278675140, i64 932777161748147867, i64 -4598930601023210555, i64 8372318254536466264, i64 -6244133875602072894, i64 -2227727028067425618, i64 -5882506081433843931, i64 6844295444378048350, i64 -7827904900560229098, i64 7448458392975520334, i64 5902521068970698203, i64 3588166857438095554, i64 -2551871958115183007, i64 3136924058554896539, i64 2318083163303707332, i64 5801889701133668331, i64 8668373394178003048, i64 -8121072225107661139, i64 1737328926286850004, i64 8497677448052449990, i64 -290208968813800444, i64 3825715696961799422, i64 2666682498141437226, i64 -3915846915949610040, i64 -2999385050654140511, i64 -8122593156320061455, i64 -4736452388221995984, i64 -5136561343477491109, i64 9002766155556464450, i64 -6344873809243820495, i64 4222020811632034085, i64 6293929713392067675, i64 5170509841942948873, i64 -5535980351006626896, i64 -6911848424346759961, i64 -2221591184173126400, i64 -669992867929503045, i64 -3656134909996892675, i64 -2513521624584111934, i64 -8929682242808745607, i64 -2758510335576358357, i64 8698422246155291661, i64 2478590661957639668, i64 -3750239271379067625, i64 4589215145464824285, i64 -5831930319752906065, i64 7939442335826697728, i64 -9152001279198460438, i64 2389014114507409533, i64 -8366521699935496641, i64 -501050900335198949, i64 6717394862362019554, i64 2109229683201367978, i64 8689209369344477035, i64 2921644964279415164, i64 6828990772614742257, i64 1376338485837925876, i64 -1401468809194136697, i64 6240342407853108257, i64 -3631220273138578015, i64 5077334566787386928, i64 1544069721734880662, i64 132735808396478296, i64 2123894425943023308, i64 6740661240144449894, i64 -4632014999006184066, i64 5965217035300479052, i64 -1950312264340299507, i64 7028699075841952857, i64 -2118145081435685808, i64 7732929879498976080, i64 -2440686039576745846, i64 2729016375227708858, i64 -1773382063398940906, i64 -4481836560336478371, i64 1632350669908838337, i64 7084023475166450463, i64 -501854443319824129, i64 5155323461508723669, i64 -6107108110881203417, i64 -4035945762212951455, i64 4702582145061644718, i64 7836613764833023140, i64 -5713684999371995556, i64 7168904772932410105, i64 -8407772787151669916, i64 -9043968315576718257, i64 575831687626421737, i64 -4746689431471600064, i64 4422919160224323186, i64 6896394212497609310, i64 -6023235691782087862, i64 3544420505210832169, i64 -757165909594063778, i64 6157679348720371052, i64 4329276571519906469, i64 -1990911523673070534, i64 5036102754559506630, i64 -2191363360671474517, i64 1482787149551272957, i64 -1373419086206088898, i64 -934854917431891576, i64 -5492754090006791928, i64 8350229320270587219, i64 4268375172736033224, i64 -796602798514079151, i64 -6769374299139659919, i64 -1373010339704555202, i64 558153512391010605, i64 -1373463267678034529, i64 -4993792621991958305, i64 -8136698012042370507, i64 7375566915343845934, i64 -7032064492354647287, i64 2309994301311950839, i64 5896332451797041072, i64 -6983004329932920911, i64 1248999508980307757, i64 -4080271877563871694, i64 3009413141735236470, i64 -4248797570944207931, i64 8965283840806038986, i64 5289540892740479427, i64 -5193669246061689962, i64 -1895981356203161417, i64 4824883659646149568, i64 -5395776397913141084, i64 6902288165270559218, i64 -4235384750046880033, i64 4358214375549315875, i64 8855320663930871481, i64 -5228889879685642492, i64 -8177087112836975401, i64 -8509539105103357001, i64 6791405731303370742, i64 7045870463469211631, i64 7265645750048097170, i64 -1989326044179505064, i64 842644652835471799, i64 -1209243922380387314, i64 4663582207556710239, i64 -4297577373777870279, i64 8572010250582765441, i64 5694556676159138567, i64 4325246712080838283, i64 5973938025963215036, i64 -4549796548320306971, i64 744006063439249200, i64 -3987929852594227464, i64 -9174510426046376719, i64 -9204916313285895378, i64 -5612381689621375715, i64 -6927524815102221286, i64 -4265815851944799383, i64 2842038318010242576, i64 3119464327141368028, i64 -1437912859693367903, i64 1724790742990198125, i64 -2634500535686466798, i64 8792529759216975699, i64 1569272213689604076, i64 -3159108859675335430, i64 -229522563114970394, i64 397749948349950634, i64 5254075025904804840, i64 -1574623705765770487, i64 -6461440890822362659, i64 -1902690733911459849, i64 7208278593579785208, i64 -3461575997545994449, i64 -3181962337129325145, i64 -2031077863010954079, i64 -5875527805943451459, i64 1764038071428550451, i64 -396811034289966075, i64 -6832544852387131717, i64 -3871910508913560060, i64 -9180933073447394271, i64 -3731965495647126210, i64 4892009338822403644, i64 6523133761360943673, i64 -5745846522624768805, i64 7039316585796341306, i64 -7925076473469592626, i64 304301083713162611, i64 -1726003528515103781, i64 8366753533561602845, i64 8499423597876631320, i64 -5915349716365289343, i64 670743902620961839, i64 117152394981318116, i64 -4596748290716725773, i64 643183389371842856, i64 1216274674189840258, i64 6960214111869039815, i64 -4674087121229360356, i64 3064757882241158812, i64 -7985864028943450751, i64 -8803724452429877900, i64 -1395847553786871300, i64 -506913173636909186, i64 1352462457279379936, i64 -4614000423561099304, i64 2692429588303683551, i64 590778048002822791, i64 8065811202012361522, i64 -3926238547747778193, i64 -3703204620423211382, i64 2325537118587936922, i64 4412208259846278127, i64 2674182280685328194, i64 -3556448767455036230, i64 740110568383034601, i64 8548263689898429283, i64 -2156809660319489262, i64 -5203113656580945669, i64 -7875436681552526434, i64 -2760485503838470592, i64 -4561440791965052204, i64 6847153886587852622, i64 4395931424043128496, i64 8051982645596938797, i64 9114103346183917613, i64 -3603557704364213317, i64 1361023414019642691, i64 4651532600415619418, i64 -188917078943289383, i64 2466794033634941590, i64 -5550756028277258522, i64 -2432298174156129787, i64 6079665906653085597, i64 -3672291795783990411, i64 6328323712919708203, i64 -8055878005998296236, i64 -736980122206200452, i64 1334221650246367013, i64 -8429141909534485000, i64 -6437022616367394509, i64 -4011533841562891425, i64 7621503169016694520, i64 9211741321464530849, i64 -7131965769529635043, i64 6717013612425610650, i64 3722262017551533379, i64 -4576935985765666311, i64 -3033465724920401499, i64 -6478674742726565411, i64 -8154268339576509978, i64 -7769549574953113127, i64 3387210867796038531, i64 2507967560012417870, i64 -1247180105410996559, i64 301665320386276874, i64 7527118991038156476, i64 -7212504450411186885, i64 812107540537404048, i64 4279076769938575332, i64 1855900901199596341, i64 -8918186086918363583, i64 -8627575265349799938, i64 8432043720557635438, i64 -1912616289121052187, i64 599249254373347417, i64 -2892806626906955292, i64 6210924363971745657, i64 8130580523857055241, i64 -8678247508345874459, i64 -5627724243553004782, i64 2582835091639302805, i64 -7564026074287861727, i64 3698273454159943285, i64 -2726081102499143337, i64 3059288435285902315, i64 -6534534383457269438, i64 8899023386900009782, i64 -6455256928074350630, i64 -7529823332178144024, i64 -2383015520372695668, i64 1742507054833257704, i64 796836558031481355, i64 -122432967421207188, i64 -8871558191444281228, i64 -648671418448029678, i64 8761271041399355254, i64 2792124057001418549, i64 5968329102244452616, i64 3046836771788934832, i64 4391340456624426369, i64 3755799554916927563, i64 2055180520412592797, i64 5358922620952506435, i64 -2853994812466153222, i64 -4804622404494944098, i64 -5966489838210891716, i64 382303836534174980, i64 -4962835142703549579, i64 -4701291683101222601, i64 -8363041113988098906, i64 -5376417822243558430, i64 -8274566214990846886, i64 -1867198030193307539, i64 211472808422052308, i64 7347925696341150050, i64 5933019625219978515, i64 -9023012376437751102, i64 -1436408721636598431, i64 5971071690419799642, i64 7693725524683606949, i64 7032948551735676304, i64 7908709191170345348, i64 2416714009132775305, i64 6625638287651413932, i64 1750089387193383490, i64 -3656503307877806725, i64 -3598293145631114598, i64 4031522693271552571, i64 -3393099874191663799, i64 -2010570594793218580, i64 -5023772131988982485, i64 5427782391517428886, i64 -5145666550416098296, i64 3116586420812233070, i64 230739682284057847, i64 -4259162158940380639, i64 -2970834648582253095, i64 -8434252398281588151, i64 9095718168838575148, i64 1384088122114487386, i64 3773237241368371796, i64 -7652966063229095980, i64 6462297343781231395, i64 937201822273646288, i64 -1158884115690577863, i64 -3342376738893994892, i64 -2273604973365266364, i64 -1560474774633098174, i64 1692952122579239971, i64 184359714222868709, i64 -309365223879338833, i64 1933467359789847024, i64 -640971213981679013, i64 8723990211693442802, i64 670171078209568691, i64 6912420285070815269, i64 4303323024682969035, i64 5736138603770770851, i64 -5262877591223626653, i64 -2547654171791459465, i64 -6616463295716430489, i64 1080569137902964073, i64 -8684934597820742356, i64 -3983904225748211854, i64 7110653435470262309, i64 -4693244782402927826, i64 -8501120947485443142, i64 -5101446184870564086, i64 3635961193419942593, i64 102718030572553533, i64 -1423737919127065363, i64 -7772602492280687265, i64 -7611163606675683648, i64 -690306556061814955, i64 7813042951506310582, i64 -8111081852120686975, i64 1365938911412771031, i64 3636590169657109720, i64 6409382201898883183, i64 -4870944638630089917, i64 -9139980777035232931, i64 2602517355716657296, i64 -7439806397794536628, i64 8553503726299699218, i64 -5091786526366009027, i64 -2125636854164061062, i64 8488467548376144412, i64 5679389459719617755, i64 1350015464551232360, i64 -7872846926078054696, i64 -9058859753328085545, i64 673218178257538019, i64 -2487638625268818213, i64 4757142894640062961, i64 1280138602624906766, i64 6080829326966341111, i64 -2542732066733528055, i64 -3110265988372465662, i64 -8953791556898154067, i64 -2078911740633181024, i64 -2367855365254725935, i64 807546176731624887, i64 -5132202980734562276, i64 3923714815476766592, i64 2686889910811702549, i64 1190475685085773412, i64 -2308461192295211672, i64 5010546142352432189, i64 315278089819409473, i64 3559085033948119228, i64 3472247027922132788, i64 1090242664305308992, i64 8588236694379566331, i64 -7925808974798549061, i64 3364664071889017954, i64 -8315534149344644702, i64 -2614827057333331821, i64 -8757391715379465147, i64 2940157161348293132, i64 4700816969116815431, i64 614875743604242768, i64 -8366479592547841752, i64 3849089266156179484, i64 -1997922981478515578, i64 -5958115928049182389, i64 8786508727283424069, i64 6062286338921725577, i64 4613036731537327303, i64 -8034303945419302114, i64 3731316916731282519, i64 2901455137669435169, i64 -4274437622571196360, i64 2903706934993736550, i64 -8464005246718795472, i64 -8533439052676595122, i64 -6999046525096417716, i64 7086713364988575823, i64 -3372849660781267533, i64 -927583605217194556, i64 925653851955406050, i64 8733827237186508157, i64 8766584848967848218, i64 1201330144236184786, i64 6981780234220077970, i64 8216296489322060855, i64 -7388005297839592307, i64 -6415376664175327998, i64 -1074144962197026873, i64 -4378955179571398791, i64 -5127046063938086042, i64 2614835365851746583, i64 6566887331428544206, i64 -3758234825488719111, i64 2467231093644774947, i64 -705880636484330689, i64 1331408398535435793, i64 -1339667037298254156, i64 4726544988650468219, i64 1222475658192142476, i64 720399641436579242, i64 -7618210528804912146, i64 -651312488252807224, i64 -3080030366367762312, i64 -7332760296205038750, i64 8711082985309150822, i64 1349006556173320450, i64 -8138974054301567751, i64 1076217018096796451, i64 5083343470793780767, i64 5033545687641433484, i64 7576856024681058576, i64 -8564973465042426736, i64 1191388338187409405, i64 -3937525751929359983, i64 5889674042402580700, i64 896120136848034077, i64 7086652431853918873, i64 4771749592914515472, i64 2485144842207504053, i64 2526993440738129298, i64 -7630746519056529742, i64 453044290019755672, i64 2905499225206113219, i64 -3813258872834426500, i64 3333654448977396556, i64 -468892976766599532, i64 -8747232747685926907, i64 6901791859825845989, i64 4920583640039371923, i64 6674670171888161448, i64 1234711259220578569, i64 5153980091604957292, i64 -1270079800499641925, i64 4948939057634217544, i64 -6101331425459595714, i64 -7495330666589437419, i64 -4817270796594145947, i64 -3021836353271613061, i64 1273923020020839119, i64 9019187012835731812, i64 -945786933566558748, i64 -1494045339341244424, i64 -2242235345949506745, i64 3178932590019401187, i64 3992752662928297707, i64 -3923003556205372122, i64 -4737369227404419853, i64 -2116520624382072823, i64 -3010200736185921149, i64 9024011871894659379, i64 7603418829576796799, i64 -100731690305837647, i64 7836545346124377448, i64 923006006798113989, i64 -1029706401341189429, i64 3238194155222033953, i64 8472976921703722160, i64 -2017970526279126521, i64 6501861390283988592, i64 1819586780781090609, i64 4070206393105340684, i64 5857418031118388361, i64 -2701487141826524824, i64 837170054483700235, i64 1072404152459706724, i64 -1686661073902909670, i64 -1974528588934277512, i64 950012773494931038, i64 7820872400735769507, i64 3639436339742018473, i64 -5609847349537023743, i64 -3033977117985329559, i64 5653251140709837853, i64 6209959975442813927, i64 8547831856592837739, i64 -443713087723303329, i64 -463246262695005611, i64 1773604810877053773, i64 -8696012050447719529, i64 2718152222132966499, i64 9218652638625590010, i64 8152343467591620186, i64 -8833229218106943099, i64 -5273150642435829548, i64 8170349360462493128, i64 -6577456158935463445, i64 8505661134549437066, i64 2937500007531061326, i64 -7618870774061338159, i64 7639710291748556869, i64 4487458090027242484, i64 1268553911830088621, i64 1456808567082772825, i64 -5725744795867211099, i64 500710096761331361, i64 2479011178276648013, i64 1981332792260420287, i64 7319353592692766122, i64 6993143358437796233, i64 1761034846576336083, i64 7406041153696077633, i64 -9087660309730280542, i64 1450679382142038105, i64 8716630447164130147, i64 3289073235981688900, i64 -7663051510453053250, i64 -1793634077445075000, i64 -5360664205012531235, i64 -3146192987356804754, i64 2146064663332729041, i64 -2277663989869519007, i64 -2878837520830229784, i64 -3592605418318788321, i64 -3096176210919214714, i64 7134254906934212892, i64 4549298733792471347, i64 -4722329936532925802, i64 -8366370209465439016, i64 8956491166888103164, i64 -2113211208491920877, i64 -835264948479628189, i64 -218147209530600254, i64 -8538931494663583820, i64 -1996695046260924211, i64 -8278405561150040349, i64 -5417740769580127396, i64 2410329894028483075, i64 -3117537731883144876, i64 2475461064582088735, i64 -5004490163374344677, i64 -2298319544478988270, i64 -2065458205376231401, i64 -6796925254170538966, i64 -8583276183521572031, i64 -6190889416391944062, i64 -6731874097289423891, i64 -9164001223259560052, i64 -3096121881210043210, i64 -3720300122893070538, i64 1626582857541454403, i64 -8070591618592824800, i64 6983829900070922179, i64 6433100506197631838, i64 3247410564521679481, i64 5759760873468964960, i64 2731723377338387493, i64 -8685686324793200158, i64 -6782117262223308528, i64 -4009762874289334341, i64 -7105324388174458236, i64 522194212828179716, i64 1334428854418356560, i64 4011537710884630777, i64 3154799111104601060, i64 5123247792913441303, i64 -5942441445438855953, i64 7645336916458344316, i64 -432983659080353965, i64 6411189624359328763, i64 7122120801295224295, i64 -8543110899511272692, i64 8316050039796964674, i64 -6161391330225903109, i64 -5537614781423042131, i64 5919969555286654683, i64 -5703290951038496897, i64 7975049682199063881, i64 -8004149483626176913, i64 2402871791449379183, i64 2302390232248911818, i64 -7813243305375866913, i64 6031781324912525782, i64 -3228289278837444278, i64 -2143589533796301797, i64 6167903599256653767, i64 4557158528984431897, i64 5490704209354457358, i64 -2211183906644827347, i64 3542439215214504277, i64 -3781255045290889999, i64 2019784790425623601, i64 -7268155485631715056, i64 -2301370630012263699, i64 6740591542244914094, i64 5087644621297519428, i64 5589297589855410795, i64 1887361416685228404, i64 3941279422581730554, i64 5100665975655976974, i64 -994079931142221030, i64 -695636014369555943, i64 -870791836892459017, i64 -8732172334559746859, i64 -7710878284948335785, i64 1680755550556888315, i64 6159248426532292817, i64 -5609891063797583305, i64 -4687753882390876603, i64 -4478948221608512369, i64 -3525909706513233480, i64 -5225508940885944875, i64 146606175866077672, i64 -6811028697600920963, i64 7532272711380377088, i64 3111811264904584113, i64 3122449538967941218, i64 -1240120885414719831, i64 -7321074745400160935, i64 3215199296426295406, i64 9050251601440067963, i64 5430599211834649507, i64 915829528948077775, i64 599839161403513493, i64 5877027788998364509, i64 -3317295653616379941, i64 -4636703329901157529, i64 7870012232483574633, i64 4628845584152391982, i64 -4073630535129409960, i64 3489243717252283271, i64 -9016537121936939958, i64 2055435275202404341, i64 5927868036410741486, i64 -347941106171941815, i64 -2562115853784379939, i64 -2319250594197255326, i64 -7568692717595147059, i64 -8530961885464593799, i64 -7308580322644177617, i64 -4502519101447901275, i64 -6616273222827137381, i64 8904547981367418687, i64 -5159526408781249706, i64 8596503602454203573, i64 -6163054246786469468, i64 -2971091292911421215, i64 -2329569215784517051, i64 2234436926480353801, i64 -6295219000041896760, i64 -2291712090273871057, i64 -7476188529100192971, i64 8455807834491769040, i64 7748070376456717979, i64 -5616316880182021477, i64 -6425601802034539077, i64 -5520712022319616945, i64 6666005061572248100, i64 -2852618366495049493, i64 3790320272073795891, i64 5304892129217386988, i64 -1844319264777358372, i64 1706784606986125946, i64 -5370755433529360559, i64 939438625652355310, i64 8773410578015552668, i64 -4086470043336161046, i64 9212716331840362372, i64 1271959021592296350, i64 1891429451139333669, i64 8807471665288259901, i64 5897028636072748843, i64 -5900873096353234807, i64 -9158367094884282619, i64 238975179102143513, i64 1391374800763696057, i64 5309476137557946960, i64 8784726369196095146, i64 5729503543762652014, i64 -8955930427015278477, i64 -3222757611624826202, i64 -163213123380435684, i64 3990908583078933372, i64 3118299658784183715, i64 -1934294817751505211, i64 -6248509997688539892, i64 3730297451470039723, i64 3956996355093450341, i64 -1725332974055556286, i64 5309470380235752116, i64 -432990528043990351, i64 -6538114310568531370, i64 1671311699050390676, i64 8051799323721902625, i64 8160577044350990926, i64 7422205495965586582, i64 -2891970068870247614, i64 1323243310325278191, i64 -6661516082018368418, i64 5593637664106958296, i64 3757700906337719127, i64 1321633637505957632, i64 -4745473224193689366, i64 -5006643686895698832, i64 -7860796611854823061, i64 -3492235040212838680, i64 5097615470064051842, i64 -8054118444365309657, i64 8924997099515394480, i64 -8927453403005045088, i64 2662849788324655450, i64 8667210962023306259, i64 3073572101305754274, i64 -3801512167530540346, i64 -1589055604770139311, i64 -1214204422783019818, i64 -619426535656807709, i64 -1934191992650816232, i64 -476810113007578037, i64 5579008939497906801, i64 2704357232432866549, i64 -5321694764829426373, i64 282778254819205977, i64 8610741359599367572, i64 4534507720106560387, i64 1572333629853082801, i64 -2716095856210921855, i64 -1251625561655719279, i64 4321910188333638542, i64 -153698677800201586, i64 2414809526006830590, i64 1418393110921682339, i64 1611017854517916406, i64 8313093165458008421, i64 1590901415493673707, i64 7083003526600156288, i64 9133588974563112411, i64 9186776439479315027, i64 35881193045019827, i64 -4281190563046789199, i64 1483308623786628455, i64 -3822957010519849299, i64 5811689635670692516, i64 84360773980875713, i64 5731160717263015314, i64 6709786177575724704, i64 -4631323641282852523, i64 1426486760628998625, i64 3234887571132720584, i64 1554311102927700884, i64 4992696561624843795, i64 -8558427621898782168, i64 -3951350925185144822, i64 -2815443360124069299, i64 4224489236777608456, i64 -4812769308922566089, i64 -2742283565430452070, i64 -2096032427233422126, i64 7533623286518745083, i64 -1328553362349538766, i64 -2754590475329566246, i64 -3076263745537215346, i64 6024798337755745748, i64 4771035473911110553, i64 6531646918392073737, i64 -7712469296294448707, i64 -789578150828815190, i64 689530210173813042, i64 2278221598420254068, i64 5373512515604634688, i64 -901514190929939486, i64 -7455068056903617284, i64 -1540976133114356721, i64 1294184552129220825, i64 2646197720042641813, i64 8860423399187635126, i64 -3124175041297779630, i64 8044084055492154767, i64 -4050762929022711560, i64 2766727828058608189, i64 3264172118617440499, i64 4096203138409840801, i64 9101383886271208842, i64 1892685688464320798, i64 -7843547909919784905, i64 1836761262084303862, i64 -7875593091289222899, i64 7976469539918302936, i64 -5498251536501371793, i64 -5061438883570998180, i64 -8894331322459798366, i64 4771127721241779101, i64 7275253087376945952, i64 -98035756345786442, i64 -2824450553962687724, i64 3518133032632241801, i64 7500214597202925512, i64 -7710597188687932127, i64 8483260253909550012, i64 4368817398638987602, i64 6718611786267174982, i64 1054542971979661186, i64 3345129292181988172, i64 3996657159068887111, i64 -535175989157492432, i64 -8627112730701976972, i64 634150362782304458, i64 -5490018609846027269, i64 8182355111193564194, i64 -4565320476414768854, i64 4192989253549960947, i64 -4324622956009268553, i64 6673066883026529581, i64 3365642975530622738, i64 -2820609429668264739, i64 4216133619238866720, i64 3597629176401169827, i64 4734011972141469065, i64 -1746104973068662490, i64 2566241885778219336, i64 -5844896019878054291, i64 360578950799444562, i64 -2389992185815492990, i64 7017109250161581448, i64 -3423166642783871333, i64 3501812460940776393, i64 1403658406644977611, i64 -5255809894746081635, i64 8955779854506082200, i64 5535869135820582541, i64 -973822663598815726, i64 -6416606514066710252, i64 -8611518535653576131, i64 -1328152540002446767, i64 -4296521796364621056, i64 -5795934659564041942, i64 2967937394407093520, i64 -7545755181988814555, i64 1840302455171354902, i64 3183618014563364921, i64 -925230639944000523, i64 3032103184553354071, i64 5507585372499734676, i64 5910316143441627459, i64 -4248944874836431451, i64 -6481615510410351636, i64 8659923023911269406, i64 6083967405188710846, i64 -7530569392799889036, i64 -4629304259419106730, i64 8872323092018703594, i64 -6190771989665730997, i64 -2663724181909966156, i64 -9218622384378668802, i64 -7373699247266533854, i64 7980530112136857896, i64 333582506449919423, i64 -3663197406489419017, i64 -7321574856390199667, i64 -8089915656417623691, i64 -7713763831629108208, i64 1647516077385492953, i64 6174597970973303969, i64 7795058893535042835, i64 9109482743731816642, i64 -5060108732511310262, i64 -6048136384001594021, i64 5354177034710600976, i64 2590243128879348065, i64 2049315098475444301, i64 -491820601083400863, i64 1847675112439703185, i64 -5707734965896702973, i64 -4441379986383872121, i64 324086452863033651, i64 1308818321287991940, i64 2834385324716467533, i64 -3147242245460344757, i64 -5721171812377808906, i64 -6850772358059600756, i64 -1420504851769585047, i64 7290206826309783753, i64 7033428695348171989, i64 8596426334053489819, i64 8292748846193063054, i64 4537176410100443029, i64 -1758097294059052155, i64 4808069033230769780, i64 -5819551122310169644, i64 4288613470397978229, i64 -9188241323626043963, i64 7581411516737586386, i64 6239532205554540673, i64 -6090776081288987886, i64 2890655621975844857, i64 -4960411529626118345, i64 140732571601869461, i64 2713890920905037302, i64 -2612200504590864911, i64 -3932245400936855189, i64 8994578045034355139, i64 4229864589681734907, i64 -8305811005125867301, i64 8098747866792530520, i64 6023236373597108646, i64 -4641288061398506417, i64 -4812977256767604335, i64 2217059806098040624, i64 -1954440554961491690, i64 -898577407381556564, i64 -4647165576833462493, i64 -8000110083733243494, i64 -2496213194148234837, i64 -1947992193975989356, i64 -7942682001008787936, i64 -310853162871375449, i64 4365720081986124066, i64 -2003387036598148403, i64 -4661390502984019673, i64 -2300429380222258076, i64 5355666685326329628, i64 -4659221073423790234, i64 -2205492867438416355, i64 -8216748918083849738, i64 3206830066160037392, i64 8571857254767118135, i64 -2026124182463703919, i64 -8684574231183666670, i64 5221892196904019620, i64 4819851957638881074, i64 898724795912959177, i64 -664186148974107471, i64 3682737836801639235, i64 805068668503622970, i64 -3251230434439491329, i64 4547028544747022483, i64 -3931744571572514167, i64 -2834268549527399803, i64 1619151008196151962, i64 1757506944178828067, i64 808287851322096921, i64 2676212342983824495, i64 -5538015970828731121, i64 4681039503104559623, i64 -7036200827223814711, i64 -417957945942546460, i64 -8926654670463214422, i64 1294019411927178284, i64 3651808111405461578, i64 1940328145586270202, i64 3417802402637725921, i64 -6944976436521839102, i64 5832287655474974112, i64 3851367263887604990, i64 -3928755603181454139, i64 -2011987608606671380, i64 549603697004021084, i64 7874568845737488543, i64 4843422965425084956, i64 7479345777168551182, i64 -3928700123774160042, i64 4149652723279055667, i64 -2702764013390803788, i64 -2478907772490973443, i64 -5793550738080864144, i64 4742518364904671260, i64 3384897762342711086, i64 690459445909425393, i64 -1723956442549856799, i64 -2925572225977283856, i64 -2395999878781649765, i64 -7371209765678285281, i64 -1809144065486589859, i64 -1272847593099971763, i64 -8369434495684254252, i64 -79024508114029209, i64 -4391875161252458013, i64 3316311587925841524, i64 -9032275185278071949, i64 6105749738811678205, i64 -7773008743187047414, i64 -1237479209474135839, i64 -1577939898160251154, i64 -3267973022980317939, i64 -3109402258394717414, i64 1343743781034175533, i64 -7773019188608312646, i64 -5572151840564506911, i64 -5959341895287482911, i64 6511985943530973507, i64 -6958312937541676928, i64 3684076947940173477, i64 6297804494979839771, i64 6171016775580208277, i64 -7508460312624751694, i64 -2275178734232002534, i64 878166964288875536, i64 -7236601391618754658, i64 -7519082485250536985, i64 3295875800894191564, i64 -8079993353947168105, i64 5929033753193199502, i64 3414376757798422884, i64 -3704555145555722122, i64 -2209748942067130018, i64 -6916693456336702676, i64 -7207678266661507403, i64 -4622642777427671093, i64 -4343046710544785149, i64 4223493222084692892, i64 7238827432266236202, i64 2554866521618836210, i64 2378876804535724851, i64 -1297715697449729110, i64 -8797853495898677939, i64 -7719228989493628064, i64 3686333498927972218, i64 -1019500553120860765, i64 -3698309829721635905, i64 -7769744110064456778, i64 3945178015360351890, i64 -8180364254312080212, i64 -29392017175106346, i64 -5090424878389926597, i64 4809424496301355993, i64 2226743929557997339, i64 -1516531043939542126, i64 5926284923427264870, i64 3915973785548149146, i64 5815915729351300371, i64 -773825724539628511, i64 7400714941731036643, i64 -7823657062453669226, i64 -8740692006784174902, i64 5346376607773360542, i64 5250121617522557572, i64 4538187340782611705, i64 8410320219679426698, i64 6058496389771214631, i64 -6282916177513464630, i64 -2840867019723731630, i64 -7623229728331152714, i64 -3748423798556560151, i64 8849780193917868985, i64 3081345090283684566, i64 2504894111211345180, i64 -237959866274270555, i64 -7285965247635564922, i64 2122363358847413766, i64 -6291309477030348269, i64 -5870869960456779705, i64 7490239831949886051, i64 7489677184471998224, i64 2717549039527625772, i64 -4259967792627616203, i64 4063294220313264925, i64 -8766526452238238884, i64 8580349413208913683, i64 -5253580602572100084, i64 208528141065812661, i64 754128412403281353, i64 -5164190382637202041, i64 7182478828431946198, i64 1747488626052255462, i64 -8226480545076190814, i64 2957952056607549651, i64 4548539817174076331, i64 4664142982720053674, i64 -6538873515287467817, i64 -2038660881884190028, i64 7382726265451359804, i64 -7551281314411037382, i64 9111128509593826930, i64 2129411896480678614, i64 -1410412725260349927, i64 -6625139390906291796, i64 7596120921244876588, i64 1862076942428452167, i64 4246197864424598454, i64 -6579645403431623293, i64 -4808025006423689587, i64 6011550940584554679, i64 -4986968118159233877, i64 -4986923306605170002, i64 -4986968118148549970, i64 1969995459132830389, i64 5642873044026796913, i64 -5829184524531600337, i64 -9108111672686513439, i64 -1194033290332934846, i64 4023432121975903285, i64 -2379843498829141911, i64 425894904218867378, i64 -2926229599880514094, i64 -1413325618783997448, i64 -2021642584175001830, i64 5626521264027465946, i64 5626532117462584003, i64 -7469830342626278051, i64 -4074533333532641467, i64 539532647873402324, i64 -8386414619820774565, i64 2061961557470813341, i64 -5602136214235996785, i64 3090532741297941698, i64 6498055134890217927, i64 1903292790790801385, i64 5734580685472095062, i64 6097116695364588701, i64 -6518412206949129369, i64 6501690820086000250, i64 -2974359450521259559, i64 -1248700111359067475, i64 8204093606214456796, i64 -6072579231854183526, i64 2208437122029239534, i64 -5491958138999425377, i64 -5171812772712735015, i64 1130235285887992033, i64 -4636073949331136097, i64 -6209133800715179327, i64 2839140593036405209, i64 -68939708370387434, i64 -3836574829426136620, i64 -342302789792510533, i64 -1540205253452081220, i64 -1460103411290686518, i64 -4338449800402841814, i64 7182708215645741095, i64 3065470496841253379, i64 -8041633660725549280, i64 -895513110546407688, i64 6604823071406264015, i64 208778088491689006, i64 -3598363362790983941, i64 4123274503795521473, i64 844141633685530709, i64 8483845115396101259, i64 -4271855350361846709, i64 -7924041773602382520, i64 -4880747436930299139, i64 -5366391542559962251, i64 7377085452562711391, i64 5731470671436773520, i64 3940458166146956914, i64 2552121607483806802, i64 -4286848573529998011, i64 -4304980261507824790, i64 4808100690656990331, i64 5293342098818908045, i64 -311902602667649206, i64 8164949830031812372, i64 -312501822440522006, i64 -538122378069654764, i64 391472587576327914, i64 -3696317027340383517, i64 -8085564600619290336, i64 -2499715533738070734, i64 -3567461186990162713, i64 1295017046630610681, i64 -7434452303120033633, i64 4653219630365748174, i64 8551450009237851339, i64 -7797291523375827726, i64 -6561045714336155810, i64 -4758581958073041449, i64 -8790980076315588575, i64 8522793156315500321, i64 -7985483723510172923, i64 -7076728779012768526, i64 4848818935207530706, i64 -844387652130663186, i64 727863721440134760, i64 3118665356994324234, i64 -7453344846474232440, i64 2827314769455839798, i64 -3214005961942050458, i64 -6428491159601255247, i64 5822909739907357027, i64 6227548868319197264, i64 -164846959136725610, i64 -5538843371153709294, i64 -2463982779695944227, i64 -9059133162646980430, i64 1611085003812686112, i64 -5307653026031978413, i64 460570811484785389, i64 -7170941632129831543, i64 -4927640103978510867, i64 6943347588270552581, i64 -7829026052487221773, i64 -8267859312609403446, i64 -5488843036363147411, i64 -5080346887140990411, i64 4318241257452469653, i64 2159151595446042038, i64 5390159902067829979, i64 7644242548681995860, i64 -6945198515088393364, i64 -1480493085163318867, i64 -6784218212467860322, i64 -3393024132492543389, i64 -5993414731682362063, i64 6689493121489757533, i64 8416117855425447585, i64 4206997127599857562, i64 5006807883752036238, i64 -6505605565980858649, i64 7235435070376229777, i64 -3937956258225408170, i64 -6821594962969829942, i64 -4524225905325701930, i64 -3453326274405241612, i64 4377335459542032233, i64 3086390735575370448, i64 2499297443883853602, i64 2343513918404285042, i64 -2907947071312459087, i64 -1634395685005199090, i64 3190381150375955659, i64 -7150953572831287168, i64 -4957606848626141517, i64 6168467928189768245, i64 -7313520696723940658, i64 1957236926849617745, i64 6103121800126281238, i64 -3118034508131248605, i64 7689942741875073570, i64 3812350086554391628, i64 -4004237505265810202, i64 4234626323340912261, i64 8435548469804017805, i64 8188151299926188868, i64 6755431056647503776, i64 6955932608810473411, i64 -5090121537569978646, i64 -4313722649733649148, i64 -992813794205518814, i64 -2977121739881177750, i64 8300702148807767455, i64 8688011364438049532, i64 -3060148919399465192, i64 -1706636374142479311, i64 -6041464957803230695, i64 8768004117370349836, i64 999413708106512669, i64 -5746766090519830361, i64 6548990594435268450, i64 5285773201321744619, i64 2482387847093425257, i64 -90453418024018371, i64 -361800984449536087, i64 -1487736334444481113, i64 -2952427951304869217, i64 -1993000209731304335, i64 4759941848098733067, i64 3147534050242044406, i64 1306237926154796312, i64 6333915842410333849, i64 7659701902773449098, i64 -5451996640186096003, i64 -4642461111295127438, i64 7280336413357241886, i64 -7240038908606010812, i64 -5080062406817670950, i64 9172407607839099131, i64 5152715037028495838, i64 -1595863175104031630, i64 71870954726383725, i64 -6663612153666139953, i64 -5780377822070833382, i64 -2558049005459743801, i64 -4036070038453276705, i64 -4579300870303961109, i64 2296366595518995665, i64 8023829834775273295, i64 5749038009903209537, i64 -540580108163145755, i64 8717530716781375244, i64 -6379718729556656265, i64 -154182621599893745, i64 -5246033660072849377, i64 -7292666866626222156, i64 1588612511653036025, i64 4707776551906258383, i64 -34187341100826984, i64 -3040468458664195375, i64 -5192743583486402036, i64 -9126708828044498260, i64 -7503741593738799742, i64 -470120406029649965, i64 6804933398549553075, i64 -2598972932694642726, i64 -1621524757755354555, i64 1835709002170865563, i64 -553083216992218985, i64 6801758402532763110, i64 8500409876800883848, i64 49766144641659031, i64 -1112192153484899379, i64 -4096634483079390968, i64 -6736803487708974780, i64 9223267588808268296, i64 -6896369194589418688, i64 -2880223413391062178, i64 3525967274053747510, i64 3906647371355904516, i64 629101740686586167, i64 941375959552], align 8

define i64 @df.main$2edart.WindowsBackend.name(i64 %v0) nounwind {
b0:
  %area = call ptr @df.get_area()
  %v1 = ptrtoint ptr getelementptr inbounds (i8, ptr @"df.s.646f238b2717745bee5c48412802f4ff", i64 2) to i64
  ret i64 %v1
}

define void @df.main$2edart.WindowsBackend.new(i64 %v0) nounwind {
b0:
  ret void
}

define i64 @df.main$2edart.MacBackend.name(i64 %v0) nounwind {
b0:
  %area = call ptr @df.get_area()
  %v1 = ptrtoint ptr getelementptr inbounds (i8, ptr @"df.s.24d4827748980086fff04bcb8934715f", i64 2) to i64
  ret i64 %v1
}

define void @df.main$2edart.MacBackend.new(i64 %v0) nounwind {
b0:
  ret void
}

define i64 @df.main$2edart.LinuxBackend.name(i64 %v0) nounwind {
b0:
  %area = call ptr @df.get_area()
  %v1 = ptrtoint ptr getelementptr inbounds (i8, ptr @"df.s.1566253b2ae2096e6cb28f3c26be63ff", i64 2) to i64
  ret i64 %v1
}

define void @df.main$2edart.LinuxBackend.new(i64 %v0) nounwind {
b0:
  ret void
}

define i64 @df.main$2edart..defaultBackends(i64 %v0) nounwind {
b0:
  %list_buf_9 = alloca [1 x i64], align 8
  %list_tags_9 = alloca [1 x i8], align 1
  %gcq = alloca { ptr, i64, [3 x i64] }, align 8
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
  ret i64 0
pilha.ok:
  store { ptr, i64, [3 x i64] } { ptr null, i64 3, [3 x i64] zeroinitializer }, ptr %gcq
  %gcs0 = getelementptr inbounds { ptr, i64, [3 x i64] }, ptr %gcq, i64 0, i32 2, i64 0
  %gcs1 = getelementptr inbounds { ptr, i64, [3 x i64] }, ptr %gcq, i64 0, i32 2, i64 1
  %gcs2 = getelementptr inbounds { ptr, i64, [3 x i64] }, ptr %gcq, i64 0, i32 2, i64 2
  %gcant = load ptr, ptr %ctxtopo, align 8
  store ptr %gcant, ptr %gcq, align 8
  store ptr %gcq, ptr %ctxtopo, align 8
  %v5 = ptrtoint ptr getelementptr inbounds (i8, ptr @"df.s.a9d599a6f00817a79ce41246096a66cc", i64 2) to i64
  store i64 %v5, ptr %gcs0
  %v9 = call i64 @dartforge_list_new(ptr %list_buf_9, i64 0, i64 10, i64 0)
  store i64 %v9, ptr %gcs1
  %v11 = ptrtoint ptr getelementptr inbounds (i8, ptr @"df.s.d2024387aa91ee92eeb44f2f09100c37", i64 2) to i64
  %v12 = call i1 @df.string_equals_object(i64 %v5, i64 %v11)
  br i1 %v12, label %b6, label %b4
b4:
  br label %b5
b5:
  %v22 = ptrtoint ptr getelementptr inbounds (i8, ptr @"df.s.a9d599a6f00817a79ce41246096a66cc", i64 2) to i64
  %v23 = call i1 @df.string_equals_object(i64 %v5, i64 %v22)
  br i1 %v23, label %b12, label %b10
b6:
  %v13 = call i64 @dartforge_tearoff(i64 ptrtoint (ptr @df.main$2edart.WindowsBackend.new$tear to i64))
  store i64 %v13, ptr %gcs2
  %ga14 = getelementptr i64, ptr %area, i64 1
  %v14 = load i8, ptr %ga14
  %c0 = zext i8 %v14 to i64
  %v15 = icmp ne i64 %c0, 0
  br i1 %v15, label %b7, label %b8
b7:
  %ga16 = getelementptr i64, ptr %area, i64 0
  %v16 = load i64, ptr %ga16
  br label %b9
b8:
  %v17 = call i64 @df.rti.c4ef71524a64e946.main$2edart()
  br label %b9
b9:
  %v18 = phi i64 [ %v16, %b7 ], [ %v17, %b8 ]
  call void @dartforge_rti_set(i64 %v13, i64 %v18)
  %v20 = call i64 @dartforge_list_append(i64 %v9, i64 %v13)
  br label %b5
b10:
  br label %b11
b11:
  %v33 = ptrtoint ptr getelementptr inbounds (i8, ptr @"df.s.1591faa1157de847b0cebd0d94681d83", i64 2) to i64
  %v34 = call i1 @df.string_equals_object(i64 %v5, i64 %v33)
  br i1 %v34, label %b18, label %b16
b12:
  store i64 0, ptr %gcs2
  %v24 = call i64 @dartforge_tearoff(i64 ptrtoint (ptr @df.main$2edart.MacBackend.new$tear to i64))
  store i64 %v24, ptr %gcs2
  %ga25 = getelementptr i64, ptr %area, i64 3
  %v25 = load i8, ptr %ga25
  %c1 = zext i8 %v25 to i64
  %v26 = icmp ne i64 %c1, 0
  br i1 %v26, label %b13, label %b14
b13:
  %ga27 = getelementptr i64, ptr %area, i64 2
  %v27 = load i64, ptr %ga27
  br label %b15
b14:
  %v28 = call i64 @df.rti.179043ae53de0e53.main$2edart()
  br label %b15
b15:
  %v29 = phi i64 [ %v27, %b13 ], [ %v28, %b14 ]
  call void @dartforge_rti_set(i64 %v24, i64 %v29)
  %v31 = call i64 @dartforge_list_append(i64 %v9, i64 %v24)
  br label %b11
b16:
  br label %b17
b17:
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs2
  %ga43 = getelementptr i64, ptr %area, i64 7
  %v43 = load i8, ptr %ga43
  %c2 = zext i8 %v43 to i64
  %v44 = icmp ne i64 %c2, 0
  br i1 %v44, label %b22, label %b23
b18:
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs2
  %v35 = call i64 @dartforge_tearoff(i64 ptrtoint (ptr @df.main$2edart.LinuxBackend.new$tear to i64))
  store i64 %v35, ptr %gcs0
  %ga36 = getelementptr i64, ptr %area, i64 5
  %v36 = load i8, ptr %ga36
  %c3 = zext i8 %v36 to i64
  %v37 = icmp ne i64 %c3, 0
  br i1 %v37, label %b19, label %b20
b19:
  %ga38 = getelementptr i64, ptr %area, i64 4
  %v38 = load i64, ptr %ga38
  br label %b21
b20:
  %v39 = call i64 @df.rti.7da4007f3d892a74.main$2edart()
  br label %b21
b21:
  %v40 = phi i64 [ %v38, %b19 ], [ %v39, %b20 ]
  call void @dartforge_rti_set(i64 %v35, i64 %v40)
  %v42 = call i64 @dartforge_list_append(i64 %v9, i64 %v35)
  br label %b17
b22:
  %ga45 = getelementptr i64, ptr %area, i64 6
  %v45 = load i64, ptr %ga45
  br label %b24
b23:
  %v46 = call i64 @df.rti.784f01e5bf36a4ef.main$2edart()
  br label %b24
b24:
  %v47 = phi i64 [ %v45, %b22 ], [ %v46, %b23 ]
  call void @dartforge_rti_set(i64 %v9, i64 %v47)
  %gcvolta24 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta24, ptr %ctxtopo, align 8
  ret i64 %v9
}

define linkonce_odr i64 @df.main$2edart.WindowsBackend.new$tear(i64 %v0, ptr %v1, ptr %v2) nounwind {
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
  %ta10 = getelementptr inbounds i8, ptr %ctx, i64 80
  %tb10 = load ptr, ptr %ta10, align 8
  %tfa10 = getelementptr inbounds i8, ptr %ctx, i64 88
  %tf10 = load ptr, ptr %tfa10, align 8
  %tnx10 = getelementptr i8, ptr %tb10, i64 24
  %tz10 = icmp ugt ptr %tnx10, %tf10
  %tx10 = call i1 @llvm.expect.i1(i1 %tz10, i1 false)
  br i1 %tx10, label %ao10.lento, label %ao10.rapido
ao10.rapido:
  store ptr %tnx10, ptr %ta10, align 8
  store i64 4410931412993, ptr %tb10, align 8
  %thb10 = ptrtoint ptr %tb10 to i64
  %th10 = add i64 %thb10, 2
  br label %ao10.fim
ao10.lento:
  %tl10 = call i64 @dartforge_object_new_t(i64 1027, i64 0, ptr @df.mt.main$2edart.WindowsBackend)
  br label %ao10.fim
ao10.fim:
  %v10 = phi i64 [ %th10, %ao10.rapido ], [ %tl10, %ao10.lento ]
  ret i64 %v10
b2:
  call void @dartforge_nsm_call()
  ret i64 0
}

define i64 @df.rti.c4ef71524a64e946.main$2edart() nounwind {
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
  %v3 = ptrtoint ptr getelementptr inbounds (i8, ptr @"df.s.a74ddd44d1f897095271484348e043a5", i64 2) to i64
  %v4 = call i64 @dartforge_rti_recipe(i64 %v3)
  %ga5 = getelementptr i64, ptr %area, i64 0
  store i64 %v4, ptr %ga5
  %ga6 = getelementptr i64, ptr %area, i64 1
  store i8 1, ptr %ga6
  ret i64 %v4
}

define linkonce_odr i64 @df.main$2edart.MacBackend.new$tear(i64 %v0, ptr %v1, ptr %v2) nounwind {
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
  %ta10 = getelementptr inbounds i8, ptr %ctx, i64 80
  %tb10 = load ptr, ptr %ta10, align 8
  %tfa10 = getelementptr inbounds i8, ptr %ctx, i64 88
  %tf10 = load ptr, ptr %tfa10, align 8
  %tnx10 = getelementptr i8, ptr %tb10, i64 24
  %tz10 = icmp ugt ptr %tnx10, %tf10
  %tx10 = call i1 @llvm.expect.i1(i1 %tz10, i1 false)
  br i1 %tx10, label %ao10.lento, label %ao10.rapido
ao10.rapido:
  store ptr %tnx10, ptr %ta10, align 8
  store i64 4406636445697, ptr %tb10, align 8
  %thb10 = ptrtoint ptr %tb10 to i64
  %th10 = add i64 %thb10, 2
  br label %ao10.fim
ao10.lento:
  %tl10 = call i64 @dartforge_object_new_t(i64 1026, i64 0, ptr @df.mt.main$2edart.MacBackend)
  br label %ao10.fim
ao10.fim:
  %v10 = phi i64 [ %th10, %ao10.rapido ], [ %tl10, %ao10.lento ]
  ret i64 %v10
b2:
  call void @dartforge_nsm_call()
  ret i64 0
}

define i64 @df.rti.179043ae53de0e53.main$2edart() nounwind {
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
  %v3 = ptrtoint ptr getelementptr inbounds (i8, ptr @"df.s.092312ceffedb5287ea8c500f2c49c7c", i64 2) to i64
  %v4 = call i64 @dartforge_rti_recipe(i64 %v3)
  %ga5 = getelementptr i64, ptr %area, i64 2
  store i64 %v4, ptr %ga5
  %ga6 = getelementptr i64, ptr %area, i64 3
  store i8 1, ptr %ga6
  ret i64 %v4
}

define linkonce_odr i64 @df.main$2edart.LinuxBackend.new$tear(i64 %v0, ptr %v1, ptr %v2) nounwind {
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
  %ta10 = getelementptr inbounds i8, ptr %ctx, i64 80
  %tb10 = load ptr, ptr %ta10, align 8
  %tfa10 = getelementptr inbounds i8, ptr %ctx, i64 88
  %tf10 = load ptr, ptr %tfa10, align 8
  %tnx10 = getelementptr i8, ptr %tb10, i64 24
  %tz10 = icmp ugt ptr %tnx10, %tf10
  %tx10 = call i1 @llvm.expect.i1(i1 %tz10, i1 false)
  br i1 %tx10, label %ao10.lento, label %ao10.rapido
ao10.rapido:
  store ptr %tnx10, ptr %ta10, align 8
  store i64 4402341478401, ptr %tb10, align 8
  %thb10 = ptrtoint ptr %tb10 to i64
  %th10 = add i64 %thb10, 2
  br label %ao10.fim
ao10.lento:
  %tl10 = call i64 @dartforge_object_new_t(i64 1025, i64 0, ptr @df.mt.main$2edart.LinuxBackend)
  br label %ao10.fim
ao10.fim:
  %v10 = phi i64 [ %th10, %ao10.rapido ], [ %tl10, %ao10.lento ]
  ret i64 %v10
b2:
  call void @dartforge_nsm_call()
  ret i64 0
}

define i64 @df.rti.7da4007f3d892a74.main$2edart() nounwind {
b0:
  %area = call ptr @df.get_area()
  %ga0 = getelementptr i64, ptr %area, i64 5
  %v0 = load i8, ptr %ga0
  %c0 = zext i8 %v0 to i64
  %v1 = icmp ne i64 %c0, 0
  br i1 %v1, label %b1, label %b2
b1:
  %ga2 = getelementptr i64, ptr %area, i64 4
  %v2 = load i64, ptr %ga2
  ret i64 %v2
b2:
  %v3 = ptrtoint ptr getelementptr inbounds (i8, ptr @"df.s.107562bc5fad35223e473f09a4a0180c", i64 2) to i64
  %v4 = call i64 @dartforge_rti_recipe(i64 %v3)
  %ga5 = getelementptr i64, ptr %area, i64 4
  store i64 %v4, ptr %ga5
  %ga6 = getelementptr i64, ptr %area, i64 5
  store i8 1, ptr %ga6
  ret i64 %v4
}

define i64 @df.rti.784f01e5bf36a4ef.main$2edart() nounwind {
b0:
  %area = call ptr @df.get_area()
  %ga0 = getelementptr i64, ptr %area, i64 7
  %v0 = load i8, ptr %ga0
  %c0 = zext i8 %v0 to i64
  %v1 = icmp ne i64 %c0, 0
  br i1 %v1, label %b1, label %b2
b1:
  %ga2 = getelementptr i64, ptr %area, i64 6
  %v2 = load i64, ptr %ga2
  ret i64 %v2
b2:
  %v3 = ptrtoint ptr getelementptr inbounds (i8, ptr @"df.s.24d14e5277ea3e91174e98c49002b4cf", i64 2) to i64
  %v4 = call i64 @dartforge_rti_recipe(i64 %v3)
  %ga5 = getelementptr i64, ptr %area, i64 6
  store i64 %v4, ptr %ga5
  %ga6 = getelementptr i64, ptr %area, i64 7
  store i8 1, ptr %ga6
  ret i64 %v4
}

define i64 @df.main$2edart..separator(i64 %v0) nounwind {
b0:
  %area = call ptr @df.get_area()
  %v5 = ptrtoint ptr getelementptr inbounds (i8, ptr @"df.s.34de1dc3712a2fb06691d5373a77308b", i64 2) to i64
  ret i64 %v5
}

define void @dart_main() nounwind {
b0:
  %sargs75 = alloca [1 x i64]
  %sargs39 = alloca [2 x i64]
  %cargs57 = alloca [1 x i64]
  %sargs78 = alloca [1 x i64]
  %sargs89 = alloca [1 x i64]
  %cargs104 = alloca [1 x i64]
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
  store { ptr, i64, [2 x i64] } { ptr null, i64 2, [2 x i64] zeroinitializer }, ptr %gcq
  %gcs0 = getelementptr inbounds { ptr, i64, [2 x i64] }, ptr %gcq, i64 0, i32 2, i64 0
  %gcs1 = getelementptr inbounds { ptr, i64, [2 x i64] }, ptr %gcq, i64 0, i32 2, i64 1
  %gcant = load ptr, ptr %ctxtopo, align 8
  store ptr %gcant, ptr %gcq, align 8
  store ptr %gcq, ptr %ctxtopo, align 8
  %v0 = call i64 @df.main$2edart..defaultBackends(i64 0)
  store i64 %v0, ptr %gcs0
  %v3 = call i64 @df.class(i64 %v0)
  %v4 = sub i64 %v3, 8
  %v5 = icmp ult i64 %v4, 3
  br i1 %v5, label %b3, label %b4
b3:
  %v6 = call i64 @df.list_len(i64 %v0)
  br label %b6
b4:
  %sat75 = getelementptr [1 x i64], ptr %sargs75, i64 0, i64 0
  store i64 0, ptr %sat75
  %sic75 = getelementptr i64, ptr %area, i64 8
  ; df.selector-exact 10
  %sf75 = call ptr @df.selector(ptr %sic75, i64 %v0, i64 -5476269667379286226, ptr @df.seln.0, i64 10)
  %v75 = call i64 %sf75(i64 %v0, ptr %sargs75, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  store i64 %v75, ptr %gcs0
  %v76 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v76 to i64
  %v77 = icmp ne i64 %c0, 0
  br i1 %v77, label %b49, label %b48
b5:
  %v142 = ptrtoint ptr getelementptr inbounds (i8, ptr @"df.s.34de1dc3712a2fb06691d5373a77308b", i64 2) to i64
  %v125 = ptrtoint ptr getelementptr inbounds (i8, ptr @"df.s.34de1dc3712a2fb06691d5373a77308b", i64 2) to i64
  %v126 = call i1 @df.string_equals_object(i64 %v142, i64 %v125)
  br i1 %v126, label %b85, label %b86
b6:
  %v141 = phi i64 [ 0, %b3 ], [ %v43, %b46 ]
  %v9 = call i64 @df.list_len(i64 %v0)
  %v10 = icmp eq i64 %v9, %v6
  br i1 %v10, label %b8, label %b7
b7:
  store i64 0, ptr %gcs1
  %ta11 = getelementptr inbounds i8, ptr %ctx, i64 96
  %tb11 = load ptr, ptr %ta11, align 8
  %tfa11 = getelementptr inbounds i8, ptr %ctx, i64 104
  %tf11 = load ptr, ptr %tfa11, align 8
  %tnx11 = getelementptr i8, ptr %tb11, i64 32
  %tz11 = icmp ugt ptr %tnx11, %tf11
  %tx11 = call i1 @llvm.expect.i1(i1 %tz11, i1 false)
  br i1 %tx11, label %ao11.lento, label %ao11.rapido
ao11.rapido:
  store ptr %tnx11, ptr %ta11, align 8
  store i64 2495376130049, ptr %tb11, align 8
  %thb11 = ptrtoint ptr %tb11 to i64
  %th11 = add i64 %thb11, 2
  br label %ao11.fim
ao11.lento:
  %tl11 = call i64 @dartforge_object_new_t(i64 581, i64 2, ptr @df.mt.dart$3acore.ConcurrentModificationError)
  br label %ao11.fim
ao11.fim:
  %v11 = phi i64 [ %th11, %ao11.rapido ], [ %tl11, %ao11.lento ]
  store i64 %v11, ptr %gcs1
  call void @df.dart$3acore.ConcurrentModificationError.new(i64 %v11, i64 %v0)
  %v13 = load i8, ptr %ctx, align 8
  %c1 = zext i8 %v13 to i64
  %v14 = icmp ne i64 %c1, 0
  br i1 %v14, label %b11, label %b10
b8:
  %v17 = icmp slt i64 %v141, %v9
  br i1 %v17, label %b9, label %b5
b9:
  %ip18 = getelementptr inbounds i8, ptr %ctx, i64 32
  %iv18 = load atomic i8, ptr %ip18 monotonic, align 8
  %v18 = call i8 @llvm.expect.i8(i8 %iv18, i8 0)
  %c2 = zext i8 %v18 to i64
  %v19 = icmp ne i64 %c2, 0
  br i1 %v19, label %b13, label %b14
b10:
  store i64 0, ptr %gcs0
  call void @dartforge_exception_throw(i64 %v11, i8 3)
  %gcvolta10 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta10, ptr %ctxtopo, align 8
  ret void
b11:
  %gcvolta11 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta11, ptr %ctxtopo, align 8
  ret void
b13:
  store i64 0, ptr %gcs1
  call void @dartforge_safepoint()
  %v21 = load i8, ptr %ctx, align 8
  %c3 = zext i8 %v21 to i64
  %v22 = icmp ne i64 %c3, 0
  br i1 %v22, label %b16, label %b15
b14:
  %v23 = call i64 @df.core_storage(i64 %v0)
  %v24 = call i64 @df.core_shape(i64 %v23)
  %v25 = add i64 %v23, 22
  %v26 = icmp eq i64 %v24, 4
  br i1 %v26, label %b18, label %b19
b15:
  br label %b14
b16:
  %gcvolta16 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta16, ptr %ctxtopo, align 8
  ret void
b17:
  %v42 = phi i64 [ %v27, %b18 ], [ %v30, %b20 ], [ %v33, %b22 ], [ %v37, %b24 ], [ %v39, %b26 ]
  store i64 %v42, ptr %gcs1
  %v43 = add i64 %v141, 1
  %v48 = call i64 @df.class(i64 %v42)
  %v49 = icmp eq i64 %v48, 11
  br i1 %v49, label %b31, label %b29
b18:
  %v27 = call i64 @df.word_ref(i64 %v25, i64 %v141)
  br label %b17
b19:
  %v28 = icmp eq i64 %v24, 34
  br i1 %v28, label %b20, label %b21
b20:
  %cp29 = inttoptr i64 %v25 to ptr
  %cg29 = getelementptr i64, ptr %cp29, i64 %v141
  %v29 = load i64, ptr %cg29, align 1
  store i64 0, ptr %gcs1
  %v30 = call i64 @df.box_int(i64 %v29)
  br label %b17
b21:
  %v31 = icmp eq i64 %v24, 66
  br i1 %v31, label %b22, label %b23
b22:
  %cp32 = inttoptr i64 %v25 to ptr
  %cg32 = getelementptr double, ptr %cp32, i64 %v141
  %v32 = load double, ptr %cg32, align 1
  store i64 0, ptr %gcs1
  %v33 = call i64 @df.box_double(double %v32)
  br label %b17
b23:
  %v34 = icmp eq i64 %v24, 98
  br i1 %v34, label %b24, label %b25
b24:
  %cp35 = inttoptr i64 %v25 to ptr
  %cg35 = getelementptr i64, ptr %cp35, i64 %v141
  %v35 = load i64, ptr %cg35, align 1
  %v36 = icmp ne i64 %v35, 0
  store i64 0, ptr %gcs1
  %v37 = call i64 @df.box_bool(i1 %v36)
  br label %b17
b25:
  store i64 0, ptr %gcs1
  %v38 = call i64 @df.box_int(i64 %v141)
  store i64 %v38, ptr %gcs1
  %sa39_0 = getelementptr [2 x i64], ptr %sargs39, i64 0, i64 0
  store i64 %v38, ptr %sa39_0
  %sat39 = getelementptr [2 x i64], ptr %sargs39, i64 0, i64 1
  store i64 0, ptr %sat39
  %sic39 = getelementptr i64, ptr %area, i64 10
  ; df.selector-exact 10
  %sf39 = call ptr @df.selector(ptr %sic39, i64 %v0, i64 2870274114568068918, ptr @df.seln.1, i64 5)
  %v39 = call i64 %sf39(i64 %v0, ptr %sargs39, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v40 = load i8, ptr %ctx, align 8
  %c4 = zext i8 %v40 to i64
  %v41 = icmp ne i64 %c4, 0
  br i1 %v41, label %b27, label %b26
b26:
  br label %b17
b27:
  %gcvolta27 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta27, ptr %ctxtopo, align 8
  ret void
b28:
  %fxk52 = and i64 %v42, -9223372036854775801
  %fxo52 = icmp eq i64 %fxk52, 2
  %fxa52 = add i64 %v42, -2
  %fxq52 = inttoptr i64 %fxa52 to ptr
  %fxvp52 = getelementptr inbounds i8, ptr %ctx, i64 40
  %fxv52 = load ptr, ptr %fxvp52, align 8
  %fcb52 = select i1 %fxo52, ptr %fxq52, ptr %fxv52
  %fp52 = getelementptr inbounds i8, ptr %fcb52, i64 16
  %fg52 = getelementptr inbounds i64, ptr %fp52, i64 2
  %v52 = load i64, ptr %fg52, align 8
  %fxk53 = and i64 %v42, -9223372036854775801
  %fxo53 = icmp eq i64 %fxk53, 2
  %fxa53 = add i64 %v42, -2
  %fxq53 = inttoptr i64 %fxa53 to ptr
  %fxvp53 = getelementptr inbounds i8, ptr %ctx, i64 40
  %fxv53 = load ptr, ptr %fxvp53, align 8
  %fcb53 = select i1 %fxo53, ptr %fxq53, ptr %fxv53
  %fp53 = getelementptr inbounds i8, ptr %fcb53, i64 16
  %fg53 = getelementptr inbounds i64, ptr %fp53, i64 1
  %v53 = load i64, ptr %fg53, align 8
  store i64 %v53, ptr %gcs1
  %ct54 = inttoptr i64 %v52 to ptr
  %v54 = call i64 %ct54(i64 %v53)
  %v55 = load i8, ptr %ctx, align 8
  %c5 = zext i8 %v55 to i64
  %v56 = icmp ne i64 %c5, 0
  br i1 %v56, label %b33, label %b32
b29:
  %cat57 = getelementptr [1 x i64], ptr %cargs57, i64 0, i64 0
  store i64 0, ptr %cat57
  %cek57 = and i64 %v42, -9223372036854775801
  %ceo57 = icmp eq i64 %cek57, 2
  br i1 %ceo57, label %ce57.obj, label %ce57.lento
ce57.obj:
  %ceca57 = add i64 %v42, 2
  %cecp57 = inttoptr i64 %ceca57 to ptr
  %cec57 = load i32, ptr %cecp57, align 4
  %ceq57 = icmp eq i32 %cec57, 11
  br i1 %ceq57, label %ce57.rapido, label %ce57.lento
ce57.rapido:
  %cefa57 = add i64 %v42, 14
  %cefp57 = inttoptr i64 %cefa57 to ptr
  %cer57 = load i64, ptr %cefp57, align 8
  br label %ce57.fim
ce57.lento:
  %cel57 = call i64 @dartforge_closure_entry(i64 %v42)
  br label %ce57.fim
ce57.fim:
  %cc57 = phi i64 [ %cer57, %ce57.rapido ], [ %cel57, %ce57.lento ]
  %cz57 = icmp eq i64 %cc57, 0
  %cq57 = select i1 %cz57, i64 ptrtoint (ptr @df_clo_invalido to i64), i64 %cc57
  %cf57 = inttoptr i64 %cq57 to ptr
  %v57 = call i64 %cf57(i64 %v42, ptr %cargs57, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  %v58 = load i8, ptr %ctx, align 8
  %c6 = zext i8 %v58 to i64
  %v59 = icmp ne i64 %c6, 0
  br i1 %v59, label %b35, label %b34
b30:
  %v60 = phi i64 [ %v54, %b32 ], [ %v57, %b34 ]
  %v61 = call i64 @df.class(i64 %v60)
  switch i64 %v61, label %b37 [ i64 1027, label %b37 i64 1026, label %b38 i64 1025, label %b39 ]
b31:
  %fxk50 = and i64 %v42, -9223372036854775801
  %fxo50 = icmp eq i64 %fxk50, 2
  %fxa50 = add i64 %v42, -2
  %fxq50 = inttoptr i64 %fxa50 to ptr
  %fxvp50 = getelementptr inbounds i8, ptr %ctx, i64 40
  %fxv50 = load ptr, ptr %fxvp50, align 8
  %fcb50 = select i1 %fxo50, ptr %fxq50, ptr %fxv50
  %fp50 = getelementptr inbounds i8, ptr %fcb50, i64 16
  %fg50 = getelementptr inbounds i64, ptr %fp50, i64 3
  %v50 = load i64, ptr %fg50, align 8
  %v51 = icmp eq i64 %v50, 6
  br i1 %v51, label %b28, label %b29
b32:
  br label %b30
b33:
  %gcvolta33 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta33, ptr %ctxtopo, align 8
  ret void
b34:
  br label %b30
b35:
  %gcvolta35 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta35, ptr %ctxtopo, align 8
  ret void
b36:
  %v71 = phi i64 [ %v143, %b37 ], [ %v144, %b38 ], [ %v145, %b39 ]
  store i64 %v71, ptr %gcs1
  call void @df.dart$3acore..print(i64 %v71)
  %v73 = load i8, ptr %ctx, align 8
  %c7 = zext i8 %v73 to i64
  %v74 = icmp ne i64 %c7, 0
  br i1 %v74, label %b47, label %b46
b37:
  %v143 = ptrtoint ptr getelementptr inbounds (i8, ptr @"df.s.646f238b2717745bee5c48412802f4ff", i64 2) to i64
  br label %b36
b38:
  %v144 = ptrtoint ptr getelementptr inbounds (i8, ptr @"df.s.24d4827748980086fff04bcb8934715f", i64 2) to i64
  br label %b36
b39:
  %v145 = ptrtoint ptr getelementptr inbounds (i8, ptr @"df.s.1566253b2ae2096e6cb28f3c26be63ff", i64 2) to i64
  br label %b36
b46:
  br label %b6
b47:
  %gcvolta47 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta47, ptr %ctxtopo, align 8
  ret void
b48:
  br label %b50
b49:
  %gcvolta49 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta49, ptr %ctxtopo, align 8
  ret void
b50:
  store i64 0, ptr %gcs1
  %sat78 = getelementptr [1 x i64], ptr %sargs78, i64 0, i64 0
  store i64 0, ptr %sat78
  %sic78 = getelementptr i64, ptr %area, i64 12
  %sf78 = call ptr @df.selector(ptr %sic78, i64 %v75, i64 -5038376148439920376, ptr @df.seln.2, i64 10)
  %v78 = call i64 %sf78(i64 %v75, ptr %sargs78, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  store i64 %v78, ptr %gcs1
  %v79 = load i8, ptr %ctx, align 8
  %c8 = zext i8 %v79 to i64
  %v80 = icmp ne i64 %c8, 0
  br i1 %v80, label %b54, label %b53
b51:
  %ip84 = getelementptr inbounds i8, ptr %ctx, i64 32
  %iv84 = load atomic i8, ptr %ip84 monotonic, align 8
  %v84 = call i8 @llvm.expect.i8(i8 %iv84, i8 0)
  %c9 = zext i8 %v84 to i64
  %v85 = icmp ne i64 %c9, 0
  br i1 %v85, label %b57, label %b58
b52:
  br label %b5
b53:
  %v81 = call i1 @df.unbox_bool(i64 %v78)
  %v82 = load i8, ptr %ctx, align 8
  %c10 = zext i8 %v82 to i64
  %v83 = icmp ne i64 %c10, 0
  br i1 %v83, label %b56, label %b55
b54:
  %gcvolta54 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta54, ptr %ctxtopo, align 8
  ret void
b55:
  br i1 %v81, label %b51, label %b52
b56:
  %gcvolta56 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta56, ptr %ctxtopo, align 8
  ret void
b57:
  store i64 0, ptr %gcs1
  call void @dartforge_safepoint()
  %v87 = load i8, ptr %ctx, align 8
  %c11 = zext i8 %v87 to i64
  %v88 = icmp ne i64 %c11, 0
  br i1 %v88, label %b60, label %b59
b58:
  store i64 0, ptr %gcs1
  %sat89 = getelementptr [1 x i64], ptr %sargs89, i64 0, i64 0
  store i64 0, ptr %sat89
  %sic89 = getelementptr i64, ptr %area, i64 14
  %sf89 = call ptr @df.selector(ptr %sic89, i64 %v75, i64 1052728634250411761, ptr @df.seln.3, i64 9)
  %v89 = call i64 %sf89(i64 %v75, ptr %sargs89, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  store i64 %v89, ptr %gcs1
  %v90 = load i8, ptr %ctx, align 8
  %c12 = zext i8 %v90 to i64
  %v91 = icmp ne i64 %c12, 0
  br i1 %v91, label %b62, label %b61
b59:
  br label %b58
b60:
  %gcvolta60 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta60, ptr %ctxtopo, align 8
  ret void
b61:
  %v95 = call i64 @df.class(i64 %v89)
  %v96 = icmp eq i64 %v95, 11
  br i1 %v96, label %b66, label %b64
b62:
  %gcvolta62 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta62, ptr %ctxtopo, align 8
  ret void
b63:
  %fxk99 = and i64 %v89, -9223372036854775801
  %fxo99 = icmp eq i64 %fxk99, 2
  %fxa99 = add i64 %v89, -2
  %fxq99 = inttoptr i64 %fxa99 to ptr
  %fxvp99 = getelementptr inbounds i8, ptr %ctx, i64 40
  %fxv99 = load ptr, ptr %fxvp99, align 8
  %fcb99 = select i1 %fxo99, ptr %fxq99, ptr %fxv99
  %fp99 = getelementptr inbounds i8, ptr %fcb99, i64 16
  %fg99 = getelementptr inbounds i64, ptr %fp99, i64 2
  %v99 = load i64, ptr %fg99, align 8
  %fxk100 = and i64 %v89, -9223372036854775801
  %fxo100 = icmp eq i64 %fxk100, 2
  %fxa100 = add i64 %v89, -2
  %fxq100 = inttoptr i64 %fxa100 to ptr
  %fxvp100 = getelementptr inbounds i8, ptr %ctx, i64 40
  %fxv100 = load ptr, ptr %fxvp100, align 8
  %fcb100 = select i1 %fxo100, ptr %fxq100, ptr %fxv100
  %fp100 = getelementptr inbounds i8, ptr %fcb100, i64 16
  %fg100 = getelementptr inbounds i64, ptr %fp100, i64 1
  %v100 = load i64, ptr %fg100, align 8
  store i64 %v100, ptr %gcs1
  %ct101 = inttoptr i64 %v99 to ptr
  %v101 = call i64 %ct101(i64 %v100)
  %v102 = load i8, ptr %ctx, align 8
  %c13 = zext i8 %v102 to i64
  %v103 = icmp ne i64 %c13, 0
  br i1 %v103, label %b68, label %b67
b64:
  %cat104 = getelementptr [1 x i64], ptr %cargs104, i64 0, i64 0
  store i64 0, ptr %cat104
  %cek104 = and i64 %v89, -9223372036854775801
  %ceo104 = icmp eq i64 %cek104, 2
  br i1 %ceo104, label %ce104.obj, label %ce104.lento
ce104.obj:
  %ceca104 = add i64 %v89, 2
  %cecp104 = inttoptr i64 %ceca104 to ptr
  %cec104 = load i32, ptr %cecp104, align 4
  %ceq104 = icmp eq i32 %cec104, 11
  br i1 %ceq104, label %ce104.rapido, label %ce104.lento
ce104.rapido:
  %cefa104 = add i64 %v89, 14
  %cefp104 = inttoptr i64 %cefa104 to ptr
  %cer104 = load i64, ptr %cefp104, align 8
  br label %ce104.fim
ce104.lento:
  %cel104 = call i64 @dartforge_closure_entry(i64 %v89)
  br label %ce104.fim
ce104.fim:
  %cc104 = phi i64 [ %cer104, %ce104.rapido ], [ %cel104, %ce104.lento ]
  %cz104 = icmp eq i64 %cc104, 0
  %cq104 = select i1 %cz104, i64 ptrtoint (ptr @df_clo_invalido to i64), i64 %cc104
  %cf104 = inttoptr i64 %cq104 to ptr
  %v104 = call i64 %cf104(i64 %v89, ptr %cargs104, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  %v105 = load i8, ptr %ctx, align 8
  %c14 = zext i8 %v105 to i64
  %v106 = icmp ne i64 %c14, 0
  br i1 %v106, label %b70, label %b69
b65:
  %v107 = phi i64 [ %v101, %b67 ], [ %v104, %b69 ]
  %v108 = call i64 @df.class(i64 %v107)
  switch i64 %v108, label %b72 [ i64 1027, label %b72 i64 1026, label %b73 i64 1025, label %b74 ]
b66:
  %fxk97 = and i64 %v89, -9223372036854775801
  %fxo97 = icmp eq i64 %fxk97, 2
  %fxa97 = add i64 %v89, -2
  %fxq97 = inttoptr i64 %fxa97 to ptr
  %fxvp97 = getelementptr inbounds i8, ptr %ctx, i64 40
  %fxv97 = load ptr, ptr %fxvp97, align 8
  %fcb97 = select i1 %fxo97, ptr %fxq97, ptr %fxv97
  %fp97 = getelementptr inbounds i8, ptr %fcb97, i64 16
  %fg97 = getelementptr inbounds i64, ptr %fp97, i64 3
  %v97 = load i64, ptr %fg97, align 8
  %v98 = icmp eq i64 %v97, 6
  br i1 %v98, label %b63, label %b64
b67:
  br label %b65
b68:
  %gcvolta68 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta68, ptr %ctxtopo, align 8
  ret void
b69:
  br label %b65
b70:
  %gcvolta70 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta70, ptr %ctxtopo, align 8
  ret void
b71:
  %v118 = phi i64 [ %v146, %b72 ], [ %v147, %b73 ], [ %v148, %b74 ]
  store i64 %v118, ptr %gcs1
  call void @df.dart$3acore..print(i64 %v118)
  %v120 = load i8, ptr %ctx, align 8
  %c15 = zext i8 %v120 to i64
  %v121 = icmp ne i64 %c15, 0
  br i1 %v121, label %b82, label %b81
b72:
  %v146 = ptrtoint ptr getelementptr inbounds (i8, ptr @"df.s.646f238b2717745bee5c48412802f4ff", i64 2) to i64
  br label %b71
b73:
  %v147 = ptrtoint ptr getelementptr inbounds (i8, ptr @"df.s.24d4827748980086fff04bcb8934715f", i64 2) to i64
  br label %b71
b74:
  %v148 = ptrtoint ptr getelementptr inbounds (i8, ptr @"df.s.1566253b2ae2096e6cb28f3c26be63ff", i64 2) to i64
  br label %b71
b81:
  br label %b50
b82:
  %gcvolta82 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta82, ptr %ctxtopo, align 8
  ret void
b85:
  %v127 = ptrtoint ptr getelementptr inbounds (i8, ptr @"df.s.c7e7393fee05ab55506b71a66b1a98f6", i64 2) to i64
  br label %b87
b86:
  %v128 = ptrtoint ptr getelementptr inbounds (i8, ptr @"df.s.e8b39ab0f31bf65256f0d025e348328e", i64 2) to i64
  br label %b87
b87:
  %v129 = phi i64 [ %v127, %b85 ], [ %v128, %b86 ]
  store i64 %v129, ptr %gcs0
  store i64 0, ptr %gcs1
  call void @df.dart$3acore..print(i64 %v129)
  %v131 = load i8, ptr %ctx, align 8
  %c16 = zext i8 %v131 to i64
  %v132 = icmp ne i64 %c16, 0
  br i1 %v132, label %b89, label %b88
b88:
  store i64 0, ptr %gcs0
  %v133 = call i64 @df.dart$3aio.Platform.lineTerminator()
  %v134 = load i8, ptr %ctx, align 8
  %c17 = zext i8 %v134 to i64
  %v135 = icmp ne i64 %c17, 0
  br i1 %v135, label %b91, label %b90
b89:
  %gcvolta89 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta89, ptr %ctxtopo, align 8
  ret void
b90:
  %v136 = call i64 @df.string_len(i64 %v133)
  %v137 = call i64 @df.box_int(i64 %v136)
  store i64 %v137, ptr %gcs0
  call void @df.dart$3acore..print(i64 %v137)
  %v139 = load i8, ptr %ctx, align 8
  %c18 = zext i8 %v139 to i64
  %v140 = icmp ne i64 %c18, 0
  br i1 %v140, label %b93, label %b92
b91:
  %gcvolta91 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta91, ptr %ctxtopo, align 8
  ret void
b92:
  %gcvolta92 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta92, ptr %ctxtopo, align 8
  ret void
b93:
  %gcvolta93 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta93, ptr %ctxtopo, align 8
  ret void
}

define i64 @df.main$2edart.WindowsBackend.name$g(i64 %v0, ptr %v1, ptr %v2) nounwind {
b0:
  %area = call ptr @df.get_area()
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
  %v11 = ptrtoint ptr getelementptr inbounds (i8, ptr @"df.s.646f238b2717745bee5c48412802f4ff", i64 2) to i64
  ret i64 %v11
b2:
  call void @dartforge_nsm_call()
  ret i64 0
}

define i64 @df.main$2edart.MacBackend.name$g(i64 %v0, ptr %v1, ptr %v2) nounwind {
b0:
  %area = call ptr @df.get_area()
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
  %v11 = ptrtoint ptr getelementptr inbounds (i8, ptr @"df.s.24d4827748980086fff04bcb8934715f", i64 2) to i64
  ret i64 %v11
b2:
  call void @dartforge_nsm_call()
  ret i64 0
}

define i64 @df.main$2edart.LinuxBackend.name$g(i64 %v0, ptr %v1, ptr %v2) nounwind {
b0:
  %area = call ptr @df.get_area()
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
  %v11 = ptrtoint ptr getelementptr inbounds (i8, ptr @"df.s.1566253b2ae2096e6cb28f3c26be63ff", i64 2) to i64
  ret i64 %v11
b2:
  call void @dartforge_nsm_call()
  ret i64 0
}

define i64 @dartforge_call_dart0(i64 %v0) nounwind {
b0:
  %cargs1 = alloca [1 x i64]
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
  store { ptr, i64, [1 x i64] } { ptr null, i64 1, [1 x i64] zeroinitializer }, ptr %gcq
  %gcs0 = getelementptr inbounds { ptr, i64, [1 x i64] }, ptr %gcq, i64 0, i32 2, i64 0
  %gcant = load ptr, ptr %ctxtopo, align 8
  store ptr %gcant, ptr %gcq, align 8
  store ptr %gcq, ptr %ctxtopo, align 8
  store i64 %v0, ptr %gcs0
  %cat1 = getelementptr [1 x i64], ptr %cargs1, i64 0, i64 0
  store i64 0, ptr %cat1
  %cek1 = and i64 %v0, -9223372036854775801
  %ceo1 = icmp eq i64 %cek1, 2
  br i1 %ceo1, label %ce1.obj, label %ce1.lento
ce1.obj:
  %ceca1 = add i64 %v0, 2
  %cecp1 = inttoptr i64 %ceca1 to ptr
  %cec1 = load i32, ptr %cecp1, align 4
  %ceq1 = icmp eq i32 %cec1, 11
  br i1 %ceq1, label %ce1.rapido, label %ce1.lento
ce1.rapido:
  %cefa1 = add i64 %v0, 14
  %cefp1 = inttoptr i64 %cefa1 to ptr
  %cer1 = load i64, ptr %cefp1, align 8
  br label %ce1.fim
ce1.lento:
  %cel1 = call i64 @dartforge_closure_entry(i64 %v0)
  br label %ce1.fim
ce1.fim:
  %cc1 = phi i64 [ %cer1, %ce1.rapido ], [ %cel1, %ce1.lento ]
  %cz1 = icmp eq i64 %cc1, 0
  %cq1 = select i1 %cz1, i64 ptrtoint (ptr @df_clo_invalido to i64), i64 %cc1
  %cf1 = inttoptr i64 %cq1 to ptr
  %v1 = call i64 %cf1(i64 %v0, ptr %cargs1, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
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

define i64 @df.$type.$3d$3d$c(i64 %v0, ptr %v1, ptr %v2) nounwind {
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
  store { ptr, i64, [2 x i64] } { ptr null, i64 2, [2 x i64] zeroinitializer }, ptr %gcq
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

define void @dartforge_rti_init() nounwind {
b0:
  %v0 = getelementptr i64, ptr @df.const.i64.3079.881c74f717bc41c6a12fbe6ffdcbfb8c7e587b27e7f448af650591eb1a83a01f, i64 0
  call void @dartforge_rti_init_table(ptr %v0, i64 24629)
  ret void
}

define i64 @dartforge_dispatch_toString(i64 %obj) nounwind {
b0:
  %a = alloca [1 x i64]
  store i64 0, ptr %a
  %area = call ptr @df.get_area()
  %ic = getelementptr i64, ptr %area, i64 16
  %f = call ptr @df.selector(ptr %ic, i64 %obj, i64 -3445551396850951156, ptr @df.seln.4, i64 10)
  %r = call i64 %f(i64 %obj, ptr %a, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  ret i64 %r
}

@df.mt.main$2edart.WindowsBackend$d = private unnamed_addr constant { i64, i64, i64, ptr, i64, ptr, i64, ptr, i64, ptr, i64, ptr, i64, ptr, i64, ptr, i64, ptr } { i64 1027, i64 8, i64 -9163797671095228218, ptr @df.dart$3acore.Object.$3d$3d$c, i64 -3445551396850951156, ptr @df.dart$3acore.Object.toString$c, i64 -1301464018035317077, ptr @df.dart$3acore.Object.noSuchMethod$tc, i64 494151758419248719, ptr @df.main$2edart.WindowsBackend.name$g, i64 3599887816169301665, ptr @df.dart$3acore.Object.noSuchMethod$c, i64 4031450617872922849, ptr @df.dart$3acore.Object.hashCode$g, i64 5306504273795186908, ptr @df.dart$3acore.Object._identityHashCode$g, i64 7093520068988870176, ptr @df.dart$3acore.Object.runtimeType$g }
define ptr @df.mt.main$2edart.WindowsBackend() nounwind {
b0:
  ret ptr @df.mt.main$2edart.WindowsBackend$d
}
@df.mt.main$2edart.MacBackend$d = private unnamed_addr constant { i64, i64, i64, ptr, i64, ptr, i64, ptr, i64, ptr, i64, ptr, i64, ptr, i64, ptr, i64, ptr } { i64 1026, i64 8, i64 -9163797671095228218, ptr @df.dart$3acore.Object.$3d$3d$c, i64 -3445551396850951156, ptr @df.dart$3acore.Object.toString$c, i64 -1301464018035317077, ptr @df.dart$3acore.Object.noSuchMethod$tc, i64 494151758419248719, ptr @df.main$2edart.MacBackend.name$g, i64 3599887816169301665, ptr @df.dart$3acore.Object.noSuchMethod$c, i64 4031450617872922849, ptr @df.dart$3acore.Object.hashCode$g, i64 5306504273795186908, ptr @df.dart$3acore.Object._identityHashCode$g, i64 7093520068988870176, ptr @df.dart$3acore.Object.runtimeType$g }
define ptr @df.mt.main$2edart.MacBackend() nounwind {
b0:
  ret ptr @df.mt.main$2edart.MacBackend$d
}
@df.mt.main$2edart.LinuxBackend$d = private unnamed_addr constant { i64, i64, i64, ptr, i64, ptr, i64, ptr, i64, ptr, i64, ptr, i64, ptr, i64, ptr, i64, ptr } { i64 1025, i64 8, i64 -9163797671095228218, ptr @df.dart$3acore.Object.$3d$3d$c, i64 -3445551396850951156, ptr @df.dart$3acore.Object.toString$c, i64 -1301464018035317077, ptr @df.dart$3acore.Object.noSuchMethod$tc, i64 494151758419248719, ptr @df.main$2edart.LinuxBackend.name$g, i64 3599887816169301665, ptr @df.dart$3acore.Object.noSuchMethod$c, i64 4031450617872922849, ptr @df.dart$3acore.Object.hashCode$g, i64 5306504273795186908, ptr @df.dart$3acore.Object._identityHashCode$g, i64 7093520068988870176, ptr @df.dart$3acore.Object.runtimeType$g }
define ptr @df.mt.main$2edart.LinuxBackend() nounwind {
b0:
  ret ptr @df.mt.main$2edart.LinuxBackend$d
}
@df.mt.$type$d = private unnamed_addr constant { i64, i64, i64, ptr, i64, ptr, i64, ptr, i64, ptr, i64, ptr, i64, ptr, i64, ptr } { i64 1073741569, i64 7, i64 -9163797671095228218, ptr @df.$type.$3d$3d$c, i64 -3445551396850951156, ptr @df.$type.toString$c, i64 -1301464018035317077, ptr @df.dart$3acore.Object.noSuchMethod$tc, i64 3599887816169301665, ptr @df.dart$3acore.Object.noSuchMethod$c, i64 4031450617872922849, ptr @df.dart$3acore.Object.hashCode$g, i64 5306504273795186908, ptr @df.dart$3acore.Object._identityHashCode$g, i64 7093520068988870176, ptr @df.dart$3acore.Object.runtimeType$g }
define ptr @df.mt.$type() nounwind {
b0:
  ret ptr @df.mt.$type$d
}
@df.img.anchor = linkonce_odr hidden constant [2 x i64] zeroinitializer, section "__DATA_CONST,__dfimg", align 8
@"\01section$start$__DATA_CONST$__dfimg" = external hidden global i8
@"\01section$end$__DATA_CONST$__dfimg" = external hidden global i8
define void @df.register.program() nounwind {
b0:
  call void @dartforge_register_class_name(i64 0, ptr @.str.13, i64 6)
  call void @dartforge_register_class_name(i64 1024, ptr @.str.14, i64 7)
  call void @dartforge_register_class_name(i64 1027, ptr @.str.15, i64 14)
  call void @dartforge_register_class_name(i64 1026, ptr @.str.16, i64 10)
  call void @dartforge_register_class_name(i64 1025, ptr @.str.17, i64 12)
  call void @dartforge_register_class_name(i64 1073741568, ptr @.str.18, i64 11)
  call void @dartforge_register_class_name(i64 1073741569, ptr @.str.19, i64 5)
  call void @dartforge_register_subclass(i64 1024, i64 0)
  call void @dartforge_register_subclass(i64 1024, i64 602)
  call void @dartforge_register_subclass(i64 1027, i64 0)
  call void @dartforge_register_subclass(i64 1027, i64 602)
  call void @dartforge_register_subclass(i64 1027, i64 1024)
  call void @dartforge_register_subclass(i64 1026, i64 0)
  call void @dartforge_register_subclass(i64 1026, i64 602)
  call void @dartforge_register_subclass(i64 1026, i64 1024)
  call void @dartforge_register_subclass(i64 1025, i64 0)
  call void @dartforge_register_subclass(i64 1025, i64 602)
  call void @dartforge_register_subclass(i64 1025, i64 1024)
  call void @dartforge_register_subclass(i64 1073741569, i64 621)
  call void @dartforge_register_image(ptr @"\01section$start$__DATA_CONST$__dfimg", ptr @"\01section$end$__DATA_CONST$__dfimg")
  call void @dartforge_register_table(i64 1027, ptr @df.mt.main$2edart.WindowsBackend)
  call void @dartforge_register_table(i64 1026, ptr @df.mt.main$2edart.MacBackend)
  call void @dartforge_register_table(i64 1025, ptr @df.mt.main$2edart.LinuxBackend)
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
  call void @df.register.sdk_tables()
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
  call void @dartforge_prepare_embedder()
  ret void
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

@df.seln.0 = private unnamed_addr constant [10 x i8] c"g:iterator"
@df.seln.1 = private unnamed_addr constant [5 x i8] c"tc:\5B\5D"
@df.seln.2 = private unnamed_addr constant [10 x i8] c"c:moveNext"
@df.seln.3 = private unnamed_addr constant [9 x i8] c"g:current"
@df.seln.4 = private unnamed_addr constant [10 x i8] c"c:toString"

@df.area = private unnamed_addr constant [2 x i64] [i64 -7318119598941579387, i64 -18]
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
; Static string literals (docs/NATIVO-ESPACO-UNIFICADO.md §2.11)
@"df.s.092312ceffedb5287ea8c500f2c49c7c" = linkonce_odr constant { i64, i64, i64, [16 x i8] } { i64 25770000901, i64 247715052, i64 14, [16 x i8] c"F<0;C1026;0;;>\00\00" }, section "__DATA_CONST,__dfimg", align 8
@"df.s.107562bc5fad35223e473f09a4a0180c" = linkonce_odr constant { i64, i64, i64, [16 x i8] } { i64 25770000901, i64 645738346, i64 14, [16 x i8] c"F<0;C1025;0;;>\00\00" }, section "__DATA_CONST,__dfimg", align 8
@"df.s.1566253b2ae2096e6cb28f3c26be63ff" = linkonce_odr constant { i64, i64, i64, [16 x i8] } { i64 25770000901, i64 258364959, i64 13, [16 x i8] c"linux backend\00\00\00" }, section "__DATA_CONST,__dfimg", align 8
@"df.s.1591faa1157de847b0cebd0d94681d83" = linkonce_odr constant { i64, i64, i64, [8 x i8] } { i64 25769935365, i64 272493910, i64 5, [8 x i8] c"linux\00\00\00" }, section "__DATA_CONST,__dfimg", align 8
@"df.s.24d14e5277ea3e91174e98c49002b4cf" = linkonce_odr constant { i64, i64, i64, [24 x i8] } { i64 25770066437, i64 196363423, i64 20, [24 x i8] c"C597<F<0;C1024;0;;>>\00\00\00\00" }, section "__DATA_CONST,__dfimg", align 8
@"df.s.24d4827748980086fff04bcb8934715f" = linkonce_odr constant { i64, i64, i64, [16 x i8] } { i64 25770000901, i64 379131578, i64 13, [16 x i8] c"macos backend\00\00\00" }, section "__DATA_CONST,__dfimg", align 8
@"df.s.34de1dc3712a2fb06691d5373a77308b" = linkonce_odr constant { i64, i64, i64, [8 x i8] } { i64 25769935365, i64 445068545, i64 1, [8 x i8] c"/\00\00\00\00\00\00\00" }, section "__DATA_CONST,__dfimg", align 8
@"df.s.646f238b2717745bee5c48412802f4ff" = linkonce_odr constant { i64, i64, i64, [16 x i8] } { i64 25770000901, i64 1019303910, i64 13, [16 x i8] c"win32 backend\00\00\00" }, section "__DATA_CONST,__dfimg", align 8
@"df.s.a74ddd44d1f897095271484348e043a5" = linkonce_odr constant { i64, i64, i64, [16 x i8] } { i64 25770000901, i64 982751516, i64 14, [16 x i8] c"F<0;C1027;0;;>\00\00" }, section "__DATA_CONST,__dfimg", align 8
@"df.s.a9d599a6f00817a79ce41246096a66cc" = linkonce_odr constant { i64, i64, i64, [8 x i8] } { i64 25769935365, i64 828860261, i64 5, [8 x i8] c"macos\00\00\00" }, section "__DATA_CONST,__dfimg", align 8
@"df.s.c7e7393fee05ab55506b71a66b1a98f6" = linkonce_odr constant { i64, i64, i64, [16 x i8] } { i64 25770000901, i64 527090562, i64 15, [16 x i8] c"posix separator\00" }, section "__DATA_CONST,__dfimg", align 8
@"df.s.d2024387aa91ee92eeb44f2f09100c37" = linkonce_odr constant { i64, i64, i64, [8 x i8] } { i64 25769935365, i64 1065208899, i64 7, [8 x i8] c"windows\00" }, section "__DATA_CONST,__dfimg", align 8
@"df.s.e8b39ab0f31bf65256f0d025e348328e" = linkonce_odr constant { i64, i64, i64, [24 x i8] } { i64 25770066437, i64 952156061, i64 17, [24 x i8] c"windows separator\00\00\00\00\00\00\00" }, section "__DATA_CONST,__dfimg", align 8

; Defined in another module (source SDK) or in the runtime
declare void @df.dart$3acore..print(i64)
declare void @df.dart$3acore.ConcurrentModificationError.new(i64, i64)
declare i64 @df.dart$3acore.Object.$3d$3d$c(i64, ptr, ptr)
declare i64 @df.dart$3acore.Object._identityHashCode$g(i64, ptr, ptr)
declare i64 @df.dart$3acore.Object.hashCode$g(i64, ptr, ptr)
declare i64 @df.dart$3acore.Object.noSuchMethod$c(i64, ptr, ptr)
declare i64 @df.dart$3acore.Object.noSuchMethod$tc(i64, ptr, ptr)
declare i64 @df.dart$3acore.Object.runtimeType$g(i64, ptr, ptr)
declare i64 @df.dart$3acore.Object.toString$c(i64, ptr, ptr)
declare i64 @df.dart$3aio.Platform.lineTerminator()
declare ptr @df.mt.dart$3acore.ConcurrentModificationError()
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
declare void @df.register.sdk_tables()


@dartforge_context_slot = external dso_local global i64, align 8
define internal ptr @df.context() nounwind willreturn memory(none) alwaysinline {
  %o = load i64, ptr @dartforge_context_slot, align 8, !invariant.load !{}
  %t = call i64 asm "mrs $0, TPIDRRO_EL0", "=r"() nounwind memory(none)
  %b = and i64 %t, -8
  %a = add i64 %b, %o
  %p = inttoptr i64 %a to ptr
  %c = load ptr, ptr %p, align 8, !invariant.load !{}
  ret ptr %c
}

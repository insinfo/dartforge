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
declare void @llvm.memcpy.p0.p0.i64(ptr, ptr, i64, i1)
declare void @llvm.memset.p0.i64(ptr, i8, i64, i1)
declare ptr @llvm.stacksave.p0()
declare void @llvm.stackrestore.p0(ptr)
declare void @df.dart$3acore..print(i64)
declare i64 @df.dart$3acore.Object.$3d$3d$c(i64, ptr, ptr)
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
declare i64 @df.dart$3acore.Object.toString$c(i64, ptr, ptr)
declare i64 @df.dart$3acore.Object.toString$g(i64, ptr, ptr)
define linkonce_odr i64 @dfc.e58fd95a432b9d0a.get() {
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
  store { ptr, i64, [1 x i64] } { ptr null, i64 1, [1 x i64] zeroinitializer }, ptr %gcq
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
  %v3 = add i64 0, 7
  %ta4 = getelementptr inbounds i8, ptr %ctx, i64 80
  %tb4 = load ptr, ptr %ta4, align 8
  %tfa4 = getelementptr inbounds i8, ptr %ctx, i64 88
  %tf4 = load ptr, ptr %tfa4, align 8
  %tnx4 = getelementptr i8, ptr %tb4, i64 24
  %tz4 = icmp ugt ptr %tnx4, %tf4
  %tnp4 = getelementptr inbounds i8, ptr %ctx, i64 56
  %tn4 = load i64, ptr %tnp4, align 8
  %tk4 = icmp ule i64 %tn4, 716
  %tq4 = or i1 %tz4, %tk4
  %tx4 = call i1 @llvm.expect.i1(i1 %tq4, i1 false)
  br i1 %tx4, label %ao4.lento, label %ao4.reg
ao4.reg:
  %trp4 = getelementptr inbounds i8, ptr %ctx, i64 48
  %tr4 = load ptr, ptr %trp4, align 8
  %trb4 = getelementptr inbounds i8, ptr %tr4, i64 716
  %trv4 = load i8, ptr %trb4, align 1
  %trz4 = icmp eq i8 %trv4, 0
  %try4 = call i1 @llvm.expect.i1(i1 %trz4, i1 false)
  br i1 %try4, label %ao4.lento, label %ao4.rapido
ao4.rapido:
  store ptr %tnx4, ptr %ta4, align 8
  %tpp4 = getelementptr inbounds i8, ptr %tb4, i64 16
  store i64 0, ptr %tpp4, align 8
  store i64 3075196649473, ptr %tb4, align 8
  %thb4 = ptrtoint ptr %tb4 to i64
  %th4 = add i64 %thb4, 2
  br label %ao4.fim
ao4.lento:
  %tl4 = call i64 @dartforge_object_new_t(i64 716, i64 1, ptr @df.mt.main$2edart.Leaf)
  br label %ao4.fim
ao4.fim:
  %v4 = phi i64 [ %th4, %ao4.rapido ], [ %tl4, %ao4.lento ]
  store i64 %v4, ptr %gcs0
  %fxa12 = add i64 %v4, -2
  %fcb12 = inttoptr i64 %fxa12 to ptr
  %fp12 = getelementptr inbounds i8, ptr %fcb12, i64 16
  %fg12 = getelementptr inbounds i64, ptr %fp12, i64 0
  store i64 %v3, ptr %fg12, align 8
  %fmp12 = getelementptr inbounds i8, ptr %fcb12, i64 8
  %fm12 = load i32, ptr %fmp12, align 4
  %fmc12 = and i32 %fm12, -2
  store i32 %fmc12, ptr %fmp12, align 4
  %v8 = call i64 @dartforge_canonical_constant(i64 %v4)
  store i64 %v8, ptr %gcs0
  %ga9 = getelementptr i64, ptr %area, i64 0
  store i64 %v8, ptr %ga9
  %gr9 = ptrtoint ptr %ga9 to i64
  call void @dartforge_gc_global_root(i64 %gr9, i64 %v8)
  call void @dartforge_mark_constant(i64 %v8, i64 ptrtoint (ptr @dfc.e58fd95a432b9d0a.get to i64))
  %ga11 = getelementptr i64, ptr %area, i64 1
  store i8 1, ptr %ga11
  %gcvolta2 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2, ptr %ctxtopo, align 8
  ret i64 %v8
}
define linkonce_odr i64 @dfc.c1ac273980e12938.get() {
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
  store { ptr, i64, [1 x i64] } { ptr null, i64 1, [1 x i64] zeroinitializer }, ptr %gcq
  %gcs0 = getelementptr inbounds { ptr, i64, [1 x i64] }, ptr %gcq, i64 0, i32 2, i64 0
  %gcant = load ptr, ptr %ctxtopo, align 8
  store ptr %gcant, ptr %gcq, align 8
  store ptr %gcq, ptr %ctxtopo, align 8
  %ga0 = getelementptr i64, ptr %area, i64 3
  %v0 = load i8, ptr %ga0
  %c0 = zext i8 %v0 to i64
  %v1 = icmp ne i64 %c0, 0
  br i1 %v1, label %b1, label %b2
b1:
  %ga2 = getelementptr i64, ptr %area, i64 2
  %v2 = load i64, ptr %ga2
  %gcvolta1 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1, ptr %ctxtopo, align 8
  ret i64 %v2
b2:
  %ta3 = getelementptr inbounds i8, ptr %ctx, i64 80
  %tb3 = load ptr, ptr %ta3, align 8
  %tfa3 = getelementptr inbounds i8, ptr %ctx, i64 88
  %tf3 = load ptr, ptr %tfa3, align 8
  %tnx3 = getelementptr i8, ptr %tb3, i64 24
  %tz3 = icmp ugt ptr %tnx3, %tf3
  %tnp3 = getelementptr inbounds i8, ptr %ctx, i64 56
  %tn3 = load i64, ptr %tnp3, align 8
  %tk3 = icmp ule i64 %tn3, 717
  %tq3 = or i1 %tz3, %tk3
  %tx3 = call i1 @llvm.expect.i1(i1 %tq3, i1 false)
  br i1 %tx3, label %ao3.lento, label %ao3.reg
ao3.reg:
  %trp3 = getelementptr inbounds i8, ptr %ctx, i64 48
  %tr3 = load ptr, ptr %trp3, align 8
  %trb3 = getelementptr inbounds i8, ptr %tr3, i64 717
  %trv3 = load i8, ptr %trb3, align 1
  %trz3 = icmp eq i8 %trv3, 0
  %try3 = call i1 @llvm.expect.i1(i1 %trz3, i1 false)
  br i1 %try3, label %ao3.lento, label %ao3.rapido
ao3.rapido:
  store ptr %tnx3, ptr %ta3, align 8
  %tpp3 = getelementptr inbounds i8, ptr %tb3, i64 16
  store i64 0, ptr %tpp3, align 8
  store i64 3079491616769, ptr %tb3, align 8
  %thb3 = ptrtoint ptr %tb3 to i64
  %th3 = add i64 %thb3, 2
  br label %ao3.fim
ao3.lento:
  %tl3 = call i64 @dartforge_object_new_t(i64 717, i64 1, ptr @df.mt.main$2edart.Parent)
  br label %ao3.fim
ao3.fim:
  %v3 = phi i64 [ %th3, %ao3.rapido ], [ %tl3, %ao3.lento ]
  store i64 %v3, ptr %gcs0
  %v11 = call i64 @dfc.e58fd95a432b9d0a.get()
  %v12 = load i8, ptr %ctx, align 8
  %c1 = zext i8 %v12 to i64
  %v13 = icmp ne i64 %c1, 0
  br i1 %v13, label %b7, label %b6
b6:
  %fxk14 = and i64 %v3, -9223372036854775801
  %fxo14 = icmp eq i64 %fxk14, 2
  %fxa14 = add i64 %v3, -2
  %fxq14 = inttoptr i64 %fxa14 to ptr
  %fxvp14 = getelementptr inbounds i8, ptr %ctx, i64 40
  %fxv14 = load ptr, ptr %fxvp14, align 8
  %fxh14 = select i1 %fxo14, ptr %fxq14, ptr %fxv14
  %fxfp14 = getelementptr inbounds i8, ptr %fxh14, i64 1
  %fxf14 = load i8, ptr %fxfp14, align 1
  %fxfb14 = and i8 %fxf14, 1
  %fxfo14 = icmp ne i8 %fxfb14, 0
  %fxcp14 = getelementptr inbounds i8, ptr %fxh14, i64 16
  %fxc14 = load ptr, ptr %fxcp14, align 8
  %fcb14 = select i1 %fxfo14, ptr %fxc14, ptr %fxh14
  %fp14 = getelementptr inbounds i8, ptr %fcb14, i64 16
  %fg14 = getelementptr inbounds i64, ptr %fp14, i64 0
  store i64 %v11, ptr %fg14, align 8
  %fmp14 = getelementptr inbounds i8, ptr %fcb14, i64 8
  %fm14 = load i32, ptr %fmp14, align 4
  %fmc14 = and i32 %fm14, -2
  %fms14 = or i32 %fm14, 1
  store i32 %fms14, ptr %fmp14, align 4
  call void @df.barrier(i64 %v3, i64 %v11)
  br label %wb14.fim
wb14.fim:
  br label %b8
b7:
  br label %b8
b8:
  %v5 = load i8, ptr %ctx, align 8
  %c2 = zext i8 %v5 to i64
  %v6 = icmp ne i64 %c2, 0
  br i1 %v6, label %b4, label %b3
b3:
  %v7 = call i64 @dartforge_canonical_constant(i64 %v3)
  store i64 %v7, ptr %gcs0
  %ga8 = getelementptr i64, ptr %area, i64 2
  store i64 %v7, ptr %ga8
  %gr8 = ptrtoint ptr %ga8 to i64
  call void @dartforge_gc_global_root(i64 %gr8, i64 %v7)
  call void @dartforge_mark_constant(i64 %v7, i64 ptrtoint (ptr @dfc.c1ac273980e12938.get to i64))
  %ga10 = getelementptr i64, ptr %area, i64 3
  store i8 1, ptr %ga10
  %gcvolta3 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta3, ptr %ctxtopo, align 8
  ret i64 %v7
b4:
  %gcvolta4 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta4, ptr %ctxtopo, align 8
  ret i64 0
}
declare i64 @df.box_int(i64)
declare void @df.barrier(i64, i64)
declare ptr @df.get_area()
declare ptr @df.mt.main$2edart.Leaf()
declare ptr @df.mt.main$2edart.Parent()
declare ptr @df.mt.$type()
define i64 @cache_test_parent() {
  %value = call i64 @dfc.c1ac273980e12938.get()
  ret i64 %value
}

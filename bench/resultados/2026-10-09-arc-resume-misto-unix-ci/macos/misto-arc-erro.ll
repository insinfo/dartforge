; Declarações do runtime nativo Rust (tabela em llvm/externs.rs)
declare i64 @dartforge_string_new(ptr, i64)
declare i64 @dartforge_string_concat(i64, i64)
declare i8 @dartforge_equal(i64, i64)
declare void @dartforge_register_class_name(i64, ptr, i64)
declare i64 @dartforge_object_new(i64, i64)
declare i64 @dartforge_object_get(i64, i64)
declare i64 @dartforge_rastro_texto(i64)
declare void @dartforge_registrar_rastro(ptr, ptr)
declare void @dartforge_registrar_campo_do_rastro(i64, ptr, i64, i64)
declare void @dartforge_registrar_tearoff_do_rastro(ptr, i64)
declare i64 @dartforge_rastro_entrar(i64, i64)
declare void @dartforge_rastro_sair(i64)
declare void @dartforge_lembrar(i64) nounwind
declare void @dartforge_memoria_arc_v1()
declare void @dartforge_arc_gravar_ref(i64, i64, i64) nounwind
declare void @dartforge_arc_inicial(i64) nounwind
declare void @dartforge_arc_global_receber_v1(i64, i64) nounwind
declare i64 @dartforge_arc_box_int_owned_v1(i64) nounwind
declare void @dartforge_arc_lancar_ref_v1(i64)
declare i64 @dartforge_arc_quadro_abrir_v1(i64) nounwind
declare i64 @dartforge_arc_quadro_carregar_v1(i64, i64) nounwind
declare void @dartforge_arc_quadro_receber_v1(i64, i64, i64) nounwind
declare void @dartforge_arc_quadro_copiar_v1(i64, i64, i64) nounwind
declare void @dartforge_arc_quadro_mover_v1(i64, i64, i64, i64) nounwind
declare void @dartforge_arc_quadro_fechar_v1(i64) nounwind
declare void @dartforge_arc_retain(i64) nounwind
declare void @dartforge_arc_release(i64) nounwind
declare void @dartforge_arc_collect() nounwind
declare i8 @dartforge_arc_verificar_abi(i64) nounwind
declare void @dartforge_object_set(i64, i64, i64, i8)
declare i8 @dartforge_late_field_initialized(i64, i64)
declare void @dartforge_late_field_mark_initialized(i64, i64)
declare i64 @dartforge_tearoff(i64)
declare void @dartforge_gc_global_root(i64, i64)
declare void @dartforge_marcar_constante(i64, i64)
declare i8 @dartforge_exception_capturavel()
declare void @dartforge_registrar_isolados(ptr, ptr)
declare void @dartforge_ffi_registrar_tipo(i64, i64)
declare void @dartforge_ffi_registrar_trampolim(ptr, i64, ptr)
declare i64 @dartforge_ffi_endereco_da_closure(i64)
declare i64 @dartforge_ffi_endereco_do_ponteiro(i64)
declare i64 @dartforge_ffi_ponteiro_de_retorno(i64, i64)
declare i64 @dartforge_typed_novo(i64, i64, i64)
declare i64 @dartforge_typed_novo_t(i64, i64, i64, ptr)
declare ptr @dartforge_contexto() memory(none) nounwind willreturn speculatable
declare i64 @dartforge_typed_len(i64, i64, i64) memory(none) nounwind willreturn speculatable
declare i64 @dartforge_typed_fill_int(i64, i64, i64, i64, i64) nounwind
declare i64 @dartforge_typed_fill_double(i64, i64, i64, i64, double) nounwind
declare i64 @dartforge_string_juntar_tipado(ptr, i64) nounwind
declare i64 @dartforge_closure_tipada(i64, i64) memory(inaccessiblemem: read) nounwind willreturn
declare i64 @dartforge_object_campos(i64) memory(inaccessiblemem: read) nounwind willreturn speculatable
declare i64 @dartforge_view_nova(i64, i64, i64, i64, i64)
declare i64 @dartforge_view_nova_t(i64, i64, i64, i64, i64, ptr)
declare i64 @dartforge_unbox_int(i64)
declare double @dartforge_unbox_double(i64)
declare i8 @dartforge_unbox_bool(i64)
declare i64 @dartforge_exception_peek_ref()
declare void @dartforge_exception_throw(i64, i8)
declare i8 @dartforge_interrupcao_pendente()
declare void @dartforge_ponto_seguro()
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
declare i64 @dartforge_type_error_new()
declare i64 @dartforge_null_check_error_new()
declare i64 @dartforge_late_error_new(i64, i64)
declare i64 @dartforge_stack_overflow_error_new()
declare void @dartforge_estouro_de_pilha()
declare i8 @dartforge_late_field_initializing(i64, i64)
declare void @dartforge_late_field_set_initializing(i64, i64, i8)
declare i64 @dartforge_no_such_method_error_new(i64)
declare i64 @dartforge_closure_entry(i64)
declare i8 @dartforge_args_casam(ptr, ptr)
declare i64 @dartforge_arg_indice(ptr, i64)
declare void @dartforge_nsm_chamada()
declare i64 @dartforge_dyn_op(i64, i64, i64)
declare i64 @dartforge_iteravel_get_ref(i64, i64)
declare i64 @dartforge_iteravel_get_bits(i64, i64)
declare ptr @dartforge_seletor(ptr, i64, i64, ptr, i64)
declare void @dartforge_registrar_metodos(i64, ptr, i64)
declare void @dartforge_membro_recusado(i64)
declare void @dartforge_registrar_cids(ptr, i64)
declare void @dartforge_registrar_ajudante(ptr, i64, ptr)
declare void @dartforge_registrar_nome_de_argumento(ptr, i64)
declare i64 @dartforge_encaminhar_nsm(i64, i64, i64, i64, i64, i64, i64)
declare void @dartforge_registrar_versao_do_sdk(ptr, i64)
declare void @dartforge_preparar_embedder()
declare i64 @dartforge_argumentos_do_main()
declare ptr @dartforge_area_de_globais(ptr)
declare ptr @dartforge_area_de_globais_id(ptr, ptr)
declare i32 @dartforge_iniciar(ptr, ptr)
declare void @dartforge_rastro_entrada(ptr, i64)
declare void @dartforge_rastro_saida()
declare i64 @dartforge_object_new_t(i64, i64, ptr)
declare void @dartforge_registrar_tabela(i64, ptr)
declare void @dartforge_laco_de_eventos(ptr)
declare void @dartforge_nativo_DartForge_scheduleImmediate(i64)
declare i64 @dartforge_nativo_DartForge_Timer_novo(i64, i64, i8)
declare void @dartforge_nativo_DartForge_Timer_cancelar(i64)
declare i64 @dartforge_nativo_Error_throwWithStackTrace(i64, i64)
declare void @dartforge_nativo_Error_trySetStackTrace(i64, i64)
declare void @dartforge_rti_iniciar_tabela(ptr, i64)
declare i64 @dartforge_lista_de_tabela(ptr, i64)
declare i64 @dartforge_lista_de_tabela_g(ptr, i64, ptr)
declare void @dartforge_ffi_registrar_composto(i64, i64, i64, i64, i64, i64, i64)
declare i64 @dartforge_ffi_composto(i64, i64, i64)
declare i64 @dartforge_ffi_ponteiro_novo(i64)
declare i64 @dartforge_ffi_endereco_do_composto(i64)
declare i64 @dartforge_ffi_composto_novo(i64)
declare i64 @dartforge_ffi_handles_abrir()
declare void @dartforge_ffi_handles_fechar(i64)
declare i64 @dartforge_ffi_handle_novo(i64)
declare i64 @dartforge_ffi_objeto_do_handle(i64)
declare i64 @dartforge_ffi_composto_copia(i64, i64)
declare void @dartforge_ffi_copiar_composto(ptr, i64, i64)
declare void @dartforge_ffi_registrar_callback(ptr, i64, ptr)
declare i64 @dartforge_ffi_callback_entrar(ptr)
declare i8 @dartforge_ffi_callback_sair(ptr, ptr)
declare void @dartforge_ffi_callback_postar(ptr, ptr, i64)
declare i64 @dartforge_ffi_callback_closure(i64)
declare i64 @dartforge_ffi_callback_ponteiro(i64, i64, i64)
declare i64 @dartforge_ffi_simbolo_nativo(ptr, i64)
declare i64 @dartforge_typed_externo(i64, i64, i64, i64)
declare i64 @dartforge_typed_externo_t(i64, i64, i64, i64, ptr)
declare i64 @dartforge_rti_receita(i64)
declare i64 @dartforge_rti_avaliar(i64, i64, i64, i64)
declare i64 @dartforge_rti_avaliar_cache(ptr, i64, i64, i64)
declare i64 @dartforge_rti_tupla_juntar(i64, i64, i64, i64)
declare void @dartforge_rti_definir(i64, i64)
declare void @dartforge_rti_registro_nomeado(i64, i64, i64)
declare i8 @dartforge_rti_e(i64, i64)
declare void @dartforge_rti_como_em(i64, i64, i64, i64)
declare i64 @dartforge_rti_texto(i64)
declare i64 @dartforge_rti_objeto_tipo(i64)
declare i64 @dartforge_alocar(i64, i64, i64) nounwind
declare i64 @dartforge_texto_novo(i64, i64) nounwind
declare i64 @dartforge_constante_canonica(i64) nounwind
declare i64 @dartforge_texto_hash(i64) nounwind willreturn
declare i8 @dartforge_texto_iguais(i64, i64) memory(read) nounwind willreturn
declare i64 @dartforge_lista_nova(ptr, i64, i64, i64) nounwind
declare i64 @dartforge_lista_acrescentar(i64, i64) nounwind
declare void @dartforge_registrar_imagem(ptr, ptr) nounwind
declare i64 @dartforge_record_novo(ptr, i64) nounwind
declare <4 x float> @llvm.fabs.v4f32(<4 x float>)
declare <2 x double> @llvm.fabs.v2f64(<2 x double>)
declare <4 x float> @llvm.sqrt.v4f32(<4 x float>)
declare <2 x double> @llvm.sqrt.v2f64(<2 x double>)
declare <4 x i32> @llvm.abs.v4i32(<4 x i32>, i1)
declare i8 @llvm.expect.i8(i8, i8)
declare i1 @llvm.expect.i1(i1, i1)
define internal i64 @df.texto_len(i64 %s) alwaysinline nounwind {
  %p = inttoptr i64 %s to ptr
  %q = getelementptr inbounds i8, ptr %p, i64 14
  %n = load i64, ptr %q, align 8
  ret i64 %n
}
define internal i64 @df.texto_unidade(i64 %s, i64 %i) alwaysinline nounwind {
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
define internal i64 @df.texto_alocar(i64 %len, i1 %dois) alwaysinline {
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
  %h = call i64 @df.alocar(i64 %cab, i64 %w)
  %p = inttoptr i64 %h to ptr
  %q = getelementptr inbounds i8, ptr %p, i64 14
  store i64 %len, ptr %q, align 8
  ret i64 %h
lento:
  %hl = call i64 @dartforge_texto_novo(i64 %len, i64 %d)
  ret i64 %hl
}
define internal void @df.texto_gravar(i64 %s, i64 %i, i64 %x) alwaysinline nounwind {
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
define internal i64 @df.texto_hash(i64 %s) alwaysinline nounwind {
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
  %l = call i64 @dartforge_texto_hash(i64 %s)
  ret i64 %l
}
define internal i1 @df.texto_igual(i64 %a, i64 %b) alwaysinline nounwind {
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
  %r = call i8 @dartforge_texto_iguais(i64 %a, i64 %b)
  %rb1 = icmp ne i8 %r, 0
  ret i1 %rb1
sim:
  ret i1 true
nao:
  ret i1 false
}
define internal i1 @df.texto_igual_a(i64 %a, i64 %b) alwaysinline nounwind {
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
  %r = call i1 @df.texto_igual(i64 %a, i64 %b)
  ret i1 %r
sim:
  ret i1 true
nao:
  ret i1 false
}
define internal i64 @df.caixa_int(i64 %v) alwaysinline {
%a = add i64 %v, 4611686018427387904
%ok = icmp ult i64 %a, -9223372036854775808
br i1 %ok, label %smi, label %heap
smi:
%s = shl i64 %v, 1
%r = or i64 %s, 1
ret i64 %r
heap:
%h = call i64 @df.alocar(i64 12884967937, i64 1)
%p = inttoptr i64 %h to ptr
%q = getelementptr inbounds i8, ptr %p, i64 14
store i64 %v, ptr %q, align 8
ret i64 %h
}
define internal i64 @df.desencaixa_int(i64 %r) alwaysinline {
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
define internal i64 @df.caixa_double(double %d) alwaysinline {
%h = call i64 @df.alocar(i64 17179935233, i64 1)
%p = inttoptr i64 %h to ptr
%q = getelementptr inbounds i8, ptr %p, i64 14
store double %d, ptr %q, align 8
ret i64 %h
}
define internal double @df.desencaixa_double(i64 %r) alwaysinline {
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
define internal double @df.num_para_double(i64 %r) alwaysinline {
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
%d = call double @df.desencaixa_double(i64 %r)
ret double %d
}
define internal i64 @df.caixa_bool(i1 %b) alwaysinline {
%ctx = call ptr @dartforge_contexto()
%o = select i1 %b, i64 360, i64 368
%p = getelementptr inbounds i8, ptr %ctx, i64 %o
%r = load i64, ptr %p, align 8, !invariant.load !{}
ret i64 %r
}
define internal i1 @df.desencaixa_bool(i64 %r) alwaysinline {
%ctx = call ptr @dartforge_contexto()
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
define internal i1 @df.identico(i64 %a, i64 %b) alwaysinline {
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
define internal i64 @df.lista_len(i64 %l) alwaysinline {
%p = inttoptr i64 %l to ptr
%q = getelementptr inbounds i8, ptr %p, i64 14
%n = load i64, ptr %q, align 8
ret i64 %n
}
define internal i64 @df.lista_armazenamento(i64 %l) alwaysinline {
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
define internal ptr @df.lista_elemento(i64 %a, i64 %i) alwaysinline {
%p = inttoptr i64 %a to ptr
%b = getelementptr inbounds i8, ptr %p, i64 22
%e = getelementptr inbounds i64, ptr %b, i64 %i
ret ptr %e
}
define internal i8 @df.lista_forma(i64 %a) alwaysinline {
%p = inttoptr i64 %a to ptr
%fp = getelementptr inbounds i8, ptr %p, i64 -1
%f = load i8, ptr %fp, align 1
%r = and i8 %f, 102
ret i8 %r
}
define internal i64 @df.lista_capacidade(i64 %l) alwaysinline {
%a = call i64 @df.lista_armazenamento(i64 %l)
%n = call i64 @df.lista_len(i64 %a)
ret i64 %n
}
define internal i64 @df.lista_ler(i64 %a, i64 %i) alwaysinline {
%e = call ptr @df.lista_elemento(i64 %a, i64 %i)
%v = load i64, ptr %e, align 8
ret i64 %v
}
define internal void @df.lista_gravar_ref(i64 %a, i64 %i, i64 %v) alwaysinline {
%p = add i64 %i, 1
call void @dartforge_arc_gravar_ref(i64 %a, i64 %p, i64 %v)
ret void
}
define internal void @df.lista_gravar_len(i64 %l, i64 %n) alwaysinline {
%p = inttoptr i64 %l to ptr
%q = getelementptr inbounds i8, ptr %p, i64 14
store i64 %n, ptr %q, align 8
ret void
}
define internal i64 @df.lista_gravar_dados(i64 %l, i64 %d) alwaysinline {
entrada:
%a = call i64 @df.lista_armazenamento(i64 %l)
%fa = call i8 @df.lista_forma(i64 %a)
%fd = call i8 @df.lista_forma(i64 %d)
%igual = icmp eq i8 %fa, %fd
br i1 %igual, label %grava, label %fim
grava:
call void @dartforge_object_set(i64 %l, i64 1, i64 %d, i8 1)
br label %fim
fim:
%r = phi i64 [ 1, %grava ], [ 0, %entrada ]
ret i64 %r
}
define internal i64 @df.tipada_len(i64 %t) alwaysinline {
  %a = add i64 %t, 14
  %p = inttoptr i64 %a to ptr
  %n = load i64, ptr %p, align 8, !invariant.load !{}
  ret i64 %n
}
define internal i64 @df.tipada_dados(i64 %t) alwaysinline {
  %a = add i64 %t, 22
  %p = inttoptr i64 %a to ptr
  %d = load i64, ptr %p, align 8, !invariant.load !{}
  ret i64 %d
}
define internal i64 @df.tipada_cid(i64 %t) alwaysinline {
  %a = add i64 %t, 2
  %p = inttoptr i64 %a to ptr
  %c = load i32, ptr %p, align 4, !invariant.load !{}
  %r = sext i32 %c to i64
  ret i64 %r
}
define internal i64 @df.tipada_len_gravavel(i64 %t, i64 %imutavel) alwaysinline {
  %c = call i64 @df.tipada_cid(i64 %t)
  %n = call i64 @df.tipada_len(i64 %t)
  %i = icmp eq i64 %c, %imutavel
  %r = select i1 %i, i64 0, i64 %n
  ret i64 %r
}
define internal i64 @df.tipada_bytes(i64 %t) alwaysinline {
  %c = call i64 @df.tipada_cid(i64 %t)
  %n = call i64 @df.tipada_len(i64 %t)
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
define internal i64 @df.tipada_base(i64 %t) alwaysinline {
  %a = add i64 %t, 30
  %p = inttoptr i64 %a to ptr
  %b = load i64, ptr %p, align 8, !invariant.load !{}
  ret i64 %b
}
define internal i64 @df.tipada_deslocamento(i64 %t) alwaysinline {
  %a = add i64 %t, 38
  %p = inttoptr i64 %a to ptr
  %d = load i64, ptr %p, align 8, !invariant.load !{}
  ret i64 %d
}
define internal i64 @df.nucleo_len(i64 %l) alwaysinline {
entrada:
  %c = call i64 @df.classe(i64 %l)
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
define internal i64 @df.nucleo_armazenamento(i64 %l) alwaysinline {
entrada:
  %c = call i64 @df.classe(i64 %l)
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
define internal i64 @df.nucleo_forma(i64 %a) alwaysinline {
  %f = add i64 %a, -1
  %p = inttoptr i64 %f to ptr
  %b = load i8, ptr %p, align 1
  %m = and i8 %b, 102
  %r = zext i8 %m to i64
  ret i64 %r
}
define internal i64 @df.nucleo_len_gravavel(i64 %l, i64 %forma) alwaysinline {
entrada:
  %c = call i64 @df.classe(i64 %l)
  %fixa = icmp eq i64 %c, 8
  %exp = icmp eq i64 %c, 10
  %mod = or i1 %fixa, %exp
  br i1 %mod, label %forma.conf, label %fim
forma.conf:
  %a = call i64 @df.nucleo_armazenamento(i64 %l)
  %f = call i64 @df.nucleo_forma(i64 %a)
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
define internal i64 @df.palavra_ref(i64 %e, i64 %i) alwaysinline {
  %o = shl i64 %i, 3
  %a = add i64 %e, %o
  %p = inttoptr i64 %a to ptr
  %r = load i64, ptr %p, align 8
  ret i64 %r
}
define internal i64 @df.simd_caixa(<2 x i64> %v, i64 %cid) alwaysinline {
  %c = shl i64 %cid, 32
  %cab = or i64 %c, 131585
  %h = call i64 @df.alocar(i64 %cab, i64 2)
  %a = add i64 %h, 14
  %p = inttoptr i64 %a to ptr
  store <2 x i64> %v, ptr %p, align 8
  ret i64 %h
}
define internal i64 @df.classe(i64 %h) alwaysinline {
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
define internal ptr @df.seletor(ptr %c, i64 %r, i64 %h, ptr %n, i64 %l) alwaysinline {
%cid = call i64 @df.classe(i64 %r)
%k = add i64 %cid, 1
%c0 = load i64, ptr %c, align 8
%sim = icmp eq i64 %c0, %k
br i1 %sim, label %acerto, label %falha
acerto:
%ep = getelementptr inbounds i64, ptr %c, i64 1
%e = load ptr, ptr %ep, align 8
ret ptr %e
falha:
%f = call ptr @dartforge_seletor(ptr %c, i64 %r, i64 %h, ptr %n, i64 %l)
ret ptr %f
}
define internal i8 @df.subclasse(i64 %cid, i64 %alvo) alwaysinline {
%ctx = call ptr @dartforge_contexto()
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
define internal i1 @df.e_objeto(i64 %h) alwaysinline {
%m = and i64 %h, -9223372036854775801
%r = icmp eq i64 %m, 2
ret i1 %r
}
define internal i1 @df.filho_jovem(i64 %v) alwaysinline {
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
%j = icmp eq i8 %e, 1
ret i1 %j
nao:
ret i1 false
}
define internal void @df.barreira(i64 %o, i64 %v) alwaysinline {
%p = inttoptr i64 %o to ptr
%ep = getelementptr inbounds i8, ptr %p, i64 -2
%e = load i8, ptr %ep, align 8
%velho = icmp eq i8 %e, 3
%x = call i1 @llvm.expect.i1(i1 %velho, i1 false)
br i1 %x, label %filho, label %fim
filho:
%j = call i1 @df.filho_jovem(i64 %v)
br i1 %j, label %lembrar, label %fim
lembrar:
call void @dartforge_lembrar(i64 %o)
br label %fim
fim:
ret void
}
define internal void @df.barreira_elemento(i64 %o, i64 %i, i64 %v) alwaysinline {
%p = inttoptr i64 %o to ptr
%ep = getelementptr inbounds i8, ptr %p, i64 -2
%e = load i8, ptr %ep, align 8
%e3 = sub i8 %e, 3
%velho = icmp ult i8 %e3, 2
%x = call i1 @llvm.expect.i1(i1 %velho, i1 false)
br i1 %x, label %filho, label %fim
filho:
%j = call i1 @df.filho_jovem(i64 %v)
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
call void @dartforge_lembrar(i64 %o)
br label %fim
fim:
ret void
}
define internal i64 @df.alocar(i64 %cab, i64 %w) alwaysinline {
%pequeno = icmp ule i64 %w, 16
br i1 %pequeno, label %tlab, label %lento
tlab:
%ctx = call ptr @dartforge_contexto()
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
%r = call i64 @dartforge_alocar(i64 %cid, i64 %w, i64 %fn)
ret i64 %r
}
define internal ptr @df.corpo(i64 %o, ptr %ctx) alwaysinline {
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
; Exceções por tabelas (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md)
declare i32 @dartforge_personalidade(...)
declare i32 @dartforge_personalidade_cleanup_itanium(...)
declare i32 @llvm.eh.typeid.for(ptr)
declare void @dartforge_registrar_portas(ptr, i64)
declare ptr @dartforge_objeto_de_desenrolamento()
declare i32 @_Unwind_RaiseException(ptr)
declare void @dartforge_desenrolamento_falhou(i32) noreturn
define internal void @df.lancar() noreturn noinline cold "gc-leaf-function" {
  %o = call ptr @dartforge_objeto_de_desenrolamento()
  %r = call i32 @_Unwind_RaiseException(ptr %o)
  call void @dartforge_desenrolamento_falhou(i32 %r)
  unreachable
}

; Closures: entrada inválida e vetores constantes
define internal i64 @df_clo_invalido(i64 %c, ptr %a, ptr %d) {
b0:
  ret i64 0
}

define void @prova_guardas() personality ptr @dartforge_personalidade_cleanup_itanium {
b0:
  %gcq = alloca { ptr, i64, [2 x i64] }, align 8
  %ctx = call ptr @dartforge_contexto()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhaq = alloca i8, align 1
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load ptr, ptr %pilhalp, align 8
  %pilhab = icmp ult ptr %pilhaq, %pilhal
  %pilhax = call i1 @llvm.expect.i1(i1 %pilhab, i1 false)
  br i1 %pilhax, label %pilha.estouro, label %pilha.ok
pilha.estouro:
  call void @dartforge_estouro_de_pilha()
  call void @df.lancar()
  unreachable
pilha.ok:
  store { ptr, i64, [2 x i64] } { ptr null, i64 2, [2 x i64] zeroinitializer }, ptr %gcq
  %gcs0 = getelementptr inbounds { ptr, i64, [2 x i64] }, ptr %gcq, i64 0, i32 2, i64 0
  %gcs1 = getelementptr inbounds { ptr, i64, [2 x i64] }, ptr %gcq, i64 0, i32 2, i64 1
  %gcant = load ptr, ptr %ctxtopo, align 8
  store ptr %gcant, ptr %gcq, align 8
  store ptr %gcq, ptr %ctxtopo, align 8
  %v0 = call i64 @dartforge_arc_box_int_owned_v1(i64 9223372036854775807)
  store i64 %v0, ptr %gcs0
  %v1 = invoke i64 @propagador(i64 %v0) to label %inv1.fim unwind label %b1
inv1.fim:
  store i64 %v1, ptr %gcs1
  br label %b2
b1:
  %lpad1 = landingpad { ptr, i32 } catch ptr null
  store ptr %gcq, ptr %ctxtopo, align 8
  %lpseletor1 = extractvalue { ptr, i32 } %lpad1, 1
  %lptipo1 = call i32 @llvm.eh.typeid.for(ptr null)
  %lpdart1 = icmp eq i32 %lpseletor1, %lptipo1
  br i1 %lpdart1, label %lpad1.dart, label %lpad1.estrangeira
lpad1.estrangeira:
  resume { ptr, i32 } %lpad1
lpad1.dart:
  %xcp2 = load i8, ptr %ctx, align 8
  %xcn2 = icmp ne i8 %xcp2, 0
  %xce2 = call i1 @llvm.expect.i1(i1 %xcn2, i1 false)
  br i1 %xce2, label %xc2.limpar, label %xc2.fim
xc2.limpar:
  call void @dartforge_exception_clear()
  %xcd2 = load i8, ptr %ctx, align 8
  %xcq2 = icmp ne i8 %xcd2, 0
  br i1 %xcq2, label %xc2.sai, label %xc2.fim
xc2.sai:
  call void @df.lancar()
  unreachable
xc2.fim:
  call void @dartforge_arc_release(i64 %v0)
  %gcvolta1 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1, ptr %ctxtopo, align 8
  ret void
b2:
  call void @dartforge_arc_release(i64 %v1)
  call void @dartforge_arc_collect()
  call void @dartforge_print_handle(i64 %v0)
  %v5 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v5 to i64
  %v6 = icmp ne i64 %c0, 0
  br i1 %v6, label %b3, label %b4
b3:
  %xcp7 = load i8, ptr %ctx, align 8
  %xcn7 = icmp ne i8 %xcp7, 0
  %xce7 = call i1 @llvm.expect.i1(i1 %xcn7, i1 false)
  br i1 %xce7, label %xc7.limpar, label %xc7.fim
xc7.limpar:
  call void @dartforge_exception_clear()
  %xcd7 = load i8, ptr %ctx, align 8
  %xcq7 = icmp ne i8 %xcd7, 0
  br i1 %xcq7, label %xc7.sai, label %xc7.fim
xc7.sai:
  call void @df.lancar()
  unreachable
xc7.fim:
  call void @dartforge_arc_release(i64 %v0)
  %gcvolta3 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta3, ptr %ctxtopo, align 8
  ret void
b4:
  %v9 = invoke i64 @propagador(i64 %v0) to label %inv9.fim unwind label %b5
inv9.fim:
  store i64 %v9, ptr %gcs1
  br label %b6
b5:
  %lpad5 = landingpad { ptr, i32 } cleanup
  store ptr %gcq, ptr %ctxtopo, align 8
  call void @dartforge_arc_release(i64 %v0)
  resume { ptr, i32 } %lpad5
b6:
  call void @dartforge_arc_release(i64 %v0)
  call void @dartforge_arc_release(i64 %v9)
  %gcvolta6 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta6, ptr %ctxtopo, align 8
  ret void
}

define i64 @retorno_guardado(i64 %v0) {
b0:
  %gcq = alloca { ptr, i64, [1 x i64] }, align 8
  %ctx = call ptr @dartforge_contexto()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhaq = alloca i8, align 1
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load ptr, ptr %pilhalp, align 8
  %pilhab = icmp ult ptr %pilhaq, %pilhal
  %pilhax = call i1 @llvm.expect.i1(i1 %pilhab, i1 false)
  br i1 %pilhax, label %pilha.estouro, label %pilha.ok
pilha.estouro:
  call void @dartforge_estouro_de_pilha()
  call void @df.lancar()
  unreachable
pilha.ok:
  store { ptr, i64, [1 x i64] } { ptr null, i64 1, [1 x i64] zeroinitializer }, ptr %gcq
  %gcs0 = getelementptr inbounds { ptr, i64, [1 x i64] }, ptr %gcq, i64 0, i32 2, i64 0
  %gcant = load ptr, ptr %ctxtopo, align 8
  store ptr %gcant, ptr %gcq, align 8
  store ptr %gcq, ptr %ctxtopo, align 8
  store i64 %v0, ptr %gcs0
  call void @dartforge_arc_lancar_ref_v1(i64 %v0)
  %v2 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v2 to i64
  %v3 = icmp ne i64 %c0, 0
  br i1 %v3, label %b1, label %b2
b1:
  call void @df.lancar()
  unreachable
b2:
  call void @dartforge_arc_retain(i64 %v0)
  %v4 = add i64 %v0, 0
  %gcvolta2 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2, ptr %ctxtopo, align 8
  ret i64 %v4
}

define i64 @propagador(i64 %v0) personality ptr @dartforge_personalidade_cleanup_itanium {
b0:
  %gcq = alloca { ptr, i64, [2 x i64] }, align 8
  %ctx = call ptr @dartforge_contexto()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhaq = alloca i8, align 1
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load ptr, ptr %pilhalp, align 8
  %pilhab = icmp ult ptr %pilhaq, %pilhal
  %pilhax = call i1 @llvm.expect.i1(i1 %pilhab, i1 false)
  br i1 %pilhax, label %pilha.estouro, label %pilha.ok
pilha.estouro:
  call void @dartforge_estouro_de_pilha()
  call void @df.lancar()
  unreachable
pilha.ok:
  store { ptr, i64, [2 x i64] } { ptr null, i64 2, [2 x i64] zeroinitializer }, ptr %gcq
  %gcs0 = getelementptr inbounds { ptr, i64, [2 x i64] }, ptr %gcq, i64 0, i32 2, i64 0
  %gcs1 = getelementptr inbounds { ptr, i64, [2 x i64] }, ptr %gcq, i64 0, i32 2, i64 1
  %gcant = load ptr, ptr %ctxtopo, align 8
  store ptr %gcant, ptr %gcq, align 8
  store ptr %gcq, ptr %ctxtopo, align 8
  store i64 %v0, ptr %gcs0
  call void @dartforge_arc_retain(i64 %v0)
  %v1 = add i64 %v0, 0
  store i64 %v1, ptr %gcs0
  %v2 = invoke i64 @retorno_guardado(i64 %v1) to label %inv2.fim unwind label %b1
inv2.fim:
  store i64 %v2, ptr %gcs1
  br label %b2
b1:
  %lpad1 = landingpad { ptr, i32 } cleanup
  store ptr %gcq, ptr %ctxtopo, align 8
  call void @dartforge_arc_release(i64 %v1)
  resume { ptr, i32 } %lpad1
b2:
  call void @dartforge_arc_release(i64 %v1)
  %gcvolta2 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2, ptr %ctxtopo, align 8
  ret i64 %v2
}

define i64 @dartforge_dispatch_toString(i64 %obj) {
b0:
  %is_null = icmp eq i64 %obj, 0
  br i1 %is_null, label %ret_null, label %check_obj
ret_null:
  %null_s = call i64 @dartforge_to_string_handle(i64 0)
  ret i64 %null_s
check_obj:
  %cls = call i64 @df.classe(i64 %obj)
  br label %fallback
fallback:
  %fb = call i64 @dartforge_to_string_handle(i64 %obj)
  ret i64 %fb
}

define internal void @df.porta.v0(ptr %f) personality ptr @dartforge_personalidade {
entrada:
  %ctx = call ptr @dartforge_contexto()
  %tp = getelementptr inbounds i8, ptr %ctx, i64 8
  %topo = load ptr, ptr %tp, align 8
  invoke void %f() to label %volta unwind label %pouso
volta:
  ret void
pouso:
  %lp = landingpad { ptr, i32 } catch ptr null
  store ptr %topo, ptr %tp, align 8
  ret void
}
define internal i64 @df.porta.r0(ptr %f) personality ptr @dartforge_personalidade {
entrada:
  %ctx = call ptr @dartforge_contexto()
  %tp = getelementptr inbounds i8, ptr %ctx, i64 8
  %topo = load ptr, ptr %tp, align 8
  %r = invoke i64 %f() to label %volta unwind label %pouso
volta:
  ret i64 %r
pouso:
  %lp = landingpad { ptr, i32 } catch ptr null
  store ptr %topo, ptr %tp, align 8
  ret i64 0
}
define internal void @df.porta.v1(ptr %f, i64 %a0) personality ptr @dartforge_personalidade {
entrada:
  %ctx = call ptr @dartforge_contexto()
  %tp = getelementptr inbounds i8, ptr %ctx, i64 8
  %topo = load ptr, ptr %tp, align 8
  invoke void %f(i64 %a0) to label %volta unwind label %pouso
volta:
  ret void
pouso:
  %lp = landingpad { ptr, i32 } catch ptr null
  store ptr %topo, ptr %tp, align 8
  ret void
}
define internal i64 @df.porta.r1(ptr %f, i64 %a0) personality ptr @dartforge_personalidade {
entrada:
  %ctx = call ptr @dartforge_contexto()
  %tp = getelementptr inbounds i8, ptr %ctx, i64 8
  %topo = load ptr, ptr %tp, align 8
  %r = invoke i64 %f(i64 %a0) to label %volta unwind label %pouso
volta:
  ret i64 %r
pouso:
  %lp = landingpad { ptr, i32 } catch ptr null
  store ptr %topo, ptr %tp, align 8
  ret i64 0
}
define internal void @df.porta.v2(ptr %f, i64 %a0, i64 %a1) personality ptr @dartforge_personalidade {
entrada:
  %ctx = call ptr @dartforge_contexto()
  %tp = getelementptr inbounds i8, ptr %ctx, i64 8
  %topo = load ptr, ptr %tp, align 8
  invoke void %f(i64 %a0, i64 %a1) to label %volta unwind label %pouso
volta:
  ret void
pouso:
  %lp = landingpad { ptr, i32 } catch ptr null
  store ptr %topo, ptr %tp, align 8
  ret void
}
define internal i64 @df.porta.r2(ptr %f, i64 %a0, i64 %a1) personality ptr @dartforge_personalidade {
entrada:
  %ctx = call ptr @dartforge_contexto()
  %tp = getelementptr inbounds i8, ptr %ctx, i64 8
  %topo = load ptr, ptr %tp, align 8
  %r = invoke i64 %f(i64 %a0, i64 %a1) to label %volta unwind label %pouso
volta:
  ret i64 %r
pouso:
  %lp = landingpad { ptr, i32 } catch ptr null
  store ptr %topo, ptr %tp, align 8
  ret i64 0
}
define internal void @df.porta.v3(ptr %f, i64 %a0, i64 %a1, i64 %a2) personality ptr @dartforge_personalidade {
entrada:
  %ctx = call ptr @dartforge_contexto()
  %tp = getelementptr inbounds i8, ptr %ctx, i64 8
  %topo = load ptr, ptr %tp, align 8
  invoke void %f(i64 %a0, i64 %a1, i64 %a2) to label %volta unwind label %pouso
volta:
  ret void
pouso:
  %lp = landingpad { ptr, i32 } catch ptr null
  store ptr %topo, ptr %tp, align 8
  ret void
}
define internal i64 @df.porta.r3(ptr %f, i64 %a0, i64 %a1, i64 %a2) personality ptr @dartforge_personalidade {
entrada:
  %ctx = call ptr @dartforge_contexto()
  %tp = getelementptr inbounds i8, ptr %ctx, i64 8
  %topo = load ptr, ptr %tp, align 8
  %r = invoke i64 %f(i64 %a0, i64 %a1, i64 %a2) to label %volta unwind label %pouso
volta:
  ret i64 %r
pouso:
  %lp = landingpad { ptr, i32 } catch ptr null
  store ptr %topo, ptr %tp, align 8
  ret i64 0
}
define internal void @df.porta.v4(ptr %f, i64 %a0, i64 %a1, i64 %a2, i64 %a3) personality ptr @dartforge_personalidade {
entrada:
  %ctx = call ptr @dartforge_contexto()
  %tp = getelementptr inbounds i8, ptr %ctx, i64 8
  %topo = load ptr, ptr %tp, align 8
  invoke void %f(i64 %a0, i64 %a1, i64 %a2, i64 %a3) to label %volta unwind label %pouso
volta:
  ret void
pouso:
  %lp = landingpad { ptr, i32 } catch ptr null
  store ptr %topo, ptr %tp, align 8
  ret void
}
define internal i64 @df.porta.r4(ptr %f, i64 %a0, i64 %a1, i64 %a2, i64 %a3) personality ptr @dartforge_personalidade {
entrada:
  %ctx = call ptr @dartforge_contexto()
  %tp = getelementptr inbounds i8, ptr %ctx, i64 8
  %topo = load ptr, ptr %tp, align 8
  %r = invoke i64 %f(i64 %a0, i64 %a1, i64 %a2, i64 %a3) to label %volta unwind label %pouso
volta:
  ret i64 %r
pouso:
  %lp = landingpad { ptr, i32 } catch ptr null
  store ptr %topo, ptr %tp, align 8
  ret i64 0
}
define internal void @df.porta.v5(ptr %f, i64 %a0, i64 %a1, i64 %a2, i64 %a3, i64 %a4) personality ptr @dartforge_personalidade {
entrada:
  %ctx = call ptr @dartforge_contexto()
  %tp = getelementptr inbounds i8, ptr %ctx, i64 8
  %topo = load ptr, ptr %tp, align 8
  invoke void %f(i64 %a0, i64 %a1, i64 %a2, i64 %a3, i64 %a4) to label %volta unwind label %pouso
volta:
  ret void
pouso:
  %lp = landingpad { ptr, i32 } catch ptr null
  store ptr %topo, ptr %tp, align 8
  ret void
}
define internal i64 @df.porta.r5(ptr %f, i64 %a0, i64 %a1, i64 %a2, i64 %a3, i64 %a4) personality ptr @dartforge_personalidade {
entrada:
  %ctx = call ptr @dartforge_contexto()
  %tp = getelementptr inbounds i8, ptr %ctx, i64 8
  %topo = load ptr, ptr %tp, align 8
  %r = invoke i64 %f(i64 %a0, i64 %a1, i64 %a2, i64 %a3, i64 %a4) to label %volta unwind label %pouso
volta:
  ret i64 %r
pouso:
  %lp = landingpad { ptr, i32 } catch ptr null
  store ptr %topo, ptr %tp, align 8
  ret i64 0
}
define internal void @df.porta.v6(ptr %f, i64 %a0, i64 %a1, i64 %a2, i64 %a3, i64 %a4, i64 %a5) personality ptr @dartforge_personalidade {
entrada:
  %ctx = call ptr @dartforge_contexto()
  %tp = getelementptr inbounds i8, ptr %ctx, i64 8
  %topo = load ptr, ptr %tp, align 8
  invoke void %f(i64 %a0, i64 %a1, i64 %a2, i64 %a3, i64 %a4, i64 %a5) to label %volta unwind label %pouso
volta:
  ret void
pouso:
  %lp = landingpad { ptr, i32 } catch ptr null
  store ptr %topo, ptr %tp, align 8
  ret void
}
define internal i64 @df.porta.r6(ptr %f, i64 %a0, i64 %a1, i64 %a2, i64 %a3, i64 %a4, i64 %a5) personality ptr @dartforge_personalidade {
entrada:
  %ctx = call ptr @dartforge_contexto()
  %tp = getelementptr inbounds i8, ptr %ctx, i64 8
  %topo = load ptr, ptr %tp, align 8
  %r = invoke i64 %f(i64 %a0, i64 %a1, i64 %a2, i64 %a3, i64 %a4, i64 %a5) to label %volta unwind label %pouso
volta:
  ret i64 %r
pouso:
  %lp = landingpad { ptr, i32 } catch ptr null
  store ptr %topo, ptr %tp, align 8
  ret i64 0
}
define internal void @df.porta.v7(ptr %f, i64 %a0, i64 %a1, i64 %a2, i64 %a3, i64 %a4, i64 %a5, i64 %a6) personality ptr @dartforge_personalidade {
entrada:
  %ctx = call ptr @dartforge_contexto()
  %tp = getelementptr inbounds i8, ptr %ctx, i64 8
  %topo = load ptr, ptr %tp, align 8
  invoke void %f(i64 %a0, i64 %a1, i64 %a2, i64 %a3, i64 %a4, i64 %a5, i64 %a6) to label %volta unwind label %pouso
volta:
  ret void
pouso:
  %lp = landingpad { ptr, i32 } catch ptr null
  store ptr %topo, ptr %tp, align 8
  ret void
}
define internal i64 @df.porta.r7(ptr %f, i64 %a0, i64 %a1, i64 %a2, i64 %a3, i64 %a4, i64 %a5, i64 %a6) personality ptr @dartforge_personalidade {
entrada:
  %ctx = call ptr @dartforge_contexto()
  %tp = getelementptr inbounds i8, ptr %ctx, i64 8
  %topo = load ptr, ptr %tp, align 8
  %r = invoke i64 %f(i64 %a0, i64 %a1, i64 %a2, i64 %a3, i64 %a4, i64 %a5, i64 %a6) to label %volta unwind label %pouso
volta:
  ret i64 %r
pouso:
  %lp = landingpad { ptr, i32 } catch ptr null
  store ptr %topo, ptr %tp, align 8
  ret i64 0
}
@df.portas = private unnamed_addr constant [16 x ptr] [ptr @df.porta.v0, ptr @df.porta.r0, ptr @df.porta.v1, ptr @df.porta.r1, ptr @df.porta.v2, ptr @df.porta.r2, ptr @df.porta.v3, ptr @df.porta.r3, ptr @df.porta.v4, ptr @df.porta.r4, ptr @df.porta.v5, ptr @df.porta.r5, ptr @df.porta.v6, ptr @df.porta.r6, ptr @df.porta.v7, ptr @df.porta.r7]

define void @dartforge_entry() {
  call void @dartforge_memoria_arc_v1()
  %df.arc.abi = call i8 @dartforge_arc_verificar_abi(i64 1)
  %df.arc.abi.ok = icmp eq i8 %df.arc.abi, 1
  br i1 %df.arc.abi.ok, label %df.arc.compativel, label %df.arc.incompativel
df.arc.incompativel:
  call void @llvm.trap()
  unreachable
df.arc.compativel:
  call void @dartforge_registrar_portas(ptr @df.portas, i64 16)
  call void @df.porta.v0(ptr @prova_guardas)
  ret void
}

@df.area = private unnamed_addr constant [2 x i64] [i64 -5689447049377982356, i64 0]
define void @df.preparar_area() {
  %a = call ptr @dartforge_area_de_globais(ptr @df.area)
  ret void
}
@df.area_id.programa = internal global i64 0, align 8
define internal ptr @df.obter_area() alwaysinline {
%ctx = call ptr @dartforge_contexto()
%id = load atomic i64, ptr @df.area_id.programa monotonic, align 8
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
%q = call ptr @dartforge_area_de_globais_id(ptr @df.area, ptr @df.area_id.programa)
ret ptr %q
}
; Definidos em outro módulo (SDK da fonte) ou no runtime
declare void @llvm.trap()


declare void @dartforge_print_handle(i64)

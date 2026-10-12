; ModuleID = 'target/itanium-acoes-estrangeira-inline.ll'
source_filename = "target/itanium-acoes-estrangeira-inline.ll"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-unknown-linux-gnu"

declare i32 @dartforge_personalidade_cleanup_itanium(...)

declare ptr @fixture_objeto(i32) local_unnamed_addr

declare void @fixture_cleanup(ptr, i32, i32, i32) local_unnamed_addr

declare i32 @fixture_resultado(ptr, i32) local_unnamed_addr

declare i32 @_Unwind_RaiseException(ptr) local_unnamed_addr

; Function Attrs: noreturn
declare void @dartforge_desenrolamento_falhou(i32) local_unnamed_addr #0

; Function Attrs: noinline noreturn
define void @lancar_fixture() local_unnamed_addr #1 {
entrada:
  %objeto = tail call ptr @fixture_objeto(i32 1)
  %r = tail call i32 @_Unwind_RaiseException(ptr %objeto)
  tail call void @dartforge_desenrolamento_falhou(i32 %r)
  unreachable
}

; Function Attrs: noinline
define noundef range(i32 77, 100) i32 @camada_dart() local_unnamed_addr #2 personality ptr @dartforge_personalidade_cleanup_itanium {
entrada:
  invoke void @lancar_fixture()
          to label %common.ret.unreachable unwind label %limpeza.i.i

limpeza.i.i:                                      ; preds = %entrada
  %lp.i.i = landingpad { ptr, i32 }
          cleanup
          catch ptr null
  %objeto.i.i = extractvalue { ptr, i32 } %lp.i.i, 0
  %seletor.i.i = extractvalue { ptr, i32 } %lp.i.i, 1
  invoke void @fixture_cleanup(ptr %objeto.i.i, i32 %seletor.i.i, i32 1, i32 0)
          to label %limpeza.body.i unwind label %limpeza.i

limpeza.i:                                        ; preds = %limpeza.i.i
  %lp.i = landingpad { ptr, i32 }
          cleanup
          catch ptr null
  %.pre = extractvalue { ptr, i32 } %lp.i, 0
  %.pre1 = extractvalue { ptr, i32 } %lp.i, 1
  br label %limpeza.body.i

limpeza.body.i:                                   ; preds = %limpeza.i.i, %limpeza.i
  %seletor.i.pre-phi = phi i32 [ %seletor.i.i, %limpeza.i.i ], [ %.pre1, %limpeza.i ]
  %objeto.i.pre-phi = phi ptr [ %objeto.i.i, %limpeza.i.i ], [ %.pre, %limpeza.i ]
  %eh.lpad-body.i = phi { ptr, i32 } [ %lp.i.i, %limpeza.i.i ], [ %lp.i, %limpeza.i ]
  invoke void @fixture_cleanup(ptr %objeto.i.pre-phi, i32 %seletor.i.pre-phi, i32 2, i32 0)
          to label %tratador.body unwind label %tratador

common.ret.unreachable:                           ; preds = %entrada
  unreachable

common.ret:                                       ; preds = %tratador.body
  ret i32 77

tratador:                                         ; preds = %limpeza.body.i
  %lp = landingpad { ptr, i32 }
          catch ptr null
  %.pre2 = extractvalue { ptr, i32 } %lp, 1
  br label %tratador.body

tratador.body:                                    ; preds = %limpeza.body.i, %tratador
  %seletor.pre-phi = phi i32 [ %seletor.i.pre-phi, %limpeza.body.i ], [ %.pre2, %tratador ]
  %eh.lpad-body = phi { ptr, i32 } [ %eh.lpad-body.i, %limpeza.body.i ], [ %lp, %tratador ]
  %tipo = tail call i32 @llvm.eh.typeid.for.p0(ptr null)
  %dart = icmp eq i32 %seletor.pre-phi, %tipo
  br i1 %dart, label %common.ret, label %retoma

retoma:                                           ; preds = %tratador.body
  resume { ptr, i32 } %eh.lpad-body
}

declare i32 @__gxx_personality_v0(...)

define i32 @main() local_unnamed_addr personality ptr @__gxx_personality_v0 {
entrada:
  %r0 = invoke i32 @camada_dart()
          to label %common.ret unwind label %tratador

common.ret:                                       ; preds = %entrada, %tratador
  %common.ret.op = phi i32 [ %r, %tratador ], [ 99, %entrada ]
  ret i32 %common.ret.op

tratador:                                         ; preds = %entrada
  %lp = landingpad { ptr, i32 }
          catch ptr null
  %objeto = extractvalue { ptr, i32 } %lp, 0
  %seletor = extractvalue { ptr, i32 } %lp, 1
  %r = tail call i32 @fixture_resultado(ptr %objeto, i32 %seletor)
  br label %common.ret
}

; Function Attrs: nofree nosync nounwind memory(none)
declare i32 @llvm.eh.typeid.for.p0(ptr) #3

attributes #0 = { noreturn }
attributes #1 = { noinline noreturn }
attributes #2 = { noinline }
attributes #3 = { nofree nosync nounwind memory(none) }

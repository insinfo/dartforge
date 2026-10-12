; ModuleID = 'target/itanium-acoes-inline.ll'
source_filename = "target/itanium-acoes-inline.ll"
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
  %objeto = tail call ptr @fixture_objeto(i32 0)
  %r = tail call i32 @_Unwind_RaiseException(ptr %objeto)
  tail call void @dartforge_desenrolamento_falhou(i32 %r)
  unreachable
}

define i32 @main() local_unnamed_addr personality ptr @dartforge_personalidade_cleanup_itanium {
entrada:
  invoke void @lancar_fixture()
          to label %common.ret.unreachable unwind label %limpeza.i.i

limpeza.i.i:                                      ; preds = %entrada
  %lp.i.i = landingpad { ptr, i32 }
          cleanup
          catch ptr null
  %objeto.i.i = extractvalue { ptr, i32 } %lp.i.i, 0
  %seletor.i.i = extractvalue { ptr, i32 } %lp.i.i, 1
  invoke void @fixture_cleanup(ptr %objeto.i.i, i32 %seletor.i.i, i32 1, i32 1)
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
  invoke void @fixture_cleanup(ptr %objeto.i.pre-phi, i32 %seletor.i.pre-phi, i32 2, i32 1)
          to label %tratador.body unwind label %tratador

common.ret.unreachable:                           ; preds = %entrada
  unreachable

tratador:                                         ; preds = %limpeza.body.i
  %lp = landingpad { ptr, i32 }
          catch ptr null
  %.pre2 = extractvalue { ptr, i32 } %lp, 0
  %.pre3 = extractvalue { ptr, i32 } %lp, 1
  br label %tratador.body

tratador.body:                                    ; preds = %limpeza.body.i, %tratador
  %seletor.pre-phi = phi i32 [ %seletor.i.pre-phi, %limpeza.body.i ], [ %.pre3, %tratador ]
  %objeto.pre-phi = phi ptr [ %objeto.i.pre-phi, %limpeza.body.i ], [ %.pre2, %tratador ]
  %r = tail call i32 @fixture_resultado(ptr %objeto.pre-phi, i32 %seletor.pre-phi)
  ret i32 %r
}

attributes #0 = { noreturn }
attributes #1 = { noinline noreturn }

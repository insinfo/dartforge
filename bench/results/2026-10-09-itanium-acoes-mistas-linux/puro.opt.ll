; ModuleID = 'target/itanium-acoes-puro.ll'
source_filename = "target/itanium-acoes-puro.ll"
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

; Function Attrs: noinline noreturn
define void @cleanup_interno() local_unnamed_addr #1 personality ptr @dartforge_personalidade_cleanup_itanium {
entrada:
  invoke void @lancar_fixture()
          to label %normal.unreachable unwind label %limpeza

normal.unreachable:                               ; preds = %entrada
  unreachable

limpeza:                                          ; preds = %entrada
  %lp = landingpad { ptr, i32 }
          cleanup
  %objeto = extractvalue { ptr, i32 } %lp, 0
  %seletor = extractvalue { ptr, i32 } %lp, 1
  tail call void @fixture_cleanup(ptr %objeto, i32 %seletor, i32 1, i32 0)
  resume { ptr, i32 } %lp
}

; Function Attrs: noinline noreturn
define void @cleanup_externo() local_unnamed_addr #1 personality ptr @dartforge_personalidade_cleanup_itanium {
entrada:
  invoke void @cleanup_interno()
          to label %normal.unreachable unwind label %limpeza

normal.unreachable:                               ; preds = %entrada
  unreachable

limpeza:                                          ; preds = %entrada
  %lp = landingpad { ptr, i32 }
          cleanup
  %objeto = extractvalue { ptr, i32 } %lp, 0
  %seletor = extractvalue { ptr, i32 } %lp, 1
  tail call void @fixture_cleanup(ptr %objeto, i32 %seletor, i32 2, i32 0)
  resume { ptr, i32 } %lp
}

define i32 @main() local_unnamed_addr personality ptr @dartforge_personalidade_cleanup_itanium {
entrada:
  invoke void @cleanup_externo()
          to label %common.ret.unreachable unwind label %tratador

common.ret.unreachable:                           ; preds = %entrada
  unreachable

tratador:                                         ; preds = %entrada
  %lp = landingpad { ptr, i32 }
          catch ptr null
  %objeto = extractvalue { ptr, i32 } %lp, 0
  %seletor = extractvalue { ptr, i32 } %lp, 1
  %r = tail call i32 @fixture_resultado(ptr %objeto, i32 %seletor)
  ret i32 %r
}

attributes #0 = { noreturn }
attributes #1 = { noinline noreturn }

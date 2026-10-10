; Prova nativa da personalidade real: dois cleanups e um catch-all externo.
target triple = "x86_64-unknown-linux-gnu"
declare i32 @dartforge_personalidade_cleanup_itanium(...)
declare ptr @fixture_objeto(i32)
declare void @fixture_cleanup(ptr, i32, i32, i32)
declare i32 @fixture_resultado(ptr, i32)
declare i32 @_Unwind_RaiseException(ptr)
declare void @dartforge_desenrolamento_falhou(i32) noreturn

define void @lancar_fixture() noinline {
entrada:
  %objeto = call ptr @fixture_objeto(i32 0)
  %r = call i32 @_Unwind_RaiseException(ptr %objeto)
  call void @dartforge_desenrolamento_falhou(i32 %r)
  unreachable
}
define internal void @cleanup_interno() alwaysinline personality ptr @dartforge_personalidade_cleanup_itanium {
entrada:
  invoke void @lancar_fixture() to label %normal unwind label %limpeza
normal:
  ret void
limpeza:
  %lp = landingpad { ptr, i32 } cleanup
  %objeto = extractvalue { ptr, i32 } %lp, 0
  %seletor = extractvalue { ptr, i32 } %lp, 1
  call void @fixture_cleanup(ptr %objeto, i32 %seletor, i32 1, i32 1)
  resume { ptr, i32 } %lp
}
define internal void @cleanup_externo() alwaysinline personality ptr @dartforge_personalidade_cleanup_itanium {
entrada:
  invoke void @cleanup_interno() to label %normal unwind label %limpeza
normal:
  ret void
limpeza:
  %lp = landingpad { ptr, i32 } cleanup
  %objeto = extractvalue { ptr, i32 } %lp, 0
  %seletor = extractvalue { ptr, i32 } %lp, 1
  call void @fixture_cleanup(ptr %objeto, i32 %seletor, i32 2, i32 1)
  resume { ptr, i32 } %lp
}
define i32 @main() personality ptr @dartforge_personalidade_cleanup_itanium {
entrada:
  invoke void @cleanup_externo() to label %normal unwind label %tratador
normal:
  ret i32 99
tratador:
  %lp = landingpad { ptr, i32 } catch ptr null
  %objeto = extractvalue { ptr, i32 } %lp, 0
  %seletor = extractvalue { ptr, i32 } %lp, 1
  %r = call i32 @fixture_resultado(ptr %objeto, i32 %seletor)
  ret i32 %r
}

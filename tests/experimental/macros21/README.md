# JsonCodable experimental

`main.dart` usa a macro incorporada do DartForge, implementada em Rust antes da análise semântica. Não é apresentado como programa aceito pelo SDK Dart 3.6.2.

`manual_expanded.dart` é o equivalente escrito manualmente e executável no SDK 3.6.2. A comparação valida o comportamento gerado, não o protocolo histórico de macros. `expected.stdout` registra essa execução. Campos nullable aparecem no mapa mesmo quando null; chaves extras da entrada são ignoradas. Mapas resultantes são novos e mutáveis.

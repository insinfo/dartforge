/// Executor de builders do DartForge (serviço `build.*` do `dfexec/1`).
///
/// O motor de build gera um *bootstrap* que importa as fábricas do plano e
/// chama [servir]; ver `crates/build/src/vm.rs` e docs/BUILD-PROTOCOLO.md.
library;

export 'src/canal.dart' show Canal, CanalStdio;
export 'src/servico.dart' show FabricaDeBuilder, protocolo, servir, versaoDoExecutor;

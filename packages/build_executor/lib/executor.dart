/// DartForge's builder executor (the `build.*` service of `dfexec/1`).
///
/// The build engine generates a *bootstrap* that imports the plan's factories
/// and calls [serve]; see `crates/build/src/vm.rs` and docs/BUILD-PROTOCOLO.md.
library;

export 'src/channel.dart' show Channel, StdioChannel;
export 'src/service.dart' show BuilderFactory, protocol, serve, executorVersion;

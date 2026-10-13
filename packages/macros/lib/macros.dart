/// The API a macro author uses (docs/MACROS-PROTOCOLO.md).
///
/// Rewritten by DartForge from the specification
/// (`dart-language/working/macros/feature-specification.md`), with the same
/// public surface as `package:macros` 0.1.3-main.0 — the 1st generation that
/// the 3.6.2 SDK runs with `--enable-experiment=macros` —, so that macros
/// written for it, like `package:json` 0.20.4's `@JsonCodable`, compile
/// unchanged. The executor side (the introspection served by the host and the
/// results) lives in `src/executor/` and is not API.
library;

export 'src/api.dart';

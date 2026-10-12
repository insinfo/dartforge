/// A API que um autor de macro usa (docs/MACROS-PROTOCOLO.md).
///
/// Reescrita pelo DartForge a partir da especificação
/// (`dart-language/working/macros/feature-specification.md`), com a mesma
/// superfície pública do `package:macros` 0.1.3-main.0 — a 1ª geração que o
/// SDK 3.6.2 executa com `--enable-experiment=macros` —, para que macros
/// escritas para ele, como o `@JsonCodable` do `package:json` 0.20.4,
/// compilem sem mudança. O lado do executor (a introspecção servida pelo
/// hospedeiro e os resultados) fica em `src/executor/` e não é API.
library;

export 'src/api.dart';

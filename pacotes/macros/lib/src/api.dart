/// A API pública de macros, dividida por assunto:
///
/// * `macros.dart` — as interfaces de macro, uma por (alvo, fase);
/// * `builders.dart` — o que cada fase pode consultar e produzir;
/// * `code.dart` — os pedaços de código que uma macro monta;
/// * `introspection.dart` — o que uma macro vê do programa;
/// * `diagnostic.dart` e `exceptions.dart` — o que ela reporta ou recebe.
library;

import 'dart:async';
import 'dart:collection' show UnmodifiableListView;

part 'api/builders.dart';
part 'api/code.dart';
part 'api/diagnostic.dart';
part 'api/exceptions.dart';
part 'api/introspection.dart';
part 'api/macros.dart';

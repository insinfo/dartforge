/// The public macro API, split by subject:
///
/// * `macros.dart` — the macro interfaces, one per (target, phase);
/// * `builders.dart` — what each phase can query and produce;
/// * `code.dart` — the pieces of code a macro assembles;
/// * `introspection.dart` — what a macro sees of the program;
/// * `diagnostic.dart` and `exceptions.dart` — what it reports or receives.
library;

import 'dart:async';
import 'dart:collection' show UnmodifiableListView;

part 'api/builders.dart';
part 'api/code.dart';
part 'api/diagnostic.dart';
part 'api/exceptions.dart';
part 'api/introspection.dart';
part 'api/macros.dart';

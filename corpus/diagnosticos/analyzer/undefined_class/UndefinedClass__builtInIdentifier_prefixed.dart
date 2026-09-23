import 'dart:math' as p;
//     ^^^^^^^^^^^
// [diag.unusedImport] Unused import: 'dart:math'.

void f(p.import x) {}
//       ^^^^^^
// [diag.builtInIdentifierAsType] The built-in identifier 'import' can't be used as a type.

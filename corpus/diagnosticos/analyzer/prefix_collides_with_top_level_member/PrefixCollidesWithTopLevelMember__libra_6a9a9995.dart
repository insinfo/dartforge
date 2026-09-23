import 'dart:math' as foo;
//     ^^^^^^^^^^^
// [diag.unusedImport] Unused import: 'dart:math'.
//                    ^^^
// [diag.prefixCollidesWithTopLevelMember][context 1] The name 'foo' is already used as an import prefix and can't be used to name a top-level element.
int get foo => 0;
//      ^^^
// [context 1] The first definition of this name.

// %before-language-feature: single-combinators
// ignore: unused_import
import 'dart:async' as async hide Future, Stream show Stream;
//                           ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
// [diag.multipleCombinatorsDeprecated] Using multiple 'hide' or 'show' combinators is never necessary and often produces surprising results.

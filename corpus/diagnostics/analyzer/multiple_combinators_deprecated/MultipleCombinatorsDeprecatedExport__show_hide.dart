// %before-language-feature: single-combinators
export 'dart:async' show Future, Stream hide Stream;
//                  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
// [diag.multipleCombinatorsDeprecated] Using multiple 'hide' or 'show' combinators is never necessary and often produces surprising results.

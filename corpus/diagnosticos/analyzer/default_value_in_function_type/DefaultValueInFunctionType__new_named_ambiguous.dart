typedef F = int Function({Object m = const {1, 2: 3}});
//                                 ^
// [diag.defaultValueInFunctionType] Parameters in a function type can't have default values.
//                                   ^^^^^^^^^^^^^^^
// [diag.ambiguousSetOrMapLiteralBoth] The literal can't be either a map or a set because it contains at least one literal map entry or a spread operator spreading a 'Map', and at least one element which is neither of these.

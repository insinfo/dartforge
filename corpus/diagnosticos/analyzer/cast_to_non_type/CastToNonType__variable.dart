var A = 0;
f(String s) { var x = s as A; }
//                ^
// [diag.unusedLocalVariable] The value of the local variable 'x' isn't used.
//                         ^
// [diag.castToNonType] The name 'A' isn't a type, so it can't be used in an 'as' expression.

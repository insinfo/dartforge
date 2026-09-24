mixin M {}
f(M m) => m + 1;
//          ^
// [diag.undefinedOperator] The operator '+' isn't defined for the type 'M'.

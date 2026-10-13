enum E { A }
f(E e) => e + 1;
//          ^
// [diag.undefinedOperator] The operator '+' isn't defined for the type 'E'.

class A {}
f() { A.B = 0;}
//      ^
// [diag.undefinedSetter] The setter 'B' isn't defined for the type 'A'.

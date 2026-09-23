class T {}
f(T e1) { e1.m = 0; }
//           ^
// [diag.undefinedSetter] The setter 'm' isn't defined for the type 'T'.

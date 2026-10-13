class T {}
f(T e) { return e.m; }
//                ^
// [diag.undefinedGetter] The getter 'm' isn't defined for the type 'T'.

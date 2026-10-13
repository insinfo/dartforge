T f<T>(T t) => null;
//             ^^^^
// [diag.returnOfInvalidTypeFromFunction] A value of type 'Null' can't be returned from the function 'f' because it has a return type of 'T'.
main() { f(<S>(S s) => s); }

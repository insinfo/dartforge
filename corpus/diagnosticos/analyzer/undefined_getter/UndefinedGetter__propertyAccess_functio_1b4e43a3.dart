void f(Function a) {
  return (a).call;
//       ^^^^^^^^
// [diag.returnOfInvalidTypeFromFunction] A value of type 'Function' can't be returned from the function 'f' because it has a return type of 'void'.
}

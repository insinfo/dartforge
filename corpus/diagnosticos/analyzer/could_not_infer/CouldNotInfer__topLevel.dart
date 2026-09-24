class C<P extends num> {
  factory C(Iterable<P> p) => C._();
  C._();
}

var c = C([]);
//        ^^
// [diag.argumentTypeNotAssignable] The argument type 'List<dynamic>' can't be assigned to the parameter type 'Iterable<num>'.

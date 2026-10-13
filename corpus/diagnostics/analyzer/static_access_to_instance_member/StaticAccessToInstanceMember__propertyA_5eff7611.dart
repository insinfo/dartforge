class C<T> {
  List<T> t = [];
}
var x = C.t;
//        ^
// [diag.staticAccessToInstanceMember] Instance member 't' can't be accessed using static access.

void f() {
  g()!;
//^^^^
// [diag.nullCheckAlwaysFails] This null-check will always throw an exception because the expression will always evaluate to 'null'.
}
Null g() => null;

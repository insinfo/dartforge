void f() async {
  (await g())!;
//^^^^^^^^^^^^
// [diag.nullCheckAlwaysFails] This null-check will always throw an exception because the expression will always evaluate to 'null'.
}
Future<Null> g() async => null;

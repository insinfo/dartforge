Future<int> g() async => 1;
Future<void> f() async {
  late var a = await g();
  var b = await g();
  late var c = () async => await g();
  late var d = [await g(), await g()];
  print([a, b, c, d]);
}
class C { late var x = await g(); }

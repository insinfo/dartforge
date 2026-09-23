typedef Fn = void Function(String s);

class CanFn {
  void call(String s) => print(s);
}

Future<Fn> f() async {
  return CanFn();
}

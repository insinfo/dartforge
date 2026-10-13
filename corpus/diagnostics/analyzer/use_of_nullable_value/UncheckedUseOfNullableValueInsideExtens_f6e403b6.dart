extension<X> on X {
  X m() => this;
}

Future<void> f(Never? x) async {
  (await x).m();
}

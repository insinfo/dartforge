Future<void> f1() async {}
Future<void> f2() async { return; }
Future<void> f3() async { return null; }
Future<void> f4() async { return g1(); }
Future<void> f5() async { return g2(); }
g1() {}
void g2() {}

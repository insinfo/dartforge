import 'package:meta/meta.dart';

class C {
  m() {
    f({@experimental int x = 0}) {
      return x;
    }
    return f();
  }
}

import 'package:meta/meta.dart';

class C {
  m({@experimental int x = 0}) {
    f() {
      return x;
    }
    return f();
  }
}

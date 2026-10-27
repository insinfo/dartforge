class Interface {
  int _hidden = 1;
  int visible = 2;
  static Object? readHidden(Interface x) {
    try {
      return x._hidden;
    } on NoSuchMethodError {
      return 'no such method';
    }
  }

  static Object? readHiddenDynamically(dynamic x) {
    try {
      return x._hidden;
    } on NoSuchMethodError {
      return 'no such method';
    }
  }
}

class Stubs implements Interface {
  int calls = 0;
  @override
  noSuchMethod(Invocation invocation) => calls++;
}

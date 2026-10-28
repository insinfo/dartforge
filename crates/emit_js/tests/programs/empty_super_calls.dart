// A `super()` call to a constructor that does nothing is left out in
// production; constructors with fields, bodies, arguments, type arguments
// or mixins still run, in the same order as on the VM.

final log = <String>[];

class Empty {
  Empty();
}

class Chain extends Empty {}

class ChainNamed extends Empty {
  ChainNamed.named() : super();
}

class WithField {
  int counter = (() {
    log.add('WithField.counter');
    return 1;
  })();
}

class OverField extends WithField {}

class WithBody {
  WithBody() {
    log.add('WithBody()');
  }
  WithBody.quiet();
}

class OverBody extends WithBody {
  OverBody() : super();
}

class OverQuiet extends WithBody {
  OverQuiet() : super.quiet();
}

class WithArgs {
  final String label;
  WithArgs(this.label);
}

class OverArgs extends WithArgs {
  OverArgs() : super('args');
}

class Generic<T> {
  bool test(Object? o) => o is T;
}

class OverGeneric extends Generic<int> {}

mixin Noted {
  String note = 'noted';
}

class OverMixin extends Empty with Noted {}

class ConstBase {
  const ConstBase();
}

class ConstChild extends ConstBase {
  const ConstChild();
}

class MyError extends Error {
  final String what;
  MyError(this.what);
  @override
  String toString() => 'MyError($what)';
}

class PlainError extends Error {}

class MyException implements Exception {
  @override
  String toString() => 'MyException';
}

class SuperParam extends WithArgs {
  SuperParam(super.label);
}

void main() {
  print([Chain() is Empty, ChainNamed.named() is Empty]);
  print(OverField().counter);
  print(log);
  OverBody();
  OverQuiet();
  print(log);
  print(OverArgs().label);
  print([OverGeneric().test(1), OverGeneric().test('x')]);
  print(OverMixin().note);
  print(identical(const ConstChild(), const ConstChild()));
  try {
    throw MyError('boom');
  } on Error catch (e) {
    print([e, e.stackTrace != null]);
  }
  try {
    throw PlainError();
  } catch (e) {
    print(e is Error);
  }
  try {
    throw MyException();
  } catch (e) {
    print(e);
  }
  print(SuperParam('param').label);
  print([Chain().runtimeType, OverMixin().runtimeType]);
}

// A late final local assigned while its initializer runs throws. A
// noSuchMethod forwarder for a setter checks dynamic writes against the
// interface type, and a generic forwarder can be torn off and called
// dynamically with default type arguments.
class Cat {
  String mood = '';
}

class MockCat implements Cat {
  @override
  dynamic noSuchMethod(Invocation invocation) => null;
}

class MockWithGenerics {
  List<Type> doStuff<T>(T t);

  @override
  noSuchMethod(i) => (i as dynamic).typeArguments;
}

void main() {
  late final int local;
  int init() {
    local = 1;
    return 2;
  }

  late final int other = init();
  try {
    print(other);
  } on Error catch (e) {
    print(e.runtimeType.toString().contains('LateError') ? 'late error' : e);
  }

  dynamic mock = MockCat();
  mock.mood = 'sleepy';
  try {
    mock.mood = 42;
    print('accepted');
  } on TypeError {
    print('TypeError');
  }

  var g = MockWithGenerics();
  var doStuff = g.doStuff;
  print(doStuff(42));
  dynamic d = g;
  print(d.doStuff<num>(1));
  print(d.doStuff('x').first == dynamic);
}

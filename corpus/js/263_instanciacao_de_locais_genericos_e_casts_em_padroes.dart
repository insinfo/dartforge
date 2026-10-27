// A generic function held in a local is instantiated when it flows into a
// non-generic function type (assignment, operator argument, relational
// pattern). A cast pattern runs its cast even when the subpattern is
// statically true.
T id<T>(T t) => t;

typedef IntFn = int Function(int);
typedef TFn = T Function<T>(T);

class Compare {
  bool operator <(IntFn f) => f is TFn;
}

sealed class M {}

class A extends M {}

class B extends M {}

int onlyA(dynamic m) => switch (m) {
      (A() || B()) as A => 0,
    };

void main() {
  const c = id;
  final d = id;
  IntFn f1 = c;
  IntFn f2 = d;
  print([f1 is TFn, f2 is TFn, f1(3), f2(4)]);
  print(Compare() < c);
  if (Compare() case < c) {
    print('generic');
  } else {
    print('instantiated');
  }
  print(onlyA(A()));
  try {
    onlyA(B());
    print('no error');
  } on TypeError {
    print('TypeError');
  }
}

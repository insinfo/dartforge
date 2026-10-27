// Implicit downcasts from `dynamic` to types that mention type parameters
// (`List<T>`, `T`, a function type over `T`) are checked like on the VM, in
// locals, returns and assignments inside generic classes and methods. A record
// literal gives each field the context of the record type, so a callable
// object in a field whose type is a function type is torn off (`.call`), also
// when the context is the schema of a declaration pattern.
class C {
  int call(int x) => x + 1;
}

typedef IntFn = int Function(int);

class Box<T> {
  late List<T> field;

  void tryAll(dynamic d) {
    try {
      field = d;
      print('field accepted');
    } catch (e) {
      print('field rejected');
    }
    try {
      List<T> local = d;
      print('local accepted ${local.length}');
    } catch (e) {
      print('local rejected');
    }
    try {
      T Function(T) f = d;
      print('function accepted ${f.runtimeType == Null}');
    } catch (e) {
      print('function rejected');
    }
  }
}

T identity<T>(dynamic d) => d;

void main() {
  Box<bool>().tryAll(<int>[1]);
  Box<int>().tryAll(<int>[1, 2]);
  Box<int>().tryAll((int x) => x);
  try {
    identity<String>(1);
    print('return accepted');
  } catch (e) {
    print('return rejected');
  }
  print(identity<int>(3));

  (IntFn,) r = (C(),);
  print(r.$1 is IntFn);
  print(r.$1(1));
  var (IntFn b,) = (C(),);
  print(b is C);
  print(b(2));
  final (IntFn x, y: IntFn y) = (C(), y: C());
  print(x(3) + y(4));
  (C,) record = (C(),);
  if (record case (IntFn f,)) {
    print('matched ${f(0)}');
  } else {
    print('a C is not a function');
  }
}

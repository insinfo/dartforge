// The checks each optimization level keeps (`dartforge-jsprod -O1`…`-O4`,
// with `dart2js`'s meanings) and the `dart2js:` pragmas that override the
// level in one member. Each line is `<check>: <outcome>`; on the VM every
// check throws.

class Box<T> {
  T value;
  Box(this.value);
  void put(T x) {
    value = x;
  }
}

class CheckedBox<T> {
  T value;
  CheckedBox(this.value);
  @pragma('dart2js:parameter:check')
  void put(T x) {
    value = x;
  }
}

@pragma('dart2js:tryInline')
@pragma('dart2js:as:trust')
T unsafeCast<T>(Object? o) => o as T;

@pragma('dart2js:downcast:check')
String checkedDowncast(dynamic d) => d;

@pragma('dart2js:index-bounds:trust')
String trustedChar(String s, int i) => s[i];

@pragma('dart2js:index-bounds:check')
int checkedIndex(List<int> l, int i) => l[i];

int index(List<int> l, int i) => l[i];

const isJavaScript = identical(1, 1.0);
const noBoundsChecks = isJavaScript
    ? pragma('dart2js:index-bounds:trust')
    : pragma('vm:unsafe:no-bounds-checks');

@noBoundsChecks
int unitAt(String s, int i) => s.codeUnitAt(i);

String outcome(Object? Function() f) {
  try {
    return '${f()}';
  } on TypeError {
    return 'TypeError';
  } on RangeError {
    return 'RangeError';
  } on NoSuchMethodError {
    return 'NoSuchMethodError';
  }
}

void main() {
  dynamic d = 3;
  print('downcast: ${outcome(() {
    String s = d;
    return s;
  })}');
  print('condition: ${outcome(() {
    dynamic c = 1;
    if (c) return 'then';
    return 'else';
  })}');
  Box<Object> b = Box<int>(1);
  print('covariant parameter: ${outcome(() {
    b.put('s');
    return b.value;
  })}');
  print('dynamic call argument: ${outcome(() {
    dynamic f = (int x) => x;
    return f('s');
  })}');
  print('dynamic call arity: ${outcome(() {
    dynamic f = (int x) => x;
    return f(1, 2);
  })}');
  print('explicit cast: ${outcome(() => d as String)}');
  print('as:trust: ${outcome(() => unsafeCast<String>(d))}');
  print('downcast:check: ${outcome(() => checkedDowncast(d))}');
  CheckedBox<Object> c = CheckedBox<int>(1);
  print('parameter:check: ${outcome(() {
    c.put('s');
    return c.value;
  })}');
  print('index: ${outcome(() => index([1, 2], 5))}');
  print('string index: ${outcome(() => 'ab'[5])}');
  print('index-bounds:trust: ${outcome(() => trustedChar('ab', 5))}');
  print('index-bounds:check: ${outcome(() => checkedIndex([1, 2], 5))}');
  print('constant pragma: ${outcome(() => unitAt('ab', 5))}');
  print('in range: ${index([1, 2], 1)} ${'ab'[1]} ${unitAt('ab', 0)}');
}

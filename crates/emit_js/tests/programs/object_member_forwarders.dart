// `Object`'s `dartx.toString` forwards to the object's own `toString` in
// production, and classes outside the native prototype chains keep no
// alias; every conversion, tear-off, equality and hash the runtime
// computes stays the VM's.

import 'dart:collection';

class Plain {}

class Named {
  final String name;
  Named(this.name);
  @override
  String toString() => 'Named($name)';
}

class Sub extends Named {
  Sub() : super('sub');
}

class SubOver extends Named {
  SubOver() : super('over');
  @override
  String toString() => 'SubOver<${super.toString()}>';
}

class Point {
  final int x, y;
  const Point(this.x, this.y);
  @override
  bool operator ==(Object other) => other is Point && other.x == x && other.y == y;
  @override
  int get hashCode => x * 31 + y;
  @override
  String toString() => 'P($x,$y)';
}

mixin Loud {
  @override
  String toString() => 'LOUD';
}

class Mixed with Loud {}

class MixedOver extends Named with Loud {
  MixedOver() : super('m');
}

class Proxy {
  @override
  String toString();
  @override
  dynamic noSuchMethod(Invocation i) => 'nsm:${i.memberName}';
}

class MyList extends ListBase<int> {
  final List<int> inner = [1, 2];
  @override
  int get length => inner.length;
  @override
  set length(int v) => inner.length = v;
  @override
  int operator [](int i) => inner[i];
  @override
  void operator []=(int i, int v) => inner[i] = v;
}

class MyError extends Error {
  @override
  String toString() => 'MyError!';
}

void main() {
  final Object o1 = Named('a'), o2 = Sub(), o3 = SubOver(), o4 = Mixed(), o5 = MixedOver();
  print([o1, o2, o3, o4, o5]);
  print('$o1 $o2 $o3 $o4 $o5');
  print([Plain().toString().startsWith('Instance of'), '${Plain()}'.contains('Plain')]);
  final Object p = const Point(1, 2), q = Point(1, 2);
  print([p == q, p.hashCode == q.hashCode, identical(p, q), {p, q}.length]);
  print({p: 'a', q: 'b'});
  final Object r = Plain();
  print([r == r, r == Plain(), r.hashCode == r.hashCode]);
  print(Proxy().toString());
  print('${MyList()} ${MyList().length}');
  print([MyError()].toString());
  final dynamic d = Point(3, 4);
  print([d.toString(), d == Point(3, 4), d.hashCode]);
  print(Set<Object>.from([Point(5, 6), Point(5, 6), Named('x')]).length);
  // Tear-offs of toString through every static type are equal.
  final n = Named('t');
  final Object asObject = n;
  final dynamic asDynamic = n;
  final String Function() t1 = n.toString, t2 = asObject.toString;
  final t3 = asDynamic.toString;
  print([t1 == t2, t2 == t3, t1 == t3, t1(), t2(), t3()]);
  print([t2 is String Function(), t2.runtimeType == t1.runtimeType]);
  final Object plain = Plain();
  final String Function() t4 = plain.toString;
  final dynamic dplain = plain;
  print([t4 == dplain.toString, t4().contains('Plain')]);
}

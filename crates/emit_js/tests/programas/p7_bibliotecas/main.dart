import 'util.dart';
import 'other.dart' as o show shown, twice;
import 'other.dart' hide twice;

class P {
  int x = 0;
  String? name;
  List<int> items = [];
  P? next;
  void add(int v) => items.add(v);
  P self() => this;
}

sealed class Shape {}
class Sq extends Shape { final double s; Sq(this.s); }
class Ci extends Shape { final double r; Ci(this.r); }

double area(Shape s) => switch (s) { Sq(s: var side) => side * side, Ci(:var r) => 3 * r * r };

String describe(Object? o) {
  return switch (o) {
    int i when i > 10 => 'big int $i',
    int i => 'int $i',
    String s => 'str $s',
    [int a, int b] => 'pair $a $b',
    [int first, ...] => 'list starting $first',
    {'k': var v} => 'map k=$v',
    (int a, String b) => 'rec $a $b',
    (x: var q) => 'named $q',
    null => 'null',
    _ => 'other',
  };
}

void main() {
  print(twice(2));
  print(o.twice(2));
  print(o.shown());
  print(hidden());
  print(shown());
  print(fromPart(1));
  print(PartClass().hello());
  print(Mode.slow.name);
  print(answer);
  print(3.sq);
  print(3.rep(2));
  print(IntX.zero());
  print(IntX(4).sq);
  IntFn f = (x) => x + 1;
  print(f(1));
  Pair pr = (1, 'a');
  print(pr.$1);
  print(pr.$2);
  print(pr);
  var rec = (a: 1, b: 'x', 3);
  print(rec.a);
  print(rec.b);
  print(rec.$1);
  print(rec);
  print(rec == (a: 1, b: 'x', 3));
  var (q, w) = (1, 2);
  print(q + w);
  final (:a, :b) = (a: 5, b: 6);
  print(a * b);
  var [h, ...t] = [1, 2, 3];
  print('$h $t');
  var {'k': kv} = {'k': 9};
  print(kv);
  var c = Counter()..inc()..inc();
  print(c.n);
  var p = P()
    ..x = 5
    ..add(1)
    ..add(2)
    ..name = 'p';
  print(p.x);
  print(p.items);
  print(p.name);
  P? np;
  print(np?.x);
  print(np?.self().x);
  np?.add(1);
  print(np?.items ?? [0]);
  np ??= P();
  print(np.x);
  print(np!.x);
  np?..x = 7..add(3);
  print(np.x);
  print(np.next?.next?.x);
  print(p.name?.length);
  print(p.name!.length);
  dynamic d = P();
  d.x = 3;
  print(d.x);
  d.add(4);
  print(d.items);
  print(d.self().x);
  dynamic dl = [1, 2];
  print(dl[0]);
  dl[0] = 5;
  print(dl);
  print(dl.length);
  dynamic ds = 'abc';
  print(ds.length);
  print(ds + 'd');
  print(ds == 'abc');
  dynamic dn = 5;
  print(dn + 1);
  print(dn * 2.5);
  print(dn > 3);
  print(-dn);
  print(dn.isEven);
  print(dn.toString());
  print(dn.hashCode == 5.hashCode);
  print(dn.runtimeType);
  dynamic df = (int x) => x + 1;
  print(df(1));
  print(area(Sq(2)).toStringAsFixed(1));
  print(area(Ci(1)).toStringAsFixed(1));
  print(describe(20));
  print(describe(5));
  print(describe('s'));
  print(describe([1, 2]));
  print(describe([7, 8, 9]));
  print(describe({'k': 3}));
  print(describe((1, 'b')));
  print(describe((x: 4)));
  print(describe(null));
  print(describe(2.5));
  var v = 7;
  switch (v) {
    case 1 || 2:
      print('one or two');
    case > 5 && < 10:
      print('mid');
    default:
      print('def');
  }
  Object obj = [1, 'two'];
  if (obj case [int n, String s]) print('match $n $s');
  if (obj case [int n, int m]) print('no'); else print('else');
  if (obj is List<Object> && obj.length == 2) print('is list');
  var m = <String, int>{'a': 1};
  switch (m) {
    case {'a': int av}:
      print('a=$av');
  }
  var (String s1, int n1) = ('z', 26);
  print('$s1$n1');
  for (var (i, j) in [(1, 2), (3, 4)]) print(i + j);
  var tuple = (1, 2);
  var (x1, y1) = tuple;
  (x1, y1) = (y1, x1);
  print('$x1 $y1');
  print(switch (Mode.fast) { Mode.fast => 'F', Mode.slow => 'S' });
  const k = 3;
  print(switch (k) { 3 => 'three', _ => 'x' });
  print([for (var i = 0; i < 2; i++) i, ...?null]);
  String? sn;
  print(sn?.toUpperCase() ?? 'none');
  print(sn ??= 'set');
  print(sn);
  int? ni = 2;
  print(ni + 1);
  print(ni.isEven);
  print(identical(p, p.self()));
  print(p.hashCode == p.hashCode);
  print(p.runtimeType);
  print(Sq(1) is Shape);
  var list = [Sq(1), Ci(2)];
  print(list.whereType<Ci>().length);
  print(list.first.runtimeType);
}

sealed class Shape {}

class Sq extends Shape {
  final double s;
  Sq(this.s);
}

class Ci extends Shape {
  final double r;
  Ci(this.r);
}

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

enum Mode { fast, slow }

typedef Pair = (int, String);

void main() {
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
  if (obj case [int n, int m]) {
    print('no $n $m');
  } else {
    print('else');
  }
  var m = <String, int>{'a': 1};
  switch (m) {
    case {'a': int av}:
      print('a=$av');
  }
  var (String s1, int n1) = ('z', 26);
  print('$s1$n1');
  for (var (i, j) in [(1, 2), (3, 4)]) {
    print(i + j);
  }
  var tuple = (1, 2);
  var (x1, y1) = tuple;
  (x1, y1) = (y1, x1);
  print('$x1 $y1');
  print(switch (Mode.fast) { Mode.fast => 'F', Mode.slow => 'S' });
  const k = 3;
  print(switch (k) { 3 => 'three', _ => 'x' });
  const origem = (0, 0);
  print(switch ((0, 0)) { origem => 'origem', _ => 'outro' });
  String label(Shape s) {
    switch (s) {
      case Sq(s: var side) when side > 1:
        return 'big square';
      case Sq():
        return 'square';
      case Ci():
        return 'circle';
    }
  }
  print(label(Sq(2)));
  print(label(Sq(1)));
  print(label(Ci(1)));
  var list = [Sq(1), Ci(2)];
  print(list.whereType<Ci>().length);
  print(list.first.runtimeType);
  var nested = [[1, 2], [3, 4]];
  for (var [x, y] in nested) print(x * y);
  if (nested case [[1, _], [_, var last]]) print('last $last');
}

import 'a.dart' as a;
import 'b.dart' as b;

enum Color { red, blue; static const disguise = Matcher(); }
enum Enhanced<T> {
  first<int>(1), second<int>(1);
  const Enhanced(this.value);
  final T value;
}
const alias = Color.red;
mixin Marker { int get marker => 1; }
enum Mixed with Marker { first }
int rightCalls = 0;
int leftCalls = 0;
class Candidate {
  bool operator ==(Object other) { rightCalls++; return true; }
  int get hashCode => 0;
}
class Matcher {
  const Matcher();
  bool operator ==(Object other) {
    leftCalls++;
    if (other == 'throw') throw StateError('expected');
    return other == 'hit';
  }
  int get hashCode => 0;
}
const matcher = Matcher();
int direct(Object? value) => switch (value) {
  Color.red => 1, Color.blue => 2, _ => 0,
};
int imported(Object? value) => switch (value) {
  a.Twin.first => 3, b.Twin.first => 4, _ => 0,
};
int enhanced(Object? value) => switch (value) {
  Enhanced.first => 5, Enhanced.second => 6, _ => 0,
};
int aliased(Object? value) => switch (value) { alias => 7, _ => 0 };
int mixed(Object? value) => switch (value) { Mixed.first => 8, _ => 0 };
int masked(Object? value) => switch (value) { Color.disguise => 9, _ => 0 };
int shadowed(Object? value) {
  const red = matcher;
  return switch (value) { red => 10, _ => 0 };
}
bool custom(Object? value) => switch (value) { matcher => true, _ => false };
bool relational(Object? value) => switch (value) { == Color.red => true, _ => false };
void main() {
  final results = <Object>[
    direct(Color.red), direct(Color.blue), direct(null), direct(1),
    direct(1.0), direct('red'), direct(Candidate()),
    imported(a.Twin.first), imported(b.Twin.first), imported(Color.red),
    enhanced(Enhanced.first), enhanced(Enhanced.second),
    aliased(Color.red), aliased(Color.blue),
    mixed(Mixed.first), mixed(null),
    custom('hit'), custom('miss'),
  ];
  try { custom('throw'); } on StateError { results.add('throw'); }
  results.add(masked('hit'));
  results.add(shadowed('hit'));
  results.add(rightCalls);
  results.add(relational(Candidate()));
  results.add(rightCalls);
  results.add(leftCalls);
  print(results.join('|'));
}

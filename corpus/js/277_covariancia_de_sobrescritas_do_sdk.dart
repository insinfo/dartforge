// A parameter of an SDK method that overrides a covariant parameter of a
// supertype is checked on entry, as the DDC does: `JSNumber.compareTo(num b)`
// and `JSString.compareTo(String other)` override `Comparable<T>.compareTo(T)`.
// The production profile compiled them without the check, and `[1, 'a'].sort()`
// sorted the list instead of throwing `TypeError`.
void attempt(String label, void Function() f) {
  try {
    f();
    print('$label: no error');
  } catch (e) {
    print('$label: ${e.runtimeType}');
  }
}

void main() {
  attempt('mixed sort', () => (<Object>[1, 'a']..sort()));
  attempt('string first', () => (<Object>['a', 1]..sort()));
  attempt('int compare string', () => Comparable.compare(1, 'a'));
  attempt('string compare int', () => Comparable.compare('a', 2.5));
  Comparable<Object> c = 3 as dynamic;
  attempt('through Comparable<Object>', () => print(c.compareTo('x')));
  attempt('same kind', () => print(Comparable.compare(2, 1.5)));
  final numbers = <Object>[3, 1.5, 2]..sort();
  print(numbers);
}

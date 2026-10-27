// A constructor of a generic class torn off without type arguments is a
// canonical generic function; instantiated (explicitly or by a context) it is
// equal for equal types and canonical for constant ones. Through a type alias
// a proper rename is the class's tear-off, another alias is generic over its
// own parameters, and its instantiation is the class's. prefix.C<int> is a
// type literal.
import 'main.dart' as self;

class C<T> {
  final T x;
  C(this.x);
}

typedef Direct<T> = C<T>;
typedef Bounded<T extends num> = C<T>;

C<int> Function(int) context(C<int> Function(int) f) => f;

void main() {
  print(C.new is C<T> Function<T>(T));
  print(identical(C.new, C.new));
  print(identical(C<int>.new, C<int>.new));
  print(C<int>.new == C<num>.new);
  print(identical(context(C.new), C<int>.new));
  print(identical(Direct.new, C.new));
  print(Bounded.new is C<T> Function<T extends num>(T));
  print(identical(context(Bounded.new), C<int>.new));
  print(identical(Bounded<int>.new, C<int>.new));
  print(identical(self.C<int>, C<int>));
  var generic = C.new;
  print(generic<String>('s').x);
}

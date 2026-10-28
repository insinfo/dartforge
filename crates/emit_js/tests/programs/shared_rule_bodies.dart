// Unrelated classes with the same supertypes share one rti rule body in
// production; each must stay a class of its own in subtype checks.

abstract class FirstGenericInterface<T> {}

abstract class SecondGenericInterface<T> {}

class A implements FirstGenericInterface<String>, SecondGenericInterface<double> {}

class B implements FirstGenericInterface<String>, SecondGenericInterface<double> {}

class C implements FirstGenericInterface<String>, SecondGenericInterface<double> {}

class D implements FirstGenericInterface<String>, SecondGenericInterface<double> {}

class E implements FirstGenericInterface<String>, SecondGenericInterface<double> {}

class F extends A {}

void main() {
  final lists = <Object>[<A>[A()], <B>[B()], <C>[], <D>[], <E>[], <F>[F()]];
  for (final l in lists) {
    print([
      l is List<A>,
      l is List<B>,
      l is List<C>,
      l is List<D>,
      l is List<E>,
      l is List<F>,
      l is List<FirstGenericInterface<String>>,
      l is List<SecondGenericInterface<double>>,
      l is List<SecondGenericInterface<int>>,
    ]);
  }
  Object f = (A a) => a;
  print(f is A Function(B));
  print(f is Object Function(F));
}

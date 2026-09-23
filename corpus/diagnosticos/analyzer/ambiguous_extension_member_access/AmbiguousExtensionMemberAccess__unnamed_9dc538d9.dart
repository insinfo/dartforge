class A {}
class B {}
class C extends A implements B {}

extension on List<A> {
  int call() => 0;
}

extension on List<B> {
  int call() => 0;
}

int f(List<C> x) => x();
//                  ^
// [diag.ambiguousExtensionMemberAccessTwo] A member named 'call' is defined in 'extension on List<A>' and 'extension on List<B>', and neither is more specific.

// Additional calls to avoid UNUSED_ELEMENT
int g(List<A> x) => x();
int h(List<B> x) => x();

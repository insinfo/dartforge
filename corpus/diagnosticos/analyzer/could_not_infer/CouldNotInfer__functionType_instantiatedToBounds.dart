class A<X extends A<X>> {}

void foo<X extends Y, Y extends A<X>>() {}

void f() {
  foo();
//^^^
// [diag.couldNotInfer] Couldn't infer type parameter 'Y'.\n'A<Object?>' doesn't conform to the bound 'A<A<Object?>>', instantiated from 'A<X>' using type arguments [A<Object?>, A<Object?>].
}

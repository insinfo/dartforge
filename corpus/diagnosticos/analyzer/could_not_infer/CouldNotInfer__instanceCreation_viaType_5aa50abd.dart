class C<X> {
  C();
  factory C.foo() => C();
  factory C.bar() = C;
}
typedef G<X> = X Function(X);
typedef A<X extends G<C<X>>> = C<X>;

void f() {
  A(); // Error.
//^
// [diag.couldNotInfer] Couldn't infer type parameter 'X'.\n\nTried to infer 'C<Object?> Function(C<Never>)' for 'X' which doesn't work:\n  Type parameter 'X' is declared to extend 'C<X> Function(C<X>)' producing 'C<C<Object?> Function(C<Never>)> Function(C<C<Object?> Function(C<Never>)>)'.\n\nConsider passing explicit type argument(s) to the generic.
  A.foo(); // Error.
//^^^^^
// [diag.couldNotInfer] Couldn't infer type parameter 'X'.\n\nTried to infer 'C<Object?> Function(C<Never>)' for 'X' which doesn't work:\n  Type parameter 'X' is declared to extend 'C<X> Function(C<X>)' producing 'C<C<Object?> Function(C<Never>)> Function(C<C<Object?> Function(C<Never>)>)'.\n\nConsider passing explicit type argument(s) to the generic.
  A.bar(); // Error.
//^^^^^
// [diag.couldNotInfer] Couldn't infer type parameter 'X'.\n\nTried to infer 'C<Object?> Function(C<Never>)' for 'X' which doesn't work:\n  Type parameter 'X' is declared to extend 'C<X> Function(C<X>)' producing 'C<C<Object?> Function(C<Never>)> Function(C<C<Object?> Function(C<Never>)>)'.\n\nConsider passing explicit type argument(s) to the generic.
}

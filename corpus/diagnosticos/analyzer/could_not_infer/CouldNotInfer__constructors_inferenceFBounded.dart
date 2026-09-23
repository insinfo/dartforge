class C<T> {}

class P<T extends C<T>, U extends C<U>> {
  T t;
  U u;
  P(this.t, this.u);
  P._();
//^^^
// [diag.notInitializedNonNullableInstanceFieldConstructor] Non-nullable instance field 't' must be initialized.
// [diag.notInitializedNonNullableInstanceFieldConstructor] Non-nullable instance field 'u' must be initialized.
  P<U, T> get reversed => new P(u, t);
}

main() {
  P._();
//^
// [context 1] The raw type was instantiated as 'P<C<Object?>, C<Object?>>', and is not regular-bounded.
// [context 2] The raw type was instantiated as 'P<C<Object?>, C<Object?>>', and is not regular-bounded.
//^^^
// [diag.couldNotInfer] Couldn't infer type parameter 'T'.\n\nTried to infer 'C<Object?>' for 'T' which doesn't work:\n  Type parameter 'T' is declared to extend 'C<T>' producing 'C<C<Object?>>'.\n\nConsider passing explicit type argument(s) to the generic.
// [diag.couldNotInfer] Couldn't infer type parameter 'U'.\n\nTried to infer 'C<Object?>' for 'U' which doesn't work:\n  Type parameter 'U' is declared to extend 'C<U>' producing 'C<C<Object?>>'.\n\nConsider passing explicit type argument(s) to the generic.
//^
// [diag.typeArgumentNotMatchingBounds][context 1] 'C<Object?>' doesn't conform to the bound 'C<C<Object?>>' of the type parameter 'T'.
// [diag.typeArgumentNotMatchingBounds][context 2] 'C<Object?>' doesn't conform to the bound 'C<C<Object?>>' of the type parameter 'U'.
}

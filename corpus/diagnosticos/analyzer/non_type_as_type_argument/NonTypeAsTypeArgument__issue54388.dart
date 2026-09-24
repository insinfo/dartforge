sealed class Option<A> {}

final class None implements Option<A> {
//                                 ^
// [diag.nonTypeAsTypeArgument] The name 'A' isn't a type, so it can't be used as a type argument.
  const None();
}

A doOption<A>(
  A Function(B Function<B>(Option<B>)) eval,
) {
  return eval(
    <B>(option) => switch (option) {
      None() => throw 7,
    },
  );
}

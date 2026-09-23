const annotation = null;

class Annotation {
  final String message;
  const Annotation(this.message);
}

class A<E> {}

class C {
  m() => new A<@annotation @Annotation("test") C>();
//             ^^^^^^^^^^^
// [diag.annotationOnTypeArgument] Type arguments can't have annotations because they aren't declarations.
//                         ^^^^^^^^^^^^^^^^^^^
// [diag.annotationOnTypeArgument] Type arguments can't have annotations because they aren't declarations.
}

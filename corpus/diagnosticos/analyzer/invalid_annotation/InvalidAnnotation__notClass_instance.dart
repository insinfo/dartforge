class Property {
  final int value;
  const Property(this.value);
}

const Property property = const Property(42);

@property(123)
// [diag.invalidAnnotation][column 1][length 14] Annotation must be either a const variable reference or const constructor invocation.
main() {
}

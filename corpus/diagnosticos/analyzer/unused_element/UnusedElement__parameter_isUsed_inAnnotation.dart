class _MyAnnotation {
  const _MyAnnotation({this.value});
  final int? value;
}

@_MyAnnotation(value: 42)
void fn() {}

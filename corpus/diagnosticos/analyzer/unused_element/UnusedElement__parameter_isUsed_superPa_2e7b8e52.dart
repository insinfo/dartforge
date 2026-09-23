class _BaseOptional([final int? value]);

class SubOptional(super.value) extends _BaseOptional;

void main() {
  print(SubOptional(42));
}

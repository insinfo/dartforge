class _BaseNamedOptional({final int? value});

class SubNamedOptional({super.value}) extends _BaseNamedOptional;

void main() {
  print(SubNamedOptional(value: 42));
}

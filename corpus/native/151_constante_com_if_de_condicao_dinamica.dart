// Coleções constantes com `if` cuja condição é uma constante de outro tipo
// declarado (`const dynamic sim = true`) são canônicas como as demais:
// `identical` com a mesma constante escrita sem o `if`.
const dynamic dynamicTrue = true;
const dynamic list = [1, 2, 3];
void main() {
  print(identical(const [1], const <int>[if (dynamicTrue) 1]));
  print(identical(const [1], const [if (true) [1]].first));
  print(identical(const {1: 1}, const [if (true) {1: 1}].first));
  print(identical(list, const <int>[if (true) ...<int>[1, 2], if (false) 9 else ...<int>[3]]));
  print(identical(const [1], const <int>[if (true) if (true) 1, if (true) if (false) 9]));
  print(identical(const [1], const <int>[if (false) 9 else if (true) 1]));
  const dynamic nonInt = "s";
  print(identical(const <int>[1], const <int>[if (true) 1, if (false) nonInt]));
  print(identical(const [1], const [[1]].first));
}

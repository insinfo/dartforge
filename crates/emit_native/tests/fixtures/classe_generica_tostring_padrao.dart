class Caixa<T> {
  final T v;
  Caixa(this.v);
}

class Simples {
  final int x = 1;
}

void main() {
  print(Caixa<int>(1).toString() == "Instance of 'Caixa<int>'");
  print(Caixa<String>('a').toString());
  print(Caixa<Caixa<int>>(Caixa(2)).toString());
  print(Simples().toString() == "Instance of 'Simples'");
}

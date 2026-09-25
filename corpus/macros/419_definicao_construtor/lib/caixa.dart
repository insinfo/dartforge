import 'tres.dart';

class Caixa {
  int valor = 1;

  @Tres()
  Caixa() { valor = 2; }
}

String resultado() => '${Caixa().valor}';

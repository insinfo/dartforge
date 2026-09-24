import 'triplo.dart';

class Calculo {
  @Triplo()
  int dobro(int valor) => valor * 2;
}

String resultado() => '${Calculo().dobro(2)}';

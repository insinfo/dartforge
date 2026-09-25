import 'triplo.dart';

@Triplo()
int dobro(int valor) => valor * 2;

String resultado() => '${dobro(2)}';

import 'extra.dart';

@Extra()
int dobro(int valor) => valor * 2;

String resultado() => '${dobro(2)}:${triplo(2)}';

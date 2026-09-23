part of 'main.dart';

import 'dart:math' as m;

part 'interna.dart';

augment class C implements Descrevivel {
  int f2 = proximo('f2');
  String descrever() => 'C com f1=$f1';
}

augment int dobro(int x) => m.max(x, 0) * 2;

import 'dart:html';

import 'package:ngdart/angular.dart';

/// Ouvintes em vários elementos: o oficial escreve todos no fim do
/// `build()`, depois de criar os nós, em ordem de documento — o do pai antes
/// dos filhos, e os de um mesmo elemento na ordem dos atributos.
@Component(
  selector: 'c08-eventos-irmaos',
  templateUrl: 'c08_eventos_irmaos.html',
)
class C08EventosIrmaos {
  void a() {}
  void b(Event e) {}
  void c() {}
  void d() {}
}
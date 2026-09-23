import 'package:ngdart/angular.dart';

import 'd06_filho_saida.dart';

/// `@Output` do filho: `subscription_N` no `build()`, depois dos eventos do
/// próprio elemento, e `initSubscriptions` no fim; o handler complexo vira
/// `_handleEvent_N`.
@Component(
  selector: 'd06-usa-saida',
  templateUrl: 'd06_usa_saida.html',
  directives: [D06FilhoSaida],
)
class D06UsaSaida {
  bool aberto = true;
  String ultimo = '';

  void guardar(String v) {
    ultimo = v;
  }

  void clicou() {}
}

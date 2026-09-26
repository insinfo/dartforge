import 'dart:html';

import 'package:ngdart/angular.dart';

/// Sonda: eventos com `$event`.
@Component(
  selector: 'i27-evento-evento',
  templateUrl: 'i27_evento_evento.html',
  directives: [coreDirectives],
)
class I27EventoEvento {
  String? valor;
  void sair() {}
  void enviar(Event e) {}
}

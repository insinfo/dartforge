import 'dart:html';

import 'package:ngdart/angular.dart';

/// `(click)="fazer(\$event)"` — o manipulador recebe o evento.
@Component(
  selector: 'c04-evento-com-argumento',
  templateUrl: 'c04_evento_com_argumento.html',
)
class C04EventoComArgumento {
  void fazer(Event e) {}
}

import 'package:ngdart/angular.dart';

/// Evento fora da lista do DOM (`keyup.enter`, `keydown.esc`): vai pelo
/// `eventManager` do ngdart (`visitCustomEvent`), não pelo
/// `addEventListener` do elemento.
@Component(
  selector: 'c14-keyup-enter',
  templateUrl: 'c14_keyup_enter.html',
)
class C14KeyupEnter {
  void enviar() {}
  void tecla(Object? e) {}
}

import 'dart:html';

import 'package:ngdart/angular.dart';

/// Sonda: `@HostListener` em componente.
@Component(
  selector: 'i18-host-listener-componente',
  templateUrl: 'i18_host_listener_componente.html',
  directives: [coreDirectives],
)
class I18HostListenerComponente {
  @HostListener('click', [r'$event'])
  void clicou(Event e) {}
}

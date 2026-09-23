import 'dart:html';

import 'package:ngdart/angular.dart';

/// Dois `@HostListener`: um com `$event` explícito e um sem parâmetro. Saem
/// depois dos ouvintes do template, na ordem de declaração.
@Component(
  selector: 'b21-host-listener-explicito',
  templateUrl: 'b21_host_listener_explicito.html',
)
class B21HostListenerExplicito {
  @HostListener('keydown', [r'$event'])
  void tecla(KeyboardEvent e) {}

  @HostListener('blur')
  void saiu() {}

  void clicou() {}
}
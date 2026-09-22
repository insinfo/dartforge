import 'dart:html';

import 'package:ngdart/angular.dart';

/// `@HostListener` — o que fez o gerador errar antes de recusar.
@Component(
  selector: 'b04-host-listener',
  templateUrl: 'b04_host_listener.html',
)
class B04HostListener {
  @HostListener('click')
  void clicou(Event e) {}
}

import 'dart:html';

import 'package:ngdart/angular.dart';

/// `@HostListener` de diretiva no mesmo evento que o template escreve (o
/// `(click)` do botão com `[liTooltip]` no limitless_ui): um handler só,
/// com a ação do template antes (`mergeEvents`).
@Directive(selector: '[j70-dica]')
class J70Dica {
  @HostListener('click', [r'$event'])
  void clicou(MouseEvent e) {}

  @HostListener('mouseenter')
  void entrou() {}

  @HostListener('focus', ['1'])
  void focou(int n) {}
}

@Directive(selector: '[j70-rastro]')
class J70Rastro {
  @HostListener('click')
  void marcar() {}
}

@Component(
  selector: 'j70-evento-e-host-listener',
  templateUrl: 'j70_evento_e_host_listener.html',
  directives: [J70Dica, J70Rastro],
)
class J70EventoEHostListener {
  int n = 0;

  void alternar() {}
}

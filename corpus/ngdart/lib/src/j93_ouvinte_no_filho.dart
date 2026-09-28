import 'dart:html';

import 'package:ngdart/angular.dart';

@Component(
  selector: 'j93-filho',
  template: '<i></i>',
)
class J93Filho {
  @HostListener('click')
  void clicado() {}
}

@Directive(selector: '[j93-dica]')
class J93Dica {
  @HostListener('mouseenter')
  void entrar() {}

  @HostListener('click', [r'$event'])
  void clicar(Event e) {}
}

/// `@HostListener` de diretiva no elemento de um componente filho, também
/// com o mesmo evento escrito no template e no próprio filho.
@Component(
  selector: 'j93-ouvinte-no-filho',
  template: '<j93-filho j93-dica></j93-filho>'
      '<j93-filho j93-dica (click)="avisar()"></j93-filho>',
  directives: [J93Filho, J93Dica],
)
class J93OuvinteNoFilho {
  void avisar() {}
}

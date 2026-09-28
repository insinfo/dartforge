import 'dart:html';

import 'package:ngdart/angular.dart';

@Directive(selector: '[j50-pai]')
class J50Pai {
  void alternar() {}
}

/// Base com `this.x` no construtor.
class J50Base implements OnInit {
  J50Base(this.pai, this.elemento);

  final J50Pai pai;
  final Element elemento;

  @override
  void ngOnInit() {}
}

/// Parâmetros `super.x` sem tipo (o `liDropdownToggle` do limitless_ui): o
/// tipo é o do parâmetro repassado no construtor da base.
@Directive(selector: '[j50-gatilho]')
class J50Gatilho extends J50Base {
  J50Gatilho(super.pai, super.elemento);

  @HostListener('click')
  void clicar() => pai.alternar();
}

@Component(
  selector: 'j50-parametro-super',
  template: '<div j50-pai><button j50-gatilho>x</button></div>',
  directives: [J50Pai, J50Gatilho],
)
class J50ParametroSuper {}

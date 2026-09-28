import 'package:ngdart/angular.dart';

/// Diretiva de formulário acima dos campos (o `LiFormDirective` do
/// limitless_ui), com `@HostBinding`: o campo dela guarda o `XNgCd`.
@Directive(selector: '[j68-form]')
class J68Form {
  @HostBinding('class.enviado')
  bool enviado = false;
}

@Directive(selector: '[j68-grupo]')
class J68Grupo {}

/// Filho que injeta os provedores de elementos acima: lidos de lá, sem o
/// injetor de fora (nem `debugInjectorWrap`).
@Component(
  selector: 'j68-campo',
  template: '<i></i>',
)
class J68Campo {
  J68Campo(this.form, @Optional() this.grupo);

  final J68Form form;
  final J68Grupo? grupo;
}

@Component(
  selector: 'j68-filho-injeta-de-cima',
  templateUrl: 'j68_filho_injeta_de_cima.html',
  directives: [J68Form, J68Grupo, J68Campo, NgIf],
)
class J68FilhoInjetaDeCima {
  bool mostrar = true;
}

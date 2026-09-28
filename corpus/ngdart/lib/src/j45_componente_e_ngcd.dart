import 'package:ngdart/angular.dart';

/// Diretiva com `@HostBinding` antes do componente no arquivo (o
/// `li-tab`/`LiTabxDirective` do limitless_ui): a classe `XNgCd` sai no
/// mesmo `.template.dart` do componente.
@Directive(selector: '[j45-ativo]')
class J45Ativo {
  @HostBinding('class.ativo')
  bool ativo = true;

  @HostBinding('attr.aria-selected')
  String get selecionado => '$ativo';
}

@Component(
  selector: 'j45-componente-e-ngcd',
  template: '<b j45-ativo>x</b>',
  directives: [J45Ativo, J45Depois],
)
class J45ComponenteENgcd {}

/// Outra, depois do componente.
@Directive(selector: '[j45-depois]')
class J45Depois {
  @HostBinding('class.depois')
  bool depois = false;
}

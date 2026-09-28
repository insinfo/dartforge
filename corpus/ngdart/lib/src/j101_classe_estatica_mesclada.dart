import 'package:ngdart/angular.dart';

/// `_mergeHtmlAndDirectiveAttrs`: o `class` estático de um componente filho
/// é mesclado com o `class` escrito no elemento (`interpolate2`); sem
/// `class` escrito, fica só no construtor da visão dele; atributo que não é
/// `class`/`style` escrito vence o do componente.
@Component(
  selector: 'j101-alternador',
  template: '<i>x</i>',
)
class J101Alternador {
  @HostBinding('class')
  static const hostClass = 'themeable';

  @HostBinding('attr.role')
  static const hostRole = 'switch';
}

@Component(
  selector: 'j101-usa',
  template: '''
<j101-alternador class="a b" role="button" title="t"></j101-alternador>
<j101-alternador></j101-alternador>
<j101-alternador class="{{ extra }}"></j101-alternador>''',
  directives: [J101Alternador],
)
class J101Usa {
  String extra = 'z';
}

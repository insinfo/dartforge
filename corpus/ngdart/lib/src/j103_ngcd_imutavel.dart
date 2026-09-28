import 'package:ngdart/angular.dart';

/// `XNgCd` com `@HostBinding` em campo `final` (`isImmutable`): escrito uma
/// vez no `if (firstCheck)`, antes dos dinâmicos, sem campo `_expr_N` mas
/// com o índice dele — o `FocusItemDirective`/`AutoIdDirective` do
/// ngcomponents; também herdado.
@Directive(selector: '[j103Item]')
class J103Item {
  @HostBinding('attr.role')
  final String role;

  J103Item(@Attribute('role') String? role) : role = role ?? 'listitem';

  @HostBinding('attr.tabindex')
  String tabIndex = '0';

  @HostBinding('class.fixo')
  final bool fixo = true;
}

class J103Base {
  @HostBinding('attr.id')
  final String id = 'j103';
}

@Directive(selector: '[j103Id]')
class J103Id extends J103Base {
  @HostBinding('attr.title')
  String? titulo;
}

@Component(
  selector: 'j103-usa',
  template: '<p j103Item role="x"></p><span j103Id></span>',
  directives: [J103Item, J103Id],
)
class J103Usa {}

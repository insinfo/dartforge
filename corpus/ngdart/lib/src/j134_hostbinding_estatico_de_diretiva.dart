import 'package:ngdart/angular.dart';

/// `@HostBinding` em membro estático de diretiva: o imutável fora de
/// `class.x`/`style.x` é `hostAttribute` (fora da `XNgCd`); o `style.x`
/// entra na `XNgCd` lido pela classe (`StaticRead`), no `if (firstCheck)`,
/// com `?.toString()` (o `_TypeResolver` dá `dynamic`) — o
/// `MaterialTooltipSourceDirective`.
@Directive(selector: '[j134Fonte]')
class J134Fonte {
  @HostBinding('style.cursor')
  static const cursor = 'pointer';

  @HostBinding('tabIndex')
  static const indice = 0;

  @HostBinding('class.ativa')
  bool ativa = true;
}

import 'dart:html';

import 'package:ngdart/angular.dart';

/// Construtor com `@Optional() @SkipSelf()`, `@Optional() @Inject(token)`,
/// `@Attribute` (em parâmetro e em `this.x`), `ChangeDetectorRef` e o
/// elemento (o `MaterialDropdownSelectComponent`): na hospedeira o atributo é
/// `null`, o token vai ao `injectorGetOptional`.
const j136Token = OpaqueToken<bool>('j136');

class J136Tamanho {}

class J136Gerador {}

@Component(
  selector: 'j136-caixa',
  template: 'x',
)
class J136Caixa {
  final String? papel;

  J136Caixa(
      @Optional() J136Gerador? gerador,
      @Optional() @SkipSelf() J136Tamanho? tamanho,
      @Optional() @Inject(j136Token) bool? rtl,
      @Attribute('classe') String? classe,
      @Attribute('papel') this.papel,
      ChangeDetectorRef cd,
      HtmlElement el);
}

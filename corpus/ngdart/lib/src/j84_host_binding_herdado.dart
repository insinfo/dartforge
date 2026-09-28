import 'package:ngdart/angular.dart';

/// `@HostBinding` herdado (o `LiDropdownToggleDirective extends
/// LiDropdownAnchorDirective` do limitless_ui): o `XNgCd` da subclasse
/// inclui as ligações da superclasse, primeiro.
@Directive(selector: '[j84-ancora]')
class J84Ancora {
  bool aberto = false;

  @HostBinding('class.aberto')
  bool get classeAberto => aberto;

  @HostBinding('attr.aria-expanded')
  String get expandido => aberto ? 'true' : 'false';

  @HostBinding('style.width.px')
  int? largura;
}

@Directive(selector: '[j84-alternador]')
class J84Alternador extends J84Ancora {
  @HostListener('click')
  void clicar() {
    aberto = !aberto;
  }
}

@Directive(selector: '[j84-item]')
class J84Item extends J84Ancora {
  @HostBinding('class.item')
  bool item = true;
}

/// Sem anotação nenhuma, só herda: também ganha o `XNgCd`.
@Directive(selector: '[j84-mudo]')
class J84Mudo extends J84Ancora {}

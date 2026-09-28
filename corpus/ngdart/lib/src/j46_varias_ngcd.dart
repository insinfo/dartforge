import 'package:ngdart/angular.dart';

/// Várias diretivas com `@HostBinding` num arquivo só de diretivas (o
/// `li-dropdown` do limitless_ui): as classes `XNgCd` na ordem do fonte,
/// com a tabela de imports compartilhada.
@Directive(selector: '[j46-um]')
class J46Um {
  @HostBinding('class.um')
  bool um = true;
}

@Directive(selector: '[j46-sem]')
class J46Sem {}

@Directive(selector: '[j46-dois]')
class J46Dois {
  @HostBinding('attr.role')
  String papel = 'menu';

  @HostBinding('style.width.px')
  int largura = 10;
}

@Pipe('j46Pipe')
class J46Pipe {
  String transform(String v) => v;
}

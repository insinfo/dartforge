import 'package:ngdart/angular.dart';

/// `exportAs:` num `@Component` (o `#x="nome"` de quem usa vale o
/// componente) e `class.x.y` (no `@HostBinding` e em `[class.x.y]`): só a
/// segunda parte conta (`template_parser.dart:100-102`).
@Component(
  selector: 'j119-gaveta',
  exportAs: 'gaveta',
  template: '<i>g</i>',
)
class J119Gaveta {
  bool aberta = false;
}

@Directive(selector: '[j119Icone]')
class J119Icone {
  @HostBinding('class.basic-icon.if')
  bool basico = true;
}

@Component(
  selector: 'j119-usa',
  template: '''
<j119-gaveta #g="gaveta"></j119-gaveta>
<span j119Icone [class.aberta.if]="g.aberta">{{ g.aberta }}</span>''',
  directives: [J119Gaveta, J119Icone],
)
class J119Usa {}

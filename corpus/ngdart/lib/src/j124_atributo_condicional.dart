import 'package:ngdart/angular.dart';

/// `[attr.x.if]`: `updateAttribute(el, 'x', (v ? '' : null))`, sempre
/// `updateAttribute` (`update_statement_visitor.dart:63-94`); `[attr.ns:x]`:
/// `updateAttributeNS` (`template_parser.dart:94-98`).
@Component(
  selector: 'j124-usa',
  template: '''<div [attr.aberto.if]="aberto" [attr.xlink:href]="ref" [attr.foo:bar]="ref"></div>
<button [attr.disabled.if]="true">b</button>''',
)
class J124Usa {
  bool aberto = true;
  String ref = '#a';
}

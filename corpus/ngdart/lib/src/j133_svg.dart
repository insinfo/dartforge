import 'package:ngdart/angular.dart';

/// Elementos SVG: o namespace implícito da tag `svg` passa aos descendentes
/// (`_NamespaceVisitor`), o nó sai com `doc.createElementNS(uri, tag)` e o
/// `append` à parte, e não é HTML (`Element`, `updateChildClassNonHtml`,
/// `addShimE`) — também dentro de `*`.
@Component(
  selector: 'j133-usa',
  template: '''<div class="a">
  <svg xmlns="http://www.w3.org/2000/svg" width="24" viewBox="0 0 24 24" class="icone">
    <path d="M12 2z" [attr.fill]="cor"/>
    <g *ngIf="x"><circle r="3" [class.ativo]="x"></circle></g>
  </svg>
</div>''',
  styles: ['.a { color: red; }'],
  directives: [NgIf],
)
class J133Usa {
  bool x = true;
  String cor = 'red';
}

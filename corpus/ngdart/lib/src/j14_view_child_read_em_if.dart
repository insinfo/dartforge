import 'package:ngdart/angular.dart';

/// Consulta dinâmica com `read:`: o `ElementRef` de um elemento dentro de
/// `*ngIf`, e a lista de `ElementRef` de um `*ngFor`.
@Component(
  selector: 'j14-view-child-read-em-if',
  templateUrl: 'j14_view_child_read_em_if.html',
  directives: [coreDirectives],
)
class J14ViewChildReadEmIf {
  bool a = true;
  List<int> itens = [1, 2];

  @ViewChild('e', read: ElementRef)
  ElementRef? elemento;

  @ViewChildren('l', read: ElementRef)
  List<ElementRef>? lista;
}

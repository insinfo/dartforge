import 'dart:html';

import 'package:ngdart/angular.dart';

/// Sonda: `@ViewChildren`.
@Component(
  selector: 'i15-view-children',
  templateUrl: 'i15_view_children.html',
  directives: [coreDirectives],
)
class I15ViewChildren {
  @ViewChildren('a')
  List<Element>? caixas;
}

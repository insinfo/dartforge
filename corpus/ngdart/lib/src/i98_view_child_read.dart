import 'package:ngdart/angular.dart';

/// Sonda: `@ViewChild(.., read: ..)`.
@Component(
  selector: 'i98-view-child-read',
  templateUrl: 'i98_view_child_read.html',
)
class I98ViewChildRead {
  @ViewChild('caixa', read: ElementRef)
  ElementRef? ref;
}

// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j104_ngcd_so_diretiva.dart';
import 'package:ngdart/src/core/change_detection/directive_change_detector.dart' as import0;
import 'j104_ngcd_so_diretiva.dart' as import1;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import2;
import 'dart:html' as import3;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import4;
import 'package:ngdart/src/runtime/check_binding.dart' as import5;

class J104ListaNgCd extends import0.DirectiveChangeDetector {
  final import1.J104Lista instance;
  Object? _expr_1;
  J104ListaNgCd(this.instance);
  void detectHostChanges(import2.RenderView view, import3.Element el) {
    bool firstCheck = view.firstCheck;
    if (firstCheck) {
      if ((this.instance.role != null)) {
        import4.updateAttribute(el, 'role', this.instance.role);
      }
    }
    final currVal_1 = this.instance.ignorar;
    if (import5.checkBinding(this._expr_1, currVal_1, null, null)) {
      import4.updateAttribute(el, 'ignoreUpAndDown', currVal_1);
      this._expr_1 = currVal_1;
    }
  }
}

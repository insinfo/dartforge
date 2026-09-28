// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j84_host_binding_herdado.dart';
import 'package:ngdart/src/core/change_detection/directive_change_detector.dart' as import0;
import 'j84_host_binding_herdado.dart' as import1;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import2;
import 'dart:html' as import3;
import 'package:ngdart/src/runtime/check_binding.dart' as import4;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import5;

class J84AncoraNgCd extends import0.DirectiveChangeDetector {
  final import1.J84Ancora instance;
  Object? _expr_0;
  Object? _expr_1;
  Object? _expr_2;
  J84AncoraNgCd(this.instance);
  void detectHostChanges(import2.RenderView view, import3.Element el) {
    final currVal_0 = this.instance.classeAberto;
    if (import4.checkBinding(this._expr_0, currVal_0, null, null)) {
      import5.updateClassBindingNonHtml(el, 'aberto', currVal_0);
      this._expr_0 = currVal_0;
    }
    final currVal_1 = this.instance.expandido;
    if (import4.checkBinding(this._expr_1, currVal_1, null, null)) {
      import5.updateAttribute(el, 'aria-expanded', currVal_1);
      this._expr_1 = currVal_1;
    }
    final currVal_2 = this.instance.largura;
    if (import4.checkBinding(this._expr_2, currVal_2, null, null)) {
      el.style.setProperty('width', ((currVal_2 == null) ? null : (currVal_2.toString() + 'px')));
      this._expr_2 = currVal_2;
    }
  }
}

class J84AlternadorNgCd extends import0.DirectiveChangeDetector {
  final import1.J84Alternador instance;
  Object? _expr_0;
  Object? _expr_1;
  Object? _expr_2;
  J84AlternadorNgCd(this.instance);
  void detectHostChanges(import2.RenderView view, import3.Element el) {
    final currVal_0 = this.instance.classeAberto;
    if (import4.checkBinding(this._expr_0, currVal_0, null, null)) {
      import5.updateClassBindingNonHtml(el, 'aberto', currVal_0);
      this._expr_0 = currVal_0;
    }
    final currVal_1 = this.instance.expandido;
    if (import4.checkBinding(this._expr_1, currVal_1, null, null)) {
      import5.updateAttribute(el, 'aria-expanded', currVal_1);
      this._expr_1 = currVal_1;
    }
    final currVal_2 = this.instance.largura;
    if (import4.checkBinding(this._expr_2, currVal_2, null, null)) {
      el.style.setProperty('width', ((currVal_2 == null) ? null : (currVal_2.toString() + 'px')));
      this._expr_2 = currVal_2;
    }
  }
}

class J84ItemNgCd extends import0.DirectiveChangeDetector {
  final import1.J84Item instance;
  Object? _expr_0;
  Object? _expr_1;
  Object? _expr_2;
  Object? _expr_3;
  J84ItemNgCd(this.instance);
  void detectHostChanges(import2.RenderView view, import3.Element el) {
    final currVal_0 = this.instance.classeAberto;
    if (import4.checkBinding(this._expr_0, currVal_0, null, null)) {
      import5.updateClassBindingNonHtml(el, 'aberto', currVal_0);
      this._expr_0 = currVal_0;
    }
    final currVal_1 = this.instance.expandido;
    if (import4.checkBinding(this._expr_1, currVal_1, null, null)) {
      import5.updateAttribute(el, 'aria-expanded', currVal_1);
      this._expr_1 = currVal_1;
    }
    final currVal_2 = this.instance.largura;
    if (import4.checkBinding(this._expr_2, currVal_2, null, null)) {
      el.style.setProperty('width', ((currVal_2 == null) ? null : (currVal_2.toString() + 'px')));
      this._expr_2 = currVal_2;
    }
    final currVal_3 = this.instance.item;
    if (import4.checkBinding(this._expr_3, currVal_3, null, null)) {
      import5.updateClassBindingNonHtml(el, 'item', currVal_3);
      this._expr_3 = currVal_3;
    }
  }
}

class J84MudoNgCd extends import0.DirectiveChangeDetector {
  final import1.J84Mudo instance;
  Object? _expr_0;
  Object? _expr_1;
  Object? _expr_2;
  J84MudoNgCd(this.instance);
  void detectHostChanges(import2.RenderView view, import3.Element el) {
    final currVal_0 = this.instance.classeAberto;
    if (import4.checkBinding(this._expr_0, currVal_0, null, null)) {
      import5.updateClassBindingNonHtml(el, 'aberto', currVal_0);
      this._expr_0 = currVal_0;
    }
    final currVal_1 = this.instance.expandido;
    if (import4.checkBinding(this._expr_1, currVal_1, null, null)) {
      import5.updateAttribute(el, 'aria-expanded', currVal_1);
      this._expr_1 = currVal_1;
    }
    final currVal_2 = this.instance.largura;
    if (import4.checkBinding(this._expr_2, currVal_2, null, null)) {
      el.style.setProperty('width', ((currVal_2 == null) ? null : (currVal_2.toString() + 'px')));
      this._expr_2 = currVal_2;
    }
  }
}

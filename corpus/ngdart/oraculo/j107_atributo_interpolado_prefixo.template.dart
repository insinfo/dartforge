// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j107_atributo_interpolado_prefixo.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j107_atributo_interpolado_prefixo.dart' as import1;
import 'dart:html' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/src/runtime/interpolate.dart' as import8;
import 'package:ngdart/src/runtime/check_binding.dart' as import9;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import11;

final List<Object> styles$J107AtributoInterpoladoPrefixo = const [];

class ViewJ107AtributoInterpoladoPrefixo0 extends import0.ComponentView<import1.J107AtributoInterpoladoPrefixo> {
  Object? _expr_0;
  Object? _expr_1;
  Object? _expr_2;
  Object? _expr_3;
  Object? _expr_5;
  Object? _expr_6;
  late final import2.DivElement _el_0;
  late final import2.HtmlElement _el_1;
  static import3.ComponentStyles? _componentStyles;
  ViewJ107AtributoInterpoladoPrefixo0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import2.document.createElement('j107-atributo-interpolado-prefixo'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/j107_atributo_interpolado_prefixo.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import2.document;
    this._el_0 = import7.appendDiv(doc, parentRenderNode);
    this._el_1 = import7.appendSpan(doc, parentRenderNode);
    final _text_2 = import7.appendText(this._el_1, 'x');
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    bool firstCheck = this.firstCheck;
    if (firstCheck) {
      import7.setAttribute(this._el_0, 'role', import8.interpolateString0(_ctx.papel)) /* REF:asset:corpus_ngdart/lib/src/j107_atributo_interpolado_prefixo.dart:537:558 */;
    }
    final currVal_0 = import8.interpolateString0(_ctx.rotulo);
    if (import9.checkBinding(this._expr_0, currVal_0, '{{rotulo}}', 'asset:corpus_ngdart/lib/src/j107_atributo_interpolado_prefixo.dart')) {
      import7.setAttribute(this._el_0, 'aria-label', currVal_0) /* REF:asset:corpus_ngdart/lib/src/j107_atributo_interpolado_prefixo.dart:408:436 */;
      this._expr_0 = currVal_0;
    }
    final currVal_1 = _ctx.desligado;
    if (import9.checkBinding(this._expr_1, currVal_1, '{{desligado}}', 'asset:corpus_ngdart/lib/src/j107_atributo_interpolado_prefixo.dart')) {
      import7.setAttribute(this._el_0, 'aria-disabled', import8.interpolate0(currVal_1)) /* REF:asset:corpus_ngdart/lib/src/j107_atributo_interpolado_prefixo.dart:437:471 */;
      this._expr_1 = currVal_1;
    }
    final currVal_2 = _ctx.minimo;
    if (import9.checkBinding(this._expr_2, currVal_2, '{{minimo}}', 'asset:corpus_ngdart/lib/src/j107_atributo_interpolado_prefixo.dart')) {
      import7.setAttribute(this._el_0, 'aria-valuemin', import8.interpolate0(currVal_2)) /* REF:asset:corpus_ngdart/lib/src/j107_atributo_interpolado_prefixo.dart:477:508 */;
      this._expr_2 = currVal_2;
    }
    final currVal_3 = import8.interpolateString0(_ctx.identificador);
    if (import9.checkBinding(this._expr_3, currVal_3, '{{identificador}}', 'asset:corpus_ngdart/lib/src/j107_atributo_interpolado_prefixo.dart')) {
      import7.setProperty(this._el_0, 'id', currVal_3) /* REF:asset:corpus_ngdart/lib/src/j107_atributo_interpolado_prefixo.dart:509:531 */;
      this._expr_3 = currVal_3;
    }
    final currVal_5 = import8.interpolate0((!_ctx.desligado));
    if (import9.checkBinding(this._expr_5, currVal_5, '{{!desligado}}', 'asset:corpus_ngdart/lib/src/j107_atributo_interpolado_prefixo.dart')) {
      import7.setAttribute(this._el_0, 'aria-hidden', currVal_5) /* REF:asset:corpus_ngdart/lib/src/j107_atributo_interpolado_prefixo.dart:559:592 */;
      this._expr_5 = currVal_5;
    }
    final currVal_6 = _ctx.minimo;
    if (import9.checkBinding(this._expr_6, currVal_6, '{{minimo}}', 'asset:corpus_ngdart/lib/src/j107_atributo_interpolado_prefixo.dart')) {
      this._el_1.style.setProperty('width', ((import8.interpolate0(currVal_6) == null) ? null : (import8.interpolate0(currVal_6).toString() + 'px'))) /* REF:asset:corpus_ngdart/lib/src/j107_atributo_interpolado_prefixo.dart:606:633 */;
      this._expr_6 = currVal_6;
    }
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$J107AtributoInterpoladoPrefixo, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J107AtributoInterpoladoPrefixoNgFactory = ComponentFactory<import1.J107AtributoInterpoladoPrefixo>('j107-atributo-interpolado-prefixo', viewFactory_J107AtributoInterpoladoPrefixoHost0);
ComponentFactory<import1.J107AtributoInterpoladoPrefixo> get J107AtributoInterpoladoPrefixoNgFactory {
  return _J107AtributoInterpoladoPrefixoNgFactory;
}

ComponentFactory<import1.J107AtributoInterpoladoPrefixo> createJ107AtributoInterpoladoPrefixoFactory() {
  return ComponentFactory('j107-atributo-interpolado-prefixo', viewFactory_J107AtributoInterpoladoPrefixoHost0);
}

final List<Object> styles$J107AtributoInterpoladoPrefixoHost = const [];

class _ViewJ107AtributoInterpoladoPrefixoHost0 extends import11.HostView<import1.J107AtributoInterpoladoPrefixo> {
  @override
  void build() {
    this.componentView = ViewJ107AtributoInterpoladoPrefixo0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J107AtributoInterpoladoPrefixo();
    this.initRootNode(_el_0);
  }
}

import11.HostView<import1.J107AtributoInterpoladoPrefixo> viewFactory_J107AtributoInterpoladoPrefixoHost0() {
  return _ViewJ107AtributoInterpoladoPrefixoHost0();
}

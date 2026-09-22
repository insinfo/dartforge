// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'a15_referencia.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'a15_referencia.dart' as import1;
import 'package:ngdart/src/runtime/text_binding.dart' as import2;
import 'dart:html' as import3;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import4;
import 'package:ngdart/src/core/linker/views/view.dart' as import5;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import6;
import 'package:ngdart/src/utilities.dart' as import7;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import8;
import 'package:ngdart/src/runtime/interpolate.dart' as import9;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import11;

final List<Object> styles$A15Referencia = const [];

class ViewA15Referencia0 extends import0.ComponentView<import1.A15Referencia> {
  final import2.TextBinding _textBinding_3 = import2.TextBinding();
  late final import3.DivElement _el_0;
  static import4.ComponentStyles? _componentStyles;
  ViewA15Referencia0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import3.document.createElement('a15-referencia'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/a15_referencia.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import3.document;
    this._el_0 = import8.appendDiv(doc, parentRenderNode);
    final _text_1 = import8.appendText(this._el_0, 'oi');
    final _el_2 = import8.appendSpan(doc, parentRenderNode);
    _el_2.append(this._textBinding_3.element);
  }

  @override
  void detectChangesInternal() {
    final local_caixa = this._el_0;
    this._textBinding_3.updateText(import9.interpolate0(local_caixa.id)) /* REF:package:corpus_ngdart/src/a15_referencia.html:26:38 */;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$A15Referencia, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _A15ReferenciaNgFactory = ComponentFactory<import1.A15Referencia>('a15-referencia', viewFactory_A15ReferenciaHost0);
ComponentFactory<import1.A15Referencia> get A15ReferenciaNgFactory {
  return _A15ReferenciaNgFactory;
}

ComponentFactory<import1.A15Referencia> createA15ReferenciaFactory() {
  return ComponentFactory('a15-referencia', viewFactory_A15ReferenciaHost0);
}

final List<Object> styles$A15ReferenciaHost = const [];

class _ViewA15ReferenciaHost0 extends import11.HostView<import1.A15Referencia> {
  @override
  void build() {
    this.componentView = ViewA15Referencia0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.A15Referencia();
    this.initRootNode(_el_0);
  }
}

import11.HostView<import1.A15Referencia> viewFactory_A15ReferenciaHost0() {
  return _ViewA15ReferenciaHost0();
}

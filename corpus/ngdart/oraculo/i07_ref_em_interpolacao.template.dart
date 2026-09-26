// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i07_ref_em_interpolacao.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i07_ref_em_interpolacao.dart' as import1;
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

final List<Object> styles$I07RefEmInterpolacao = const [];

class ViewI07RefEmInterpolacao0 extends import0.ComponentView<import1.I07RefEmInterpolacao> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  late final import3.InputElement _el_0;
  static import4.ComponentStyles? _componentStyles;
  ViewI07RefEmInterpolacao0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import3.document.createElement('i07-ref-em-interpolacao'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/i07_ref_em_interpolacao.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import3.document;
    this._el_0 = import8.appendElement<import3.InputElement>(doc, parentRenderNode, 'input');
    parentRenderNode.append(this._textBinding_1.element);
    this._el_0.addEventListener('keyup', this.eventHandler1(this._handleEvent_0));
  }

  @override
  void detectChangesInternal() {
    final local_campo = this._el_0;
    this._textBinding_1.updateText(import9.interpolate0(local_campo.value)) /* REF:package:corpus_ngdart/src/i07_ref_em_interpolacao.html:26:41 */;
  }

  void _handleEvent_0($event) {
    0;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$I07RefEmInterpolacao, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I07RefEmInterpolacaoNgFactory = ComponentFactory<import1.I07RefEmInterpolacao>('i07-ref-em-interpolacao', viewFactory_I07RefEmInterpolacaoHost0);
ComponentFactory<import1.I07RefEmInterpolacao> get I07RefEmInterpolacaoNgFactory {
  return _I07RefEmInterpolacaoNgFactory;
}

ComponentFactory<import1.I07RefEmInterpolacao> createI07RefEmInterpolacaoFactory() {
  return ComponentFactory('i07-ref-em-interpolacao', viewFactory_I07RefEmInterpolacaoHost0);
}

final List<Object> styles$I07RefEmInterpolacaoHost = const [];

class _ViewI07RefEmInterpolacaoHost0 extends import11.HostView<import1.I07RefEmInterpolacao> {
  @override
  void build() {
    this.componentView = ViewI07RefEmInterpolacao0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I07RefEmInterpolacao();
    this.initRootNode(_el_0);
  }
}

import11.HostView<import1.I07RefEmInterpolacao> viewFactory_I07RefEmInterpolacaoHost0() {
  return _ViewI07RefEmInterpolacaoHost0();
}

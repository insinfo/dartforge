// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j25_membros_estaticos.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j25_membros_estaticos.dart' as import1;
import 'package:ngdart/src/runtime/text_binding.dart' as import2;
import 'dart:html' as import3;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import4;
import 'package:ngdart/src/core/linker/views/view.dart' as import5;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import6;
import 'package:ngdart/src/utilities.dart' as import7;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import8;
import 'package:ngdart/src/runtime/interpolate.dart' as import9;
import 'package:ngdart/src/runtime/check_binding.dart' as import10;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import12;

final List<Object> styles$J25MembrosEstaticos = const [];

class ViewJ25MembrosEstaticos0 extends import0.ComponentView<import1.J25MembrosEstaticos> {
  final import2.TextBinding _textBinding_4 = import2.TextBinding();
  final import2.TextBinding _textBinding_7 = import2.TextBinding();
  Object? _expr_1;
  Object? _expr_2;
  late final import3.HtmlElement _el_0;
  late final import3.HtmlElement _el_3;
  late final import3.HtmlElement _el_6;
  static import4.ComponentStyles? _componentStyles;
  ViewJ25MembrosEstaticos0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import3.document.createElement('j25-membros-estaticos'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/j25_membros_estaticos.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import3.document;
    this._el_0 = import8.appendSpan(doc, parentRenderNode);
    final _text_1 = import8.appendText(this._el_0, import9.interpolate0(import1.J25MembrosEstaticos.fixo));
    final _text_2 = import8.appendText(parentRenderNode, '\n');
    this._el_3 = import8.appendElement<import3.HtmlElement>(doc, parentRenderNode, 'b');
    this._el_3.append(this._textBinding_4.element);
    final _text_5 = import8.appendText(parentRenderNode, '\n');
    this._el_6 = import8.appendElement<import3.HtmlElement>(doc, parentRenderNode, 'i');
    this._el_6.append(this._textBinding_7.element);
    final _text_8 = import8.appendText(parentRenderNode, '\n');
    final _el_9 = import8.appendElement<import3.ButtonElement>(doc, parentRenderNode, 'button');
    final _text_10 = import8.appendText(_el_9, '+');
    _el_9.addEventListener('click', this.eventHandler1(this._handleEvent_0));
  }

  @override
  void detectChangesInternal() {
    bool firstCheck = this.firstCheck;
    if (firstCheck) {
      if ((import1.J25MembrosEstaticos.constante != null)) {
        import8.setProperty(this._el_0, 'title', import1.J25MembrosEstaticos.constante) /* REF:package:corpus_ngdart/src/j25_membros_estaticos.html:6:25 */;
      }
    }
    final currVal_1 = import1.J25MembrosEstaticos.mutavel;
    if (import10.checkBinding(this._expr_1, currVal_1, 'mutavel', 'package:corpus_ngdart/src/j25_membros_estaticos.html')) {
      import8.setProperty(this._el_3, 'title', currVal_1) /* REF:package:corpus_ngdart/src/j25_membros_estaticos.html:45:62 */;
      this._expr_1 = currVal_1;
    }
    this._textBinding_4.updateText(import9.interpolate0(import1.J25MembrosEstaticos.calculado)) /* REF:package:corpus_ngdart/src/j25_membros_estaticos.html:63:76 */;
    final currVal_2 = import1.J25MembrosEstaticos.talvez;
    if (import10.checkBinding(this._expr_2, currVal_2, 'talvez', 'package:corpus_ngdart/src/j25_membros_estaticos.html')) {
      import8.updateAttribute(this._el_6, 'data-x', currVal_2) /* REF:package:corpus_ngdart/src/j25_membros_estaticos.html:84:106 */;
      this._expr_2 = currVal_2;
    }
    this._textBinding_7.updateText(import9.interpolate0(import1.J25MembrosEstaticos.contador)) /* REF:package:corpus_ngdart/src/j25_membros_estaticos.html:107:119 */;
  }

  void _handleEvent_0($event) {
    import1.J25MembrosEstaticos.contador = 1;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$J25MembrosEstaticos, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J25MembrosEstaticosNgFactory = ComponentFactory<import1.J25MembrosEstaticos>('j25-membros-estaticos', viewFactory_J25MembrosEstaticosHost0);
ComponentFactory<import1.J25MembrosEstaticos> get J25MembrosEstaticosNgFactory {
  return _J25MembrosEstaticosNgFactory;
}

ComponentFactory<import1.J25MembrosEstaticos> createJ25MembrosEstaticosFactory() {
  return ComponentFactory('j25-membros-estaticos', viewFactory_J25MembrosEstaticosHost0);
}

final List<Object> styles$J25MembrosEstaticosHost = const [];

class _ViewJ25MembrosEstaticosHost0 extends import12.HostView<import1.J25MembrosEstaticos> {
  @override
  void build() {
    this.componentView = ViewJ25MembrosEstaticos0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J25MembrosEstaticos();
    this.initRootNode(_el_0);
  }
}

import12.HostView<import1.J25MembrosEstaticos> viewFactory_J25MembrosEstaticosHost0() {
  return _ViewJ25MembrosEstaticosHost0();
}

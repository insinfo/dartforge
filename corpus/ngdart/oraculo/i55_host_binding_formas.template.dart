// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i55_host_binding_formas.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i55_host_binding_formas.dart' as import1;
import 'package:ngdart/src/runtime/text_binding.dart' as import2;
import 'dart:html' as import3;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import4;
import 'package:ngdart/src/core/linker/views/view.dart' as import5;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import6;
import 'package:ngdart/src/utilities.dart' as import7;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import8;
import 'package:ngdart/src/runtime/check_binding.dart' as import9;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import11;

final List<Object> styles$I55HostBindingFormas = const [];

class ViewI55HostBindingFormas0 extends import0.ComponentView<import1.I55HostBindingFormas> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  Object? _expr_0;
  Object? _expr_1;
  Object? _expr_2;
  Object? _expr_3;
  late final import3.HtmlElement _el_0;
  static import4.ComponentStyles? _componentStyles;
  ViewI55HostBindingFormas0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import3.document.createElement('i55-host-binding-formas'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/i55_host_binding_formas.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import3.document;
    this._el_0 = import8.appendElement<import3.HtmlElement>(doc, parentRenderNode, 'p');
    this._el_0.append(this._textBinding_1.element);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.papel;
    if (import9.checkBinding(this._expr_0, currVal_0, 'papel', 'package:corpus_ngdart/src/i55_host_binding_formas.html')) {
      import8.setProperty(this._el_0, 'title', currVal_0) /* REF:package:corpus_ngdart/src/i55_host_binding_formas.html:3:18 */;
      this._expr_0 = currVal_0;
    }
    this._textBinding_1.updateTextWithPrimitive(_ctx.n) /* REF:package:corpus_ngdart/src/i55_host_binding_formas.html:19:24 */;
  }

  void detectHostChanges(bool firstCheck) {
    final _ctx = this.ctx;
    if (firstCheck) {
      if ((_ctx.fixo != null)) {
        import8.updateClassBindingNonHtml(this.rootElement, 'fixo', _ctx.fixo);
      }
    }
    final currVal_1 = _ctx.rotulo;
    if (import9.checkBinding(this._expr_1, currVal_1, null, null)) {
      import8.updateAttribute(this.rootElement, 'aria-label', currVal_1);
      this._expr_1 = currVal_1;
    }
    final currVal_2 = _ctx.ativo;
    if (import9.checkBinding(this._expr_2, currVal_2, null, null)) {
      import8.updateClassBindingNonHtml(this.rootElement, 'ativo', currVal_2);
      this._expr_2 = currVal_2;
    }
    final currVal_3 = _ctx.papel;
    if (import9.checkBinding(this._expr_3, currVal_3, null, null)) {
      import8.updateAttribute(this.rootElement, 'role', currVal_3);
      this._expr_3 = currVal_3;
    }
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$I55HostBindingFormas, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I55HostBindingFormasNgFactory = ComponentFactory<import1.I55HostBindingFormas>('i55-host-binding-formas', viewFactory_I55HostBindingFormasHost0);
ComponentFactory<import1.I55HostBindingFormas> get I55HostBindingFormasNgFactory {
  return _I55HostBindingFormasNgFactory;
}

ComponentFactory<import1.I55HostBindingFormas> createI55HostBindingFormasFactory() {
  return ComponentFactory('i55-host-binding-formas', viewFactory_I55HostBindingFormasHost0);
}

final List<Object> styles$I55HostBindingFormasHost = const [];

class _ViewI55HostBindingFormasHost0 extends import11.HostView<import1.I55HostBindingFormas> {
  @override
  void build() {
    this.componentView = ViewI55HostBindingFormas0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I55HostBindingFormas();
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    bool firstCheck = this.firstCheck;
    this.componentView.detectHostChanges(firstCheck);
    this.componentView.detectChanges();
  }
}

import11.HostView<import1.I55HostBindingFormas> viewFactory_I55HostBindingFormasHost0() {
  return _ViewI55HostBindingFormasHost0();
}

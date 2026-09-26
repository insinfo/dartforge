// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i59_contador.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i59_contador.dart' as import1;
import 'package:ngdart/src/runtime/text_binding.dart' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'dart:html' as import7;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import8;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import10;

final List<Object> styles$I59Contador = const [];

class ViewI59Contador0 extends import0.ComponentView<import1.I59Contador> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  static import3.ComponentStyles? _componentStyles;
  ViewI59Contador0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('i59-contador'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/i59_contador.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import7.document;
    final _el_0 = import8.appendElement<import7.HtmlElement>(doc, parentRenderNode, 'b');
    _el_0.append(this._textBinding_1.element);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    this._textBinding_1.updateTextWithPrimitive(_ctx.valor) /* REF:package:corpus_ngdart/src/i59_contador.html:3:12 */;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$I59Contador, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I59ContadorNgFactory = ComponentFactory<import1.I59Contador>('i59-contador', viewFactory_I59ContadorHost0);
ComponentFactory<import1.I59Contador> get I59ContadorNgFactory {
  return _I59ContadorNgFactory;
}

ComponentFactory<import1.I59Contador> createI59ContadorFactory() {
  return ComponentFactory('i59-contador', viewFactory_I59ContadorHost0);
}

final List<Object> styles$I59ContadorHost = const [];

class _ViewI59ContadorHost0 extends import10.HostView<import1.I59Contador> {
  @override
  void build() {
    this.componentView = ViewI59Contador0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I59Contador();
    this.initRootNode(_el_0);
  }
}

import10.HostView<import1.I59Contador> viewFactory_I59ContadorHost0() {
  return _ViewI59ContadorHost0();
}

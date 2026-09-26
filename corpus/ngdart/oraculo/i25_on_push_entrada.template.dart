// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i25_on_push_entrada.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i25_on_push_entrada.dart' as import1;
import 'package:ngdart/src/runtime/text_binding.dart' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'dart:html' as import7;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import8;
import 'package:ngdart/src/runtime/interpolate.dart' as import9;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import11;

final List<Object> styles$I25OnPushEntrada = const [];

class ViewI25OnPushEntrada0 extends import0.ComponentView<import1.I25OnPushEntrada> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  static import3.ComponentStyles? _componentStyles;
  ViewI25OnPushEntrada0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkOnce) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('i25-on-push-entrada'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/i25_on_push_entrada.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import7.document;
    final _el_0 = import8.appendElement<import7.HtmlElement>(doc, parentRenderNode, 'p');
    _el_0.append(this._textBinding_1.element);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    this._textBinding_1.updateText(import9.interpolateString0(_ctx.titulo)) /* REF:package:corpus_ngdart/src/i25_on_push_entrada.html:3:13 */;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$I25OnPushEntrada, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I25OnPushEntradaNgFactory = ComponentFactory<import1.I25OnPushEntrada>('i25-on-push-entrada', viewFactory_I25OnPushEntradaHost0);
ComponentFactory<import1.I25OnPushEntrada> get I25OnPushEntradaNgFactory {
  return _I25OnPushEntradaNgFactory;
}

ComponentFactory<import1.I25OnPushEntrada> createI25OnPushEntradaFactory() {
  return ComponentFactory('i25-on-push-entrada', viewFactory_I25OnPushEntradaHost0);
}

final List<Object> styles$I25OnPushEntradaHost = const [];

class _ViewI25OnPushEntradaHost0 extends import11.HostView<import1.I25OnPushEntrada> {
  @override
  void build() {
    this.componentView = ViewI25OnPushEntrada0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I25OnPushEntrada();
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    bool changed = false;
    if (changed) {
      this.componentView.markAsCheckOnce();
    }
    this.componentView.detectChanges();
  }
}

import11.HostView<import1.I25OnPushEntrada> viewFactory_I25OnPushEntradaHost0() {
  return _ViewI25OnPushEntradaHost0();
}

// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'c04_evento_com_argumento.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'c04_evento_com_argumento.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;

final List<Object> styles$C04EventoComArgumento = const [];

class ViewC04EventoComArgumento0 extends import0.ComponentView<import1.C04EventoComArgumento> {
  static import2.ComponentStyles? _componentStyles;
  ViewC04EventoComArgumento0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('c04-evento-com-argumento'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/c04_evento_com_argumento.dart' : null);
  }

  @override
  void build() {
    final _ctx = this.ctx;
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendElement<import6.ButtonElement>(doc, parentRenderNode, 'button');
    final _text_1 = import7.appendText(_el_0, 'ok');
    _el_0.addEventListener('click', this.eventHandler1(_ctx.fazer));
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$C04EventoComArgumento, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _C04EventoComArgumentoNgFactory = ComponentFactory<import1.C04EventoComArgumento>('c04-evento-com-argumento', viewFactory_C04EventoComArgumentoHost0);
ComponentFactory<import1.C04EventoComArgumento> get C04EventoComArgumentoNgFactory {
  return _C04EventoComArgumentoNgFactory;
}

ComponentFactory<import1.C04EventoComArgumento> createC04EventoComArgumentoFactory() {
  return ComponentFactory('c04-evento-com-argumento', viewFactory_C04EventoComArgumentoHost0);
}

final List<Object> styles$C04EventoComArgumentoHost = const [];

class _ViewC04EventoComArgumentoHost0 extends import9.HostView<import1.C04EventoComArgumento> {
  @override
  void build() {
    this.componentView = ViewC04EventoComArgumento0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.C04EventoComArgumento();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.C04EventoComArgumento> viewFactory_C04EventoComArgumentoHost0() {
  return _ViewC04EventoComArgumentoHost0();
}

// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'botao.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'botao.dart' as import1;
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

final List<Object> styles$DepBotao = const [];

class ViewDepBotao0 extends import0.ComponentView<import1.DepBotao> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  static import3.ComponentStyles? _componentStyles;
  ViewDepBotao0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('dep-botao'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:dep_ng/lib/src/botao.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import7.document;
    final _el_0 = import8.appendElement<import7.ButtonElement>(doc, parentRenderNode, 'button');
    _el_0.append(this._textBinding_1.element);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    this._textBinding_1.updateText(import9.interpolateString0(_ctx.rotulo)) /* REF:package:dep_ng/src/botao.html:8:18 */;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$DepBotao, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _DepBotaoNgFactory = ComponentFactory<import1.DepBotao>('dep-botao', viewFactory_DepBotaoHost0);
ComponentFactory<import1.DepBotao> get DepBotaoNgFactory {
  return _DepBotaoNgFactory;
}

ComponentFactory<import1.DepBotao> createDepBotaoFactory() {
  return ComponentFactory('dep-botao', viewFactory_DepBotaoHost0);
}

final List<Object> styles$DepBotaoHost = const [];

class _ViewDepBotaoHost0 extends import11.HostView<import1.DepBotao> {
  @override
  void build() {
    this.componentView = ViewDepBotao0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.DepBotao();
    this.initRootNode(_el_0);
  }
}

import11.HostView<import1.DepBotao> viewFactory_DepBotaoHost0() {
  return _ViewDepBotaoHost0();
}

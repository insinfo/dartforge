// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j117_view_child_diretiva.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j117_view_child_diretiva.dart' as import1;
import 'dart:html' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/src/devtools.dart' as import8;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import10;
import 'package:ngdart/src/core/change_detection/directive_change_detector.dart' as import11;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import12;
import 'package:ngdart/src/runtime/check_binding.dart' as import13;

final List<Object> styles$J117ViewChildDiretiva = const [];

class ViewJ117ViewChildDiretiva0 extends import0.ComponentView<import1.J117ViewChildDiretiva> {
  late final import1.J117Envoltorio _J117Envoltorio_0_5;
  late final J117BotaoNgCd _J117Botao_1_5;
  late final import2.HtmlElement _el_1;
  static import3.ComponentStyles? _componentStyles;
  ViewJ117ViewChildDiretiva0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import2.document.createElement('j117-view-child-diretiva'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/j117_view_child_diretiva.dart' : null);
  }

  @override
  void build() {
    final _ctx = this.ctx;
    final parentRenderNode = this.initViewRoot();
    final doc = import2.document;
    final _el_0 = import7.appendDiv(doc, parentRenderNode);
    import7.setAttribute(_el_0, 'j117Envoltorio', '');
    this._J117Envoltorio_0_5 = import1.J117Envoltorio();
    if (import8.isDevToolsEnabled) {
      import8.Inspector.instance.registerDirective(_el_0, this._J117Envoltorio_0_5);
    }
    this._el_1 = import7.appendSpan(doc, _el_0);
    import7.setAttribute(this._el_1, 'j117Botao', '');
    this._J117Botao_1_5 = J117BotaoNgCd(import1.J117Botao());
    if (import8.isDevToolsEnabled) {
      import8.Inspector.instance.registerDirective(this._el_1, this._J117Botao_1_5.instance);
    }
    final _text_2 = import7.appendText(this._el_1, 'b');
    _el_0.addEventListener('click', this.eventHandler0(_ctx.clicou));
    _ctx.botao = this._J117Botao_1_5.instance;
    _ctx.envoltorio = this._J117Envoltorio_0_5;
  }

  @override
  void detectChangesInternal() {
    this._J117Botao_1_5.detectHostChanges(this, this._el_1);
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$J117ViewChildDiretiva, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J117ViewChildDiretivaNgFactory = ComponentFactory<import1.J117ViewChildDiretiva>('j117-view-child-diretiva', viewFactory_J117ViewChildDiretivaHost0);
ComponentFactory<import1.J117ViewChildDiretiva> get J117ViewChildDiretivaNgFactory {
  return _J117ViewChildDiretivaNgFactory;
}

ComponentFactory<import1.J117ViewChildDiretiva> createJ117ViewChildDiretivaFactory() {
  return ComponentFactory('j117-view-child-diretiva', viewFactory_J117ViewChildDiretivaHost0);
}

final List<Object> styles$J117ViewChildDiretivaHost = const [];

class _ViewJ117ViewChildDiretivaHost0 extends import10.HostView<import1.J117ViewChildDiretiva> {
  @override
  void build() {
    this.componentView = ViewJ117ViewChildDiretiva0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J117ViewChildDiretiva();
    this.initRootNode(_el_0);
  }
}

import10.HostView<import1.J117ViewChildDiretiva> viewFactory_J117ViewChildDiretivaHost0() {
  return _ViewJ117ViewChildDiretivaHost0();
}

class J117BotaoNgCd extends import11.DirectiveChangeDetector {
  final import1.J117Botao instance;
  Object? _expr_0;
  J117BotaoNgCd(this.instance);
  void detectHostChanges(import12.RenderView view, import2.Element el) {
    final currVal_0 = this.instance.papel;
    if (import13.checkBinding(this._expr_0, currVal_0, null, null)) {
      import7.updateAttribute(el, 'role', currVal_0);
      this._expr_0 = currVal_0;
    }
  }
}

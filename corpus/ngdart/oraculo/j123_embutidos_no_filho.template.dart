// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j123_embutidos_no_filho.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j123_embutidos_no_filho.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import8;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/devtools.dart' as import10;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import11;
import 'package:ngdart/src/runtime/check_binding.dart' as import12;

final List<Object> styles$J123Botao = const [];

class ViewJ123Botao0 extends import0.ComponentView<import1.J123Botao> {
  static import2.ComponentStyles? _componentStyles;
  ViewJ123Botao0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j123-botao'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j123_embutidos_no_filho.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this.project(parentRenderNode, 0);
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J123Botao, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J123BotaoNgFactory = ComponentFactory<import1.J123Botao>('j123-botao', viewFactory_J123BotaoHost0);
ComponentFactory<import1.J123Botao> get J123BotaoNgFactory {
  return _J123BotaoNgFactory;
}

ComponentFactory<import1.J123Botao> createJ123BotaoFactory() {
  return ComponentFactory('j123-botao', viewFactory_J123BotaoHost0);
}

final List<Object> styles$J123BotaoHost = const [];

class _ViewJ123BotaoHost0 extends import8.HostView<import1.J123Botao> {
  @override
  void build() {
    this.componentView = ViewJ123Botao0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J123Botao();
    this.initRootNode(_el_0);
  }
}

import8.HostView<import1.J123Botao> viewFactory_J123BotaoHost0() {
  return _ViewJ123BotaoHost0();
}

final List<Object> styles$J123Usa = const [];

class ViewJ123Usa0 extends import0.ComponentView<import1.J123Usa> {
  late final ViewJ123Botao0 _compView_0;
  late final ViewContainer _appEl_0;
  late final import1.J123Botao _J123Botao_0_8;
  late final import1.J123Dica _J123Dica_0_9;
  Object? _expr_0;
  static import2.ComponentStyles? _componentStyles;
  ViewJ123Usa0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j123-usa'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j123_embutidos_no_filho.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = ViewJ123Botao0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this._appEl_0 = ViewContainer(0, null, this, _el_0);
    this._J123Botao_0_8 = import1.J123Botao();
    this._J123Dica_0_9 = import1.J123Dica(this._appEl_0, _el_0, this._appEl_0, this._compView_0, null);
    if (import10.isDevToolsEnabled) {
      import10.Inspector.instance.registerDirective(_el_0, this._J123Dica_0_9);
    }
    final _text_1 = import11.createText('x');
    this._compView_0.createAndProject(this._J123Botao_0_8, [
      <Object>[_text_1]
    ]);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.t;
    if (import12.checkBinding(this._expr_0, currVal_0, 't', 'asset:corpus_ngdart/lib/src/j123_embutidos_no_filho.dart')) {
      if (import10.isDevToolsEnabled) {
        import10.Inspector.instance.recordInput(this._J123Dica_0_9, 'j123Dica', currVal_0);
      }
      this._J123Dica_0_9.texto = currVal_0 /* REF:asset:corpus_ngdart/lib/src/j123_embutidos_no_filho.dart:755:769 */;
      this._expr_0 = currVal_0;
    }
    this._appEl_0.detectChangesInNestedViews();
    this._compView_0.detectChanges();
  }

  @override
  void destroyInternal() {
    this._appEl_0.destroyNestedViews();
    this._compView_0.destroyInternalState();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J123Usa, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J123UsaNgFactory = ComponentFactory<import1.J123Usa>('j123-usa', viewFactory_J123UsaHost0);
ComponentFactory<import1.J123Usa> get J123UsaNgFactory {
  return _J123UsaNgFactory;
}

ComponentFactory<import1.J123Usa> createJ123UsaFactory() {
  return ComponentFactory('j123-usa', viewFactory_J123UsaHost0);
}

final List<Object> styles$J123UsaHost = const [];

class _ViewJ123UsaHost0 extends import8.HostView<import1.J123Usa> {
  @override
  void build() {
    this.componentView = ViewJ123Usa0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J123Usa();
    this.initRootNode(_el_0);
  }
}

import8.HostView<import1.J123Usa> viewFactory_J123UsaHost0() {
  return _ViewJ123UsaHost0();
}

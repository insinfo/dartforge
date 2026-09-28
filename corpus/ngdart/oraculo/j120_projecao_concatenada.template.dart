// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j120_projecao_concatenada.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j120_projecao_concatenada.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'package:ngdart/src/devtools.dart' as import12;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import14;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import15;

final List<Object> styles$J120Caixa = const [];

class ViewJ120Caixa0 extends import0.ComponentView<import1.J120Caixa> {
  static import2.ComponentStyles? _componentStyles;
  ViewJ120Caixa0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j120-caixa'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j120_projecao_concatenada.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendDiv(doc, parentRenderNode);
    this.project(_el_0, 0);
    this.project(parentRenderNode, 1);
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J120Caixa, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J120CaixaNgFactory = ComponentFactory<import1.J120Caixa>('j120-caixa', viewFactory_J120CaixaHost0);
ComponentFactory<import1.J120Caixa> get J120CaixaNgFactory {
  return _J120CaixaNgFactory;
}

ComponentFactory<import1.J120Caixa> createJ120CaixaFactory() {
  return ComponentFactory('j120-caixa', viewFactory_J120CaixaHost0);
}

final List<Object> styles$J120CaixaHost = const [];

class _ViewJ120CaixaHost0 extends import9.HostView<import1.J120Caixa> {
  @override
  void build() {
    this.componentView = ViewJ120Caixa0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J120Caixa();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J120Caixa> viewFactory_J120CaixaHost0() {
  return _ViewJ120CaixaHost0();
}

final List<Object> styles$J120Usa = const [];

class ViewJ120Usa0 extends import0.ComponentView<import1.J120Usa> {
  late final ViewJ120Caixa0 _compView_0;
  late final import1.J120Caixa _J120Caixa_0_5;
  late final import1.J120Det _J120Det_0_6;
  late final ViewContainer _appEl_3;
  late final NgIf _NgIf_3_9;
  static import2.ComponentStyles? _componentStyles;
  ViewJ120Usa0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j120-usa'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j120_projecao_concatenada.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = ViewJ120Caixa0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    import7.setAttribute(_el_0, 'j120Det', '');
    this._J120Caixa_0_5 = import1.J120Caixa();
    this._J120Det_0_6 = import1.J120Det(this._compView_0);
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_el_0, this._J120Det_0_6);
    }
    final _text_1 = import7.createText('a');
    final _text_2 = import7.createText('b');
    this._compView_0.createAndProject(this._J120Caixa_0_5, [
      const <Object>[],
      <Object>[_text_1]
        ..addAll(import5.unsafeCast(this.projectedNodes[0]))
        ..addAll(<Object>[_text_2])
    ]);
    final _anchor_3 = import7.appendAnchor(parentRenderNode);
    this._appEl_3 = ViewContainer(3, null, this, _anchor_3);
    var _TemplateRef_3_8 = TemplateRef(this._appEl_3, viewFactory_J120Usa1);
    this._NgIf_3_9 = NgIf(this._appEl_3, _TemplateRef_3_8);
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_anchor_3, this._NgIf_3_9);
    }
    final _text_4 = import7.appendText(parentRenderNode, '\n');
    final doc = import6.document;
    final _el_5 = import7.appendElement<import6.ButtonElement>(doc, parentRenderNode, 'button');
    final _text_6 = import7.appendText(_el_5, 'e');
    _el_5.addEventListener('focus', this.eventHandler1(this._handleEvent_0));
    _el_5.addEventListener('blur', this.eventHandler1(this._handleEvent_1));
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.recordInput(this._NgIf_3_9, 'ngIf', _ctx.visivel);
    }
    this._NgIf_3_9.ngIf = _ctx.visivel /* REF:asset:corpus_ngdart/lib/src/j120_projecao_concatenada.dart:919:935 */;
    this._appEl_3.detectChangesInNestedViews();
    this._compView_0.detectChanges();
  }

  @override
  void destroyInternal() {
    this._appEl_3.destroyNestedViews();
    this._compView_0.destroyInternalState();
  }

  void _handleEvent_0($event) {
    final _ctx = this.ctx;
    _ctx.visivel = false;
  }

  void _handleEvent_1($event) {
    final _ctx = this.ctx;
    _ctx.aberto = true;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J120Usa, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J120UsaNgFactory = ComponentFactory<import1.J120Usa>('j120-usa', viewFactory_J120UsaHost0);
ComponentFactory<import1.J120Usa> get J120UsaNgFactory {
  return _J120UsaNgFactory;
}

ComponentFactory<import1.J120Usa> createJ120UsaFactory() {
  return ComponentFactory('j120-usa', viewFactory_J120UsaHost0);
}

class _ViewJ120Usa1 extends import14.EmbeddedView<import1.J120Usa> {
  _ViewJ120Usa1(import15.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final _text_0 = import7.createText('c');
    final _text_1 = import7.createText('d');
    this.initRootNodesAndSubscriptions(
        import5.unsafeCast(<Object>[_text_0]
          ..addAll(import5.unsafeCast(this.projectedNodes[1]))
          ..addAll(<Object>[_text_1])),
        null);
  }
}

import14.EmbeddedView<void> viewFactory_J120Usa1(import15.RenderView parentView, int parentIndex) {
  return _ViewJ120Usa1(parentView, parentIndex);
}

final List<Object> styles$J120UsaHost = const [];

class _ViewJ120UsaHost0 extends import9.HostView<import1.J120Usa> {
  @override
  void build() {
    this.componentView = ViewJ120Usa0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J120Usa();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J120Usa> viewFactory_J120UsaHost0() {
  return _ViewJ120UsaHost0();
}

final List<Object> styles$J120Raiz = const [];

class ViewJ120Raiz0 extends import0.ComponentView<import1.J120Raiz> {
  static import2.ComponentStyles? _componentStyles;
  ViewJ120Raiz0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j120-raiz'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j120_projecao_concatenada.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final _text_0 = import7.appendText(parentRenderNode, 'antes');
    this.project(parentRenderNode, 0);
    final _text_1 = import7.appendText(parentRenderNode, 'depois');
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J120Raiz, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J120RaizNgFactory = ComponentFactory<import1.J120Raiz>('j120-raiz', viewFactory_J120RaizHost0);
ComponentFactory<import1.J120Raiz> get J120RaizNgFactory {
  return _J120RaizNgFactory;
}

ComponentFactory<import1.J120Raiz> createJ120RaizFactory() {
  return ComponentFactory('j120-raiz', viewFactory_J120RaizHost0);
}

final List<Object> styles$J120RaizHost = const [];

class _ViewJ120RaizHost0 extends import9.HostView<import1.J120Raiz> {
  @override
  void build() {
    this.componentView = ViewJ120Raiz0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J120Raiz();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J120Raiz> viewFactory_J120RaizHost0() {
  return _ViewJ120RaizHost0();
}

final List<Object> styles$J120Conteiner = const [];

class ViewJ120Conteiner0 extends import0.ComponentView<import1.J120Conteiner> {
  static import2.ComponentStyles? _componentStyles;
  ViewJ120Conteiner0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j120-conteiner'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j120_projecao_concatenada.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final _text_0 = import7.appendText(parentRenderNode, 'x');
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J120Conteiner, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J120ConteinerNgFactory = ComponentFactory<import1.J120Conteiner>('j120-conteiner', viewFactory_J120ConteinerHost0);
ComponentFactory<import1.J120Conteiner> get J120ConteinerNgFactory {
  return _J120ConteinerNgFactory;
}

ComponentFactory<import1.J120Conteiner> createJ120ConteinerFactory() {
  return ComponentFactory('j120-conteiner', viewFactory_J120ConteinerHost0);
}

final List<Object> styles$J120ConteinerHost = const [];

class _ViewJ120ConteinerHost0 extends import9.HostView<import1.J120Conteiner> {
  late dynamic _J120Servico_0_9 = import1.servico();
  late final ViewContainer _appEl_0;
  @override
  void build() {
    this.componentView = ViewJ120Conteiner0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this._appEl_0 = ViewContainer(0, null, this, _el_0);
    this.component = import1.J120Conteiner(this._appEl_0);
    this.initRootNode(this._appEl_0);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((identical(token, import1.J120Servico) && (0 == nodeIndex))) {
      return this._J120Servico_0_9;
    }
    return notFoundResult;
  }

  @override
  void detectChangesInternal() {
    this._appEl_0.detectChangesInNestedViews();
    this.componentView.detectChanges();
  }

  @override
  void destroyInternal() {
    this._appEl_0.destroyNestedViews();
  }
}

import9.HostView<import1.J120Conteiner> viewFactory_J120ConteinerHost0() {
  return _ViewJ120ConteinerHost0();
}

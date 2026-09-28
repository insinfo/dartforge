// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j121_leitura_de_container.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j121_leitura_de_container.dart' as import1;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import4;
import 'package:ngdart/src/core/linker/views/view.dart' as import5;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import6;
import 'package:ngdart/src/utilities.dart' as import7;
import 'dart:html' as import8;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import9;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import11;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import12;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import13;
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'package:ngdart/src/devtools.dart' as import15;

final List<Object> styles$J121Dinamico = const [];

class ViewJ121Dinamico0 extends import0.ComponentView<import1.J121Dinamico> {
  late final ViewContainer _appEl_0;
  late final TemplateRef _TemplateRef_0_8;
  static import4.ComponentStyles? _componentStyles;
  ViewJ121Dinamico0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkOnce) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('j121-dinamico'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/j121_leitura_de_container.dart' : null);
  }

  @override
  void build() {
    final _ctx = this.ctx;
    final parentRenderNode = this.initViewRoot();
    final _anchor_0 = import9.appendAnchor(parentRenderNode);
    this._appEl_0 = ViewContainer(0, null, this, _anchor_0);
    this._TemplateRef_0_8 = TemplateRef(this._appEl_0, viewFactory_J121Dinamico1);
    _ctx.vcr = this._appEl_0;
  }

  @override
  void detectChangesInCheckAlwaysViews() {
    this._appEl_0.detectChangesInCheckAlwaysViews();
  }

  @override
  void detectChangesInternal() {
    this._appEl_0.detectChangesInNestedViews();
  }

  @override
  void destroyInternal() {
    this._appEl_0.destroyNestedViews();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$J121Dinamico, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J121DinamicoNgFactory = ComponentFactory<import1.J121Dinamico>('j121-dinamico', viewFactory_J121DinamicoHost0);
ComponentFactory<import1.J121Dinamico> get J121DinamicoNgFactory {
  return _J121DinamicoNgFactory;
}

ComponentFactory<import1.J121Dinamico> createJ121DinamicoFactory() {
  return ComponentFactory('j121-dinamico', viewFactory_J121DinamicoHost0);
}

class _ViewJ121Dinamico1 extends import11.EmbeddedView<import1.J121Dinamico> {
  _ViewJ121Dinamico1(import12.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    this.initRootNodesAndSubscriptions(import7.unsafeCast(const <Object>[]), null);
  }
}

import11.EmbeddedView<void> viewFactory_J121Dinamico1(import12.RenderView parentView, int parentIndex) {
  return _ViewJ121Dinamico1(parentView, parentIndex);
}

final List<Object> styles$J121DinamicoHost = const [];

class _ViewJ121DinamicoHost0 extends import13.HostView<import1.J121Dinamico> {
  late final ViewContainer _appEl_0;
  @override
  void build() {
    this.componentView = ViewJ121Dinamico0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this._appEl_0 = ViewContainer(0, null, this, _el_0);
    this.component = import1.J121Dinamico(this._appEl_0);
    this.initRootNode(this._appEl_0);
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

import13.HostView<import1.J121Dinamico> viewFactory_J121DinamicoHost0() {
  return _ViewJ121DinamicoHost0();
}

final List<Object> styles$J121Ligado = const [];

class ViewJ121Ligado0 extends import0.ComponentView<import1.J121Ligado> {
  late final ViewJ121Dinamico0 _compView_0;
  late final ViewContainer _appEl_0;
  late final import1.J121Dinamico _J121Dinamico_0_8;
  late final ViewContainer _appEl_1;
  late final NgIf _NgIf_1_9;
  static import4.ComponentStyles? _componentStyles;
  ViewJ121Ligado0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkOnce) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('j121-ligado'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/j121_leitura_de_container.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = ViewJ121Dinamico0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this._appEl_0 = ViewContainer(0, null, this, _el_0);
    this._J121Dinamico_0_8 = import1.J121Dinamico(this._appEl_0);
    this._compView_0.create(this._J121Dinamico_0_8);
    final _anchor_1 = import9.appendAnchor(parentRenderNode);
    this._appEl_1 = ViewContainer(1, null, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, viewFactory_J121Ligado1);
    this._NgIf_1_9 = NgIf(this._appEl_1, _TemplateRef_1_8);
    if (import15.isDevToolsEnabled) {
      import15.Inspector.instance.registerDirective(_anchor_1, this._NgIf_1_9);
    }
  }

  @override
  void detectChangesInCheckAlwaysViews() {
    this._appEl_0.detectChangesInCheckAlwaysViews();
    this._appEl_1.detectChangesInCheckAlwaysViews();
    this._compView_0.detectChangesInCheckAlwaysViews();
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import15.isDevToolsEnabled) {
      import15.Inspector.instance.recordInput(this._NgIf_1_9, 'ngIf', _ctx.mostra);
    }
    this._NgIf_1_9.ngIf = _ctx.mostra /* REF:asset:corpus_ngdart/lib/src/j121_leitura_de_container.dart:1062:1076 */;
    this._appEl_0.detectChangesInNestedViews();
    this._appEl_1.detectChangesInNestedViews();
    this._compView_0.detectChanges();
  }

  @override
  void destroyInternal() {
    this._appEl_0.destroyNestedViews();
    this._appEl_1.destroyNestedViews();
    this._compView_0.destroyInternalState();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$J121Ligado, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J121LigadoNgFactory = ComponentFactory<import1.J121Ligado>('j121-ligado', viewFactory_J121LigadoHost0);
ComponentFactory<import1.J121Ligado> get J121LigadoNgFactory {
  return _J121LigadoNgFactory;
}

ComponentFactory<import1.J121Ligado> createJ121LigadoFactory() {
  return ComponentFactory('j121-ligado', viewFactory_J121LigadoHost0);
}

class _ViewJ121Ligado1 extends import11.EmbeddedView<import1.J121Ligado> {
  late final ViewJ121Dinamico0 _compView_1;
  late final ViewContainer _appEl_1;
  late final import1.J121Dinamico _J121Dinamico_1_8;
  late final ViewContainer _appEl_2;
  late final NgIf _NgIf_2_9;
  _ViewJ121Ligado1(import12.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    final _el_0 = import7.unsafeCast(doc.createElement('div'));
    this._compView_1 = ViewJ121Dinamico0(this, 1);
    final _el_1 = this._compView_1.rootElement;
    _el_0.append(_el_1);
    this._appEl_1 = ViewContainer(1, 0, this, _el_1);
    this._J121Dinamico_1_8 = import1.J121Dinamico(this._appEl_1);
    this._compView_1.create(this._J121Dinamico_1_8);
    final _anchor_2 = import9.appendAnchor(_el_0);
    this._appEl_2 = ViewContainer(2, 0, this, _anchor_2);
    var _TemplateRef_2_8 = TemplateRef(this._appEl_2, viewFactory_J121Ligado2);
    this._NgIf_2_9 = NgIf(this._appEl_2, _TemplateRef_2_8);
    if (import15.isDevToolsEnabled) {
      import15.Inspector.instance.registerDirective(_anchor_2, this._NgIf_2_9);
    }
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInCheckAlwaysViews() {
    this._appEl_1.detectChangesInCheckAlwaysViews();
    this._appEl_2.detectChangesInCheckAlwaysViews();
    this._compView_1.detectChangesInCheckAlwaysViews();
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import15.isDevToolsEnabled) {
      import15.Inspector.instance.recordInput(this._NgIf_2_9, 'ngIf', _ctx.mostra);
    }
    this._NgIf_2_9.ngIf = _ctx.mostra /* REF:asset:corpus_ngdart/lib/src/j121_leitura_de_container.dart:1118:1133 */;
    this._appEl_1.detectChangesInNestedViews();
    this._appEl_2.detectChangesInNestedViews();
    this._compView_1.detectChanges();
  }

  @override
  void destroyInternal() {
    this._appEl_1.destroyNestedViews();
    this._appEl_2.destroyNestedViews();
    this._compView_1.destroyInternalState();
  }
}

import11.EmbeddedView<void> viewFactory_J121Ligado1(import12.RenderView parentView, int parentIndex) {
  return _ViewJ121Ligado1(parentView, parentIndex);
}

class _ViewJ121Ligado2 extends import11.EmbeddedView<import1.J121Ligado> {
  _ViewJ121Ligado2(import12.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final _text_0 = import9.createText('x');
    this.initRootNode(_text_0);
  }
}

import11.EmbeddedView<void> viewFactory_J121Ligado2(import12.RenderView parentView, int parentIndex) {
  return _ViewJ121Ligado2(parentView, parentIndex);
}

final List<Object> styles$J121LigadoHost = const [];

class _ViewJ121LigadoHost0 extends import13.HostView<import1.J121Ligado> {
  @override
  void build() {
    this.componentView = ViewJ121Ligado0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J121Ligado();
    this.initRootNode(_el_0);
  }
}

import13.HostView<import1.J121Ligado> viewFactory_J121LigadoHost0() {
  return _ViewJ121LigadoHost0();
}

final List<Object> styles$J121Comum = const [];

class ViewJ121Comum0 extends import0.ComponentView<import1.J121Comum> {
  late final ViewContainer _appEl_2;
  late final TemplateRef _TemplateRef_2_8;
  static import4.ComponentStyles? _componentStyles;
  ViewJ121Comum0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('j121-comum'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/j121_leitura_de_container.dart' : null);
  }

  @override
  void build() {
    final _ctx = this.ctx;
    final parentRenderNode = this.initViewRoot();
    final doc = import8.document;
    final _el_0 = import9.appendElement<import8.HtmlElement>(doc, parentRenderNode, 'p');
    final _text_1 = import9.appendText(_el_0, 'a');
    final _anchor_2 = import9.appendAnchor(parentRenderNode);
    this._appEl_2 = ViewContainer(2, null, this, _anchor_2);
    this._TemplateRef_2_8 = TemplateRef(this._appEl_2, viewFactory_J121Comum1);
    _ctx.ponto = this._appEl_2;
  }

  @override
  void detectChangesInternal() {
    this._appEl_2.detectChangesInNestedViews();
  }

  @override
  void destroyInternal() {
    this._appEl_2.destroyNestedViews();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$J121Comum, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J121ComumNgFactory = ComponentFactory<import1.J121Comum>('j121-comum', viewFactory_J121ComumHost0);
ComponentFactory<import1.J121Comum> get J121ComumNgFactory {
  return _J121ComumNgFactory;
}

ComponentFactory<import1.J121Comum> createJ121ComumFactory() {
  return ComponentFactory('j121-comum', viewFactory_J121ComumHost0);
}

class _ViewJ121Comum1 extends import11.EmbeddedView<import1.J121Comum> {
  _ViewJ121Comum1(import12.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    this.initRootNodesAndSubscriptions(import7.unsafeCast(const <Object>[]), null);
  }
}

import11.EmbeddedView<void> viewFactory_J121Comum1(import12.RenderView parentView, int parentIndex) {
  return _ViewJ121Comum1(parentView, parentIndex);
}

final List<Object> styles$J121ComumHost = const [];

class _ViewJ121ComumHost0 extends import13.HostView<import1.J121Comum> {
  @override
  void build() {
    this.componentView = ViewJ121Comum0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J121Comum();
    this.initRootNode(_el_0);
  }
}

import13.HostView<import1.J121Comum> viewFactory_J121ComumHost0() {
  return _ViewJ121ComumHost0();
}

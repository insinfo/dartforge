// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j47_container_no_hospedeiro.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j47_container_no_hospedeiro.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/runtime/check_binding.dart' as import11;
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/devtools.dart' as import14;
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import15;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import16;

final List<Object> styles$J47Simples = const [];

class ViewJ47Simples0 extends import0.ComponentView<import1.J47Simples> {
  static import2.ComponentStyles? _componentStyles;
  ViewJ47Simples0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j47-simples'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j47_container_no_hospedeiro.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendElement<import6.HtmlElement>(doc, parentRenderNode, 'b');
    final _text_1 = import7.appendText(_el_0, 's');
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J47Simples, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J47SimplesNgFactory = ComponentFactory<import1.J47Simples>('j47-simples', viewFactory_J47SimplesHost0);
ComponentFactory<import1.J47Simples> get J47SimplesNgFactory {
  return _J47SimplesNgFactory;
}

ComponentFactory<import1.J47Simples> createJ47SimplesFactory() {
  return ComponentFactory('j47-simples', viewFactory_J47SimplesHost0);
}

final List<Object> styles$J47SimplesHost = const [];

class _ViewJ47SimplesHost0 extends import9.HostView<import1.J47Simples> {
  late final ViewContainer _appEl_0;
  @override
  void build() {
    this.componentView = ViewJ47Simples0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this._appEl_0 = ViewContainer(0, null, this, _el_0);
    this.component = import1.J47Simples(this._appEl_0);
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

import9.HostView<import1.J47Simples> viewFactory_J47SimplesHost0() {
  return _ViewJ47SimplesHost0();
}

final List<Object> styles$J47Completo = const [];

class ViewJ47Completo0 extends import0.ComponentView<import1.J47Completo> {
  Object? _expr_0;
  static import2.ComponentStyles? _componentStyles;
  ViewJ47Completo0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j47-completo'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j47_container_no_hospedeiro.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendElement<import6.HtmlElement>(doc, parentRenderNode, 'i');
    final _text_1 = import7.appendText(_el_0, 'c');
  }

  void detectHostChanges(bool firstCheck) {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.ativo;
    if (import11.checkBinding(this._expr_0, currVal_0, null, null)) {
      import7.updateClassBindingNonHtml(this.rootElement, 'ativo', currVal_0);
      this._expr_0 = currVal_0;
    }
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J47Completo, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J47CompletoNgFactory = ComponentFactory<import1.J47Completo>('j47-completo', viewFactory_J47CompletoHost0);
ComponentFactory<import1.J47Completo> get J47CompletoNgFactory {
  return _J47CompletoNgFactory;
}

ComponentFactory<import1.J47Completo> createJ47CompletoFactory() {
  return ComponentFactory('j47-completo', viewFactory_J47CompletoHost0);
}

final List<Object> styles$J47CompletoHost = const [];

class _ViewJ47CompletoHost0 extends import9.HostView<import1.J47Completo> {
  late final ViewContainer _appEl_0;
  @override
  void build() {
    this.componentView = ViewJ47Completo0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this._appEl_0 = ViewContainer(0, null, this, _el_0);
    this.component = import1.J47Completo(_el_0, this._appEl_0);
    this.initRootNode(this._appEl_0);
  }

  @override
  void detectChangesInternal() {
    bool firstCheck = this.firstCheck;
    if (((!import11.debugThrowIfChanged) && firstCheck)) {
      this.component.ngOnInit();
    }
    this._appEl_0.detectChangesInNestedViews();
    if ((!import11.debugThrowIfChanged)) {
      if (firstCheck) {
        this.component.ngAfterContentInit();
      }
    }
    this.componentView.detectHostChanges(firstCheck);
    this.componentView.detectChanges();
    if ((!import11.debugThrowIfChanged)) {
      if (firstCheck) {
        this.component.ngAfterViewInit();
      }
    }
  }

  @override
  void destroyInternal() {
    this._appEl_0.destroyNestedViews();
    this.component.ngOnDestroy();
  }
}

import9.HostView<import1.J47Completo> viewFactory_J47CompletoHost0() {
  return _ViewJ47CompletoHost0();
}

final List<Object> styles$J47Usa = const [];

class ViewJ47Usa0 extends import0.ComponentView<import1.J47Usa> {
  late final ViewJ47Simples0 _compView_0;
  late final ViewContainer _appEl_0;
  late final import1.J47Simples _J47Simples_0_8;
  late final ViewContainer _appEl_1;
  late final NgIf _NgIf_1_9;
  static import2.ComponentStyles? _componentStyles;
  ViewJ47Usa0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j47-usa'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j47_container_no_hospedeiro.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = ViewJ47Simples0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this._appEl_0 = ViewContainer(0, null, this, _el_0);
    this._J47Simples_0_8 = import1.J47Simples(this._appEl_0);
    this._compView_0.create(this._J47Simples_0_8);
    final _anchor_1 = import7.appendAnchor(parentRenderNode);
    this._appEl_1 = ViewContainer(1, null, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, viewFactory_J47Usa1);
    this._NgIf_1_9 = NgIf(this._appEl_1, _TemplateRef_1_8);
    if (import14.isDevToolsEnabled) {
      import14.Inspector.instance.registerDirective(_anchor_1, this._NgIf_1_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import14.isDevToolsEnabled) {
      import14.Inspector.instance.recordInput(this._NgIf_1_9, 'ngIf', _ctx.mostrar);
    }
    this._NgIf_1_9.ngIf = _ctx.mostrar /* REF:asset:corpus_ngdart/lib/src/j47_container_no_hospedeiro.dart:1089:1104 */;
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
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J47Usa, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J47UsaNgFactory = ComponentFactory<import1.J47Usa>('j47-usa', viewFactory_J47UsaHost0);
ComponentFactory<import1.J47Usa> get J47UsaNgFactory {
  return _J47UsaNgFactory;
}

ComponentFactory<import1.J47Usa> createJ47UsaFactory() {
  return ComponentFactory('j47-usa', viewFactory_J47UsaHost0);
}

class _ViewJ47Usa1 extends import15.EmbeddedView<import1.J47Usa> {
  late final ViewJ47Simples0 _compView_1;
  late final ViewContainer _appEl_1;
  late final import1.J47Simples _J47Simples_1_8;
  _ViewJ47Usa1(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import6.document;
    final _el_0 = import5.unsafeCast(doc.createElement('p'));
    this._compView_1 = ViewJ47Simples0(this, 1);
    final _el_1 = this._compView_1.rootElement;
    _el_0.append(_el_1);
    this._appEl_1 = ViewContainer(1, 0, this, _el_1);
    this._J47Simples_1_8 = import1.J47Simples(this._appEl_1);
    this._compView_1.create(this._J47Simples_1_8);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    this._appEl_1.detectChangesInNestedViews();
    this._compView_1.detectChanges();
  }

  @override
  void destroyInternal() {
    this._appEl_1.destroyNestedViews();
    this._compView_1.destroyInternalState();
  }
}

import15.EmbeddedView<void> viewFactory_J47Usa1(import16.RenderView parentView, int parentIndex) {
  return _ViewJ47Usa1(parentView, parentIndex);
}

final List<Object> styles$J47UsaHost = const [];

class _ViewJ47UsaHost0 extends import9.HostView<import1.J47Usa> {
  @override
  void build() {
    this.componentView = ViewJ47Usa0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J47Usa();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J47Usa> viewFactory_J47UsaHost0() {
  return _ViewJ47UsaHost0();
}

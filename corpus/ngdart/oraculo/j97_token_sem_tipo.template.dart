// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j97_token_sem_tipo.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j97_token_sem_tipo.dart' as import1;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import4;
import 'package:ngdart/src/core/linker/views/view.dart' as import5;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import6;
import 'package:ngdart/src/utilities.dart' as import7;
import 'dart:html' as import8;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import9;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/devtools.dart' as import11;
import 'package:ngdart/src/runtime/check_binding.dart' as import12;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import14;
import 'package:ngdart/src/runtime/text_binding.dart' as import15;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import16;
import 'package:ngdart/src/runtime/interpolate.dart' as import17;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import18;
import 'package:ngdart/src/di/errors.dart' as import19;
import 'package:ngdart/src/meta/di_tokens.dart' as import20;
import 'dart:core';

final List<Object> styles$J97TokenSemTipo = const [];

class ViewJ97TokenSemTipo0<T> extends import0.ComponentView<import1.J97TokenSemTipo<T>> {
  late final ViewContainer _appEl_1;
  late final NgIf _NgIf_1_9;
  Object? _expr_0;
  static import4.ComponentStyles? _componentStyles;
  ViewJ97TokenSemTipo0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('j97-token-sem-tipo'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/j97_token_sem_tipo.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import8.document;
    final _el_0 = import9.appendElement<import8.HtmlElement>(doc, parentRenderNode, 'i');
    final _anchor_1 = import9.appendAnchor(parentRenderNode);
    this._appEl_1 = ViewContainer(1, null, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, (parentView, parentIndex) {
      return viewFactory_J97TokenSemTipo1<T>(parentView, parentIndex);
    });
    this._NgIf_1_9 = NgIf(this._appEl_1, _TemplateRef_1_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_1, this._NgIf_1_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.recordInput(this._NgIf_1_9, 'ngIf', _ctx.compacto);
    }
    this._NgIf_1_9.ngIf = _ctx.compacto /* REF:asset:corpus_ngdart/lib/src/j97_token_sem_tipo.dart:404:420 */;
    this._appEl_1.detectChangesInNestedViews();
  }

  @override
  void destroyInternal() {
    this._appEl_1.destroyNestedViews();
  }

  void detectHostChanges(bool firstCheck) {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.compacto;
    if (import12.checkBinding(this._expr_0, currVal_0, null, null)) {
      import9.updateClassBindingNonHtml(this.rootElement, 'compacto', currVal_0);
      this._expr_0 = currVal_0;
    }
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$J97TokenSemTipo, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J97TokenSemTipoNgFactory = ComponentFactory<import1.J97TokenSemTipo>('j97-token-sem-tipo', viewFactory_J97TokenSemTipoHost0);
ComponentFactory<import1.J97TokenSemTipo> get J97TokenSemTipoNgFactory {
  return _J97TokenSemTipoNgFactory;
}

ComponentFactory<import1.J97TokenSemTipo<T>> createJ97TokenSemTipoFactory<T>() {
  return ComponentFactory('j97-token-sem-tipo', viewFactory_J97TokenSemTipoHost0);
}

class _ViewJ97TokenSemTipo1<T> extends import14.EmbeddedView<import1.J97TokenSemTipo<T>> {
  final import15.TextBinding _textBinding_1 = import15.TextBinding();
  _ViewJ97TokenSemTipo1(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    final _el_0 = import7.unsafeCast(doc.createElement('p'));
    _el_0.append(this._textBinding_1.element);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    this._textBinding_1.updateText(import17.interpolate0(_ctx.valor)) /* REF:asset:corpus_ngdart/lib/src/j97_token_sem_tipo.dart:421:432 */;
  }
}

import14.EmbeddedView<void> viewFactory_J97TokenSemTipo1<T>(import16.RenderView parentView, int parentIndex) {
  return _ViewJ97TokenSemTipo1<T>(parentView, parentIndex);
}

final List<Object> styles$J97TokenSemTipoHost = const [];

class _ViewJ97TokenSemTipoHost0<T> extends import18.HostView<import1.J97TokenSemTipo<T>> {
  @override
  void build() {
    this.componentView = ViewJ97TokenSemTipo0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = (import7.isDevMode
        ? import19.debugInjectorWrap(import1.J97TokenSemTipo, () {
            return import1.J97TokenSemTipo(this.injectorGetOptional(const import20.OpaqueToken<Object>('j97Relogio'), this.parentIndex), this.injectorGetOptional(const import20.OpaqueToken<String>('j97Tipado'), this.parentIndex), null);
          })
        : import1.J97TokenSemTipo(this.injectorGetOptional(const import20.OpaqueToken<Object>('j97Relogio'), this.parentIndex), this.injectorGetOptional(const import20.OpaqueToken<String>('j97Tipado'), this.parentIndex), null));
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    bool firstCheck = this.firstCheck;
    this.componentView.detectHostChanges(firstCheck);
    this.componentView.detectChanges();
  }
}

import18.HostView<import1.J97TokenSemTipo<T>> viewFactory_J97TokenSemTipoHost0<T>() {
  return _ViewJ97TokenSemTipoHost0();
}

final List<Object> styles$J97Limitado = const [];

class ViewJ97Limitado0<T extends num, U> extends import0.ComponentView<import1.J97Limitado<T, U>> {
  final import15.TextBinding _textBinding_1 = import15.TextBinding();
  static import4.ComponentStyles? _componentStyles;
  ViewJ97Limitado0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('j97-limitado'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/j97_token_sem_tipo.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import8.document;
    final _el_0 = import9.appendElement<import8.HtmlElement>(doc, parentRenderNode, 'b');
    _el_0.append(this._textBinding_1.element);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    this._textBinding_1.updateText(import17.interpolate0(_ctx.total)) /* REF:asset:corpus_ngdart/lib/src/j97_token_sem_tipo.dart:784:795 */;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$J97Limitado, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J97LimitadoNgFactory = ComponentFactory<import1.J97Limitado>('j97-limitado', viewFactory_J97LimitadoHost0);
ComponentFactory<import1.J97Limitado> get J97LimitadoNgFactory {
  return _J97LimitadoNgFactory;
}

ComponentFactory<import1.J97Limitado<T, U>> createJ97LimitadoFactory<T extends num, U>() {
  return ComponentFactory('j97-limitado', viewFactory_J97LimitadoHost0);
}

final List<Object> styles$J97LimitadoHost = const [];

class _ViewJ97LimitadoHost0<T extends num, U> extends import18.HostView<import1.J97Limitado<T, U>> {
  @override
  void build() {
    this.componentView = ViewJ97Limitado0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J97Limitado();
    this.initRootNode(_el_0);
  }
}

import18.HostView<import1.J97Limitado<T, U>> viewFactory_J97LimitadoHost0<T extends num, U>() {
  return _ViewJ97LimitadoHost0();
}

final List<Object> styles$J97Usa = const [];

class ViewJ97Usa0 extends import0.ComponentView<import1.J97Usa> {
  late final ViewJ97TokenSemTipo0 _compView_0;
  late final import1.J97TokenSemTipo _J97TokenSemTipo_0_5;
  late final ViewJ97Limitado0 _compView_1;
  late final import1.J97Limitado _J97Limitado_1_5;
  static import4.ComponentStyles? _componentStyles;
  ViewJ97Usa0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('j97-usa'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/j97_token_sem_tipo.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = ViewJ97TokenSemTipo0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    import9.setAttribute(_el_0, 'modo', 'x');
    this._J97TokenSemTipo_0_5 = (import7.isDevMode
        ? import19.debugInjectorWrap(import1.J97TokenSemTipo, () {
            return import1.J97TokenSemTipo((this.parentView!).injectorGetOptional(const import20.OpaqueToken<Object>('j97Relogio'), this.parentIndex), (this.parentView!).injectorGetOptional(const import20.OpaqueToken<String>('j97Tipado'), this.parentIndex), 'x');
          })
        : import1.J97TokenSemTipo((this.parentView!).injectorGetOptional(const import20.OpaqueToken<Object>('j97Relogio'), this.parentIndex), (this.parentView!).injectorGetOptional(const import20.OpaqueToken<String>('j97Tipado'), this.parentIndex), 'x'));
    this._compView_0.create(this._J97TokenSemTipo_0_5);
    this._compView_1 = ViewJ97Limitado0(this, 1);
    final _el_1 = this._compView_1.rootElement;
    parentRenderNode.append(_el_1);
    this._J97Limitado_1_5 = import1.J97Limitado();
    this._compView_1.create(this._J97Limitado_1_5);
  }

  @override
  void detectChangesInternal() {
    bool firstCheck = this.firstCheck;
    this._compView_0.detectHostChanges(firstCheck);
    this._compView_0.detectChanges();
    this._compView_1.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_0.destroyInternalState();
    this._compView_1.destroyInternalState();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$J97Usa, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J97UsaNgFactory = ComponentFactory<import1.J97Usa>('j97-usa', viewFactory_J97UsaHost0);
ComponentFactory<import1.J97Usa> get J97UsaNgFactory {
  return _J97UsaNgFactory;
}

ComponentFactory<import1.J97Usa> createJ97UsaFactory() {
  return ComponentFactory('j97-usa', viewFactory_J97UsaHost0);
}

final List<Object> styles$J97UsaHost = const [];

class _ViewJ97UsaHost0 extends import18.HostView<import1.J97Usa> {
  @override
  void build() {
    this.componentView = ViewJ97Usa0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J97Usa();
    this.initRootNode(_el_0);
  }
}

import18.HostView<import1.J97Usa> viewFactory_J97UsaHost0() {
  return _ViewJ97UsaHost0();
}

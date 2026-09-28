// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j68_filho_injeta_de_cima.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j68_filho_injeta_de_cima.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;
import 'package:ngdart/src/di/errors.dart' as import10;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'package:ngdart/src/devtools.dart' as import13;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import15;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import16;
import 'package:ngdart/src/core/change_detection/directive_change_detector.dart' as import17;
import 'package:ngdart/src/runtime/check_binding.dart' as import18;

final List<Object> styles$J68Campo = const [];

class ViewJ68Campo0 extends import0.ComponentView<import1.J68Campo> {
  static import2.ComponentStyles? _componentStyles;
  ViewJ68Campo0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j68-campo'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j68_filho_injeta_de_cima.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendElement<import6.HtmlElement>(doc, parentRenderNode, 'i');
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J68Campo, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J68CampoNgFactory = ComponentFactory<import1.J68Campo>('j68-campo', viewFactory_J68CampoHost0);
ComponentFactory<import1.J68Campo> get J68CampoNgFactory {
  return _J68CampoNgFactory;
}

ComponentFactory<import1.J68Campo> createJ68CampoFactory() {
  return ComponentFactory('j68-campo', viewFactory_J68CampoHost0);
}

final List<Object> styles$J68CampoHost = const [];

class _ViewJ68CampoHost0 extends import9.HostView<import1.J68Campo> {
  @override
  void build() {
    this.componentView = ViewJ68Campo0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = (import5.isDevMode
        ? import10.debugInjectorWrap(import1.J68Campo, () {
            return import1.J68Campo(this.injectorGet(import1.J68Form, this.parentIndex), this.injectorGetOptional(import1.J68Grupo, this.parentIndex));
          })
        : import1.J68Campo(this.injectorGet(import1.J68Form, this.parentIndex), this.injectorGetOptional(import1.J68Grupo, this.parentIndex)));
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J68Campo> viewFactory_J68CampoHost0() {
  return _ViewJ68CampoHost0();
}

final List<Object> styles$J68FilhoInjetaDeCima = const [];

class ViewJ68FilhoInjetaDeCima0 extends import0.ComponentView<import1.J68FilhoInjetaDeCima> {
  late final J68FormNgCd _J68Form_0_5;
  late final import1.J68Grupo _J68Grupo_1_5;
  late final ViewJ68Campo0 _compView_2;
  late final import1.J68Campo _J68Campo_2_5;
  late final ViewContainer _appEl_3;
  late final NgIf _NgIf_3_9;
  late final import6.FormElement _el_0;
  static import2.ComponentStyles? _componentStyles;
  ViewJ68FilhoInjetaDeCima0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j68-filho-injeta-de-cima'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j68_filho_injeta_de_cima.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    this._el_0 = import7.appendElement<import6.FormElement>(doc, parentRenderNode, 'form');
    import7.setAttribute(this._el_0, 'j68-form', '');
    this._J68Form_0_5 = J68FormNgCd(import1.J68Form());
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(this._el_0, this._J68Form_0_5.instance);
    }
    final _el_1 = import7.appendDiv(doc, this._el_0);
    import7.setAttribute(_el_1, 'j68-grupo', '');
    this._J68Grupo_1_5 = import1.J68Grupo();
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(_el_1, this._J68Grupo_1_5);
    }
    this._compView_2 = ViewJ68Campo0(this, 2);
    final _el_2 = this._compView_2.rootElement;
    _el_1.append(_el_2);
    this._J68Campo_2_5 = import1.J68Campo(this._J68Form_0_5.instance, this._J68Grupo_1_5);
    this._compView_2.create(this._J68Campo_2_5);
    final _anchor_3 = import7.appendAnchor(this._el_0);
    this._appEl_3 = ViewContainer(3, 0, this, _anchor_3);
    var _TemplateRef_3_8 = TemplateRef(this._appEl_3, viewFactory_J68FilhoInjetaDeCima1);
    this._NgIf_3_9 = NgIf(this._appEl_3, _TemplateRef_3_8);
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(_anchor_3, this._NgIf_3_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.recordInput(this._NgIf_3_9, 'ngIf', _ctx.mostrar);
    }
    this._NgIf_3_9.ngIf = _ctx.mostrar /* REF:package:corpus_ngdart/src/j68_filho_injeta_de_cima.html:76:91 */;
    this._appEl_3.detectChangesInNestedViews();
    this._J68Form_0_5.detectHostChanges(this, this._el_0);
    this._compView_2.detectChanges();
  }

  @override
  void destroyInternal() {
    this._appEl_3.destroyNestedViews();
    this._compView_2.destroyInternalState();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J68FilhoInjetaDeCima, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J68FilhoInjetaDeCimaNgFactory = ComponentFactory<import1.J68FilhoInjetaDeCima>('j68-filho-injeta-de-cima', viewFactory_J68FilhoInjetaDeCimaHost0);
ComponentFactory<import1.J68FilhoInjetaDeCima> get J68FilhoInjetaDeCimaNgFactory {
  return _J68FilhoInjetaDeCimaNgFactory;
}

ComponentFactory<import1.J68FilhoInjetaDeCima> createJ68FilhoInjetaDeCimaFactory() {
  return ComponentFactory('j68-filho-injeta-de-cima', viewFactory_J68FilhoInjetaDeCimaHost0);
}

class _ViewJ68FilhoInjetaDeCima1 extends import15.EmbeddedView<import1.J68FilhoInjetaDeCima> {
  late final ViewJ68Campo0 _compView_1;
  late final import1.J68Campo _J68Campo_1_5;
  _ViewJ68FilhoInjetaDeCima1(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import6.document;
    final _el_0 = import5.unsafeCast(doc.createElement('p'));
    this._compView_1 = ViewJ68Campo0(this, 1);
    final _el_1 = this._compView_1.rootElement;
    _el_0.append(_el_1);
    this._J68Campo_1_5 = (import5.isDevMode
        ? import10.debugInjectorWrap(import1.J68Campo, () {
            return import1.J68Campo(import5.unsafeCast<ViewJ68FilhoInjetaDeCima0>((this.parentView!))._J68Form_0_5.instance, ((this.parentView!).parentView!).injectorGetOptional(import1.J68Grupo, (this.parentView!).parentIndex));
          })
        : import1.J68Campo(import5.unsafeCast<ViewJ68FilhoInjetaDeCima0>((this.parentView!))._J68Form_0_5.instance, ((this.parentView!).parentView!).injectorGetOptional(import1.J68Grupo, (this.parentView!).parentIndex)));
    this._compView_1.create(this._J68Campo_1_5);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    this._compView_1.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_1.destroyInternalState();
  }
}

import15.EmbeddedView<void> viewFactory_J68FilhoInjetaDeCima1(import16.RenderView parentView, int parentIndex) {
  return _ViewJ68FilhoInjetaDeCima1(parentView, parentIndex);
}

final List<Object> styles$J68FilhoInjetaDeCimaHost = const [];

class _ViewJ68FilhoInjetaDeCimaHost0 extends import9.HostView<import1.J68FilhoInjetaDeCima> {
  @override
  void build() {
    this.componentView = ViewJ68FilhoInjetaDeCima0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J68FilhoInjetaDeCima();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J68FilhoInjetaDeCima> viewFactory_J68FilhoInjetaDeCimaHost0() {
  return _ViewJ68FilhoInjetaDeCimaHost0();
}

class J68FormNgCd extends import17.DirectiveChangeDetector {
  final import1.J68Form instance;
  Object? _expr_0;
  J68FormNgCd(this.instance);
  void detectHostChanges(import16.RenderView view, import6.Element el) {
    final currVal_0 = this.instance.enviado;
    if (import18.checkBinding(this._expr_0, currVal_0, null, null)) {
      import7.updateClassBindingNonHtml(el, 'enviado', currVal_0);
      this._expr_0 = currVal_0;
    }
  }
}

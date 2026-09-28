// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j116_pedido_do_conteudo.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j116_pedido_do_conteudo.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import8;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'package:ngdart/src/runtime/dom_helpers.dart' as import11;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/devtools.dart' as import13;
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import14;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import15;

final List<Object> styles$J116Popup = const [];

class ViewJ116Popup0 extends import0.ComponentView<import1.J116Popup> {
  static import2.ComponentStyles? _componentStyles;
  ViewJ116Popup0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j116-popup'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j116_pedido_do_conteudo.dart' : null);
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
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J116Popup, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J116PopupNgFactory = ComponentFactory<import1.J116Popup>('j116-popup', viewFactory_J116PopupHost0);
ComponentFactory<import1.J116Popup> get J116PopupNgFactory {
  return _J116PopupNgFactory;
}

ComponentFactory<import1.J116Popup> createJ116PopupFactory() {
  return ComponentFactory('j116-popup', viewFactory_J116PopupHost0);
}

final List<Object> styles$J116PopupHost = const [];

class _ViewJ116PopupHost0 extends import8.HostView<import1.J116Popup> {
  late dynamic _J116Hierarquia_0_6 = import1.hierarquia(this.component);
  late dynamic _J116Ref_0_7 = import1.ref(this.component);
  @override
  void build() {
    this.componentView = ViewJ116Popup0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J116Popup();
    this.initRootNode(_el_0);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((0 == nodeIndex)) {
      if (identical(token, import1.J116Hierarquia)) {
        return this._J116Hierarquia_0_6;
      }
      if (identical(token, import1.J116Ref)) {
        return this._J116Ref_0_7;
      }
    }
    return notFoundResult;
  }
}

import8.HostView<import1.J116Popup> viewFactory_J116PopupHost0() {
  return _ViewJ116PopupHost0();
}

final List<Object> styles$J116Usa = const [];

class ViewJ116Usa0 extends import0.ComponentView<import1.J116Usa> {
  late final ViewContainer _appEl_0;
  late final NgIf _NgIf_0_9;
  static import2.ComponentStyles? _componentStyles;
  ViewJ116Usa0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j116-usa'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j116_pedido_do_conteudo.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final _anchor_0 = import11.appendAnchor(parentRenderNode);
    this._appEl_0 = ViewContainer(0, null, this, _anchor_0);
    var _TemplateRef_0_8 = TemplateRef(this._appEl_0, viewFactory_J116Usa1);
    this._NgIf_0_9 = NgIf(this._appEl_0, _TemplateRef_0_8);
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(_anchor_0, this._NgIf_0_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.recordInput(this._NgIf_0_9, 'ngIf', _ctx.mostra);
    }
    this._NgIf_0_9.ngIf = _ctx.mostra /* REF:asset:corpus_ngdart/lib/src/j116_pedido_do_conteudo.dart:824:838 */;
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
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J116Usa, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J116UsaNgFactory = ComponentFactory<import1.J116Usa>('j116-usa', viewFactory_J116UsaHost0);
ComponentFactory<import1.J116Usa> get J116UsaNgFactory {
  return _J116UsaNgFactory;
}

ComponentFactory<import1.J116Usa> createJ116UsaFactory() {
  return ComponentFactory('j116-usa', viewFactory_J116UsaHost0);
}

class _ViewJ116Usa1 extends import14.EmbeddedView<import1.J116Usa> {
  late dynamic _J116Hierarquia_0_7 = import1.hierarquia(this._J116Popup_0_5);
  late final ViewJ116Popup0 _compView_0;
  late final import1.J116Popup _J116Popup_0_5;
  late final dynamic _J116Ref_0_6;
  late final import1.J116Foco _J116Foco_1_5;
  _ViewJ116Usa1(import15.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    this._compView_0 = ViewJ116Popup0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    this._J116Popup_0_5 = import1.J116Popup();
    this._J116Ref_0_6 = import1.ref(this._J116Popup_0_5);
    final doc = import6.document;
    final _el_1 = import5.unsafeCast(doc.createElement('div'));
    import11.setAttribute(_el_1, 'j116Foco', '');
    this._J116Foco_1_5 = import1.J116Foco(this._J116Ref_0_6);
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(_el_1, this._J116Foco_1_5);
    }
    final _text_2 = import11.appendText(_el_1, 'x');
    this._compView_0.createAndProject(this._J116Popup_0_5, [
      <Object>[_el_1]
    ]);
    this.initRootNode(_el_0);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((nodeIndex <= 2)) {
      if (identical(token, import1.J116Ref)) {
        return this._J116Ref_0_6;
      }
      if (identical(token, import1.J116Hierarquia)) {
        return this._J116Hierarquia_0_7;
      }
    }
    return notFoundResult;
  }

  @override
  void detectChangesInternal() {
    this._compView_0.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_0.destroyInternalState();
  }
}

import14.EmbeddedView<void> viewFactory_J116Usa1(import15.RenderView parentView, int parentIndex) {
  return _ViewJ116Usa1(parentView, parentIndex);
}

final List<Object> styles$J116UsaHost = const [];

class _ViewJ116UsaHost0 extends import8.HostView<import1.J116Usa> {
  @override
  void build() {
    this.componentView = ViewJ116Usa0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J116Usa();
    this.initRootNode(_el_0);
  }
}

import8.HostView<import1.J116Usa> viewFactory_J116UsaHost0() {
  return _ViewJ116UsaHost0();
}

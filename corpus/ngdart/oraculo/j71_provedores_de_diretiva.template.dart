// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j71_provedores_de_diretiva.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j71_provedores_de_diretiva.dart' as import1;
import 'package:ngdart/src/utilities.dart' as import2;
import 'package:ngdart/src/di/errors.dart' as import3;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import6;
import 'package:ngdart/src/core/linker/views/view.dart' as import7;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import8;
import 'dart:html' as import9;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import10;
import 'package:ngdart/src/devtools.dart' as import11;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import14;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import15;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import16;

final List<Object> styles$J71ProvedoresDeDiretiva = const [];

class ViewJ71ProvedoresDeDiretiva0 extends import0.ComponentView<import1.J71ProvedoresDeDiretiva> {
  late dynamic _J71Preguicoso_1_7 = (import2.isDevMode
      ? import3.debugInjectorWrap(import1.J71Preguicoso, () {
          return import1.J71Preguicoso((this.parentView!).injectorGet(import1.J71Externo, this.parentIndex));
        })
      : import1.J71Preguicoso((this.parentView!).injectorGet(import1.J71Externo, this.parentIndex)));
  late dynamic _J71Rotulo_1_8 = (this.parentView!).injectorGet(import1.J71Externo, this.parentIndex);
  late final import1.J71Grupo _J71Grupo_0_5;
  late final dynamic _J71Servico_1_5;
  late final import1.J71Espiao _J71Espiao_1_6;
  late final ViewContainer _appEl_2;
  late final NgIf _NgIf_2_9;
  static import6.ComponentStyles? _componentStyles;
  ViewJ71ProvedoresDeDiretiva0(import7.View parentView, int parentIndex) : super(parentView, parentIndex, import8.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import2.unsafeCast(import9.document.createElement('j71-provedores-de-diretiva'));
  }
  static String? get _debugComponentUrl {
    return (import2.isDevMode ? 'asset:corpus_ngdart/lib/src/j71_provedores_de_diretiva.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import9.document;
    final _el_0 = import10.appendElement<import9.HtmlElement>(doc, parentRenderNode, 'section');
    import10.setAttribute(_el_0, 'j71-grupo', '');
    this._J71Grupo_0_5 = import1.J71Grupo();
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_el_0, this._J71Grupo_0_5);
    }
    final _el_1 = import10.appendDiv(doc, _el_0);
    import10.setAttribute(_el_1, 'j71-espiao', '');
    this._J71Servico_1_5 = (import2.isDevMode
        ? import3.debugInjectorWrap(import1.J71Servico, () {
            return import1.J71Servico((this.parentView!).injectorGetOptional(import1.J71Config, this.parentIndex), this._J71Grupo_0_5);
          })
        : import1.J71Servico((this.parentView!).injectorGetOptional(import1.J71Config, this.parentIndex), this._J71Grupo_0_5));
    this._J71Espiao_1_6 = import1.J71Espiao(this._J71Servico_1_5);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_el_1, this._J71Espiao_1_6);
    }
    final _anchor_2 = import10.appendAnchor(_el_0);
    this._appEl_2 = ViewContainer(2, 0, this, _anchor_2);
    var _TemplateRef_2_8 = TemplateRef(this._appEl_2, viewFactory_J71ProvedoresDeDiretiva1);
    this._NgIf_2_9 = NgIf(this._appEl_2, _TemplateRef_2_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_2, this._NgIf_2_9);
    }
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((1 == nodeIndex)) {
      if (identical(token, import1.J71Servico)) {
        return this._J71Servico_1_5;
      }
      if (identical(token, import1.J71Preguicoso)) {
        return this._J71Preguicoso_1_7;
      }
      if (identical(token, import1.J71Rotulo)) {
        return this._J71Rotulo_1_8;
      }
    }
    return notFoundResult;
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.recordInput(this._NgIf_2_9, 'ngIf', _ctx.mostrar);
    }
    this._NgIf_2_9.ngIf = _ctx.mostrar /* REF:package:corpus_ngdart/src/j71_provedores_de_diretiva.html:50:65 */;
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
      _componentStyles = (styles = import6.ComponentStyles.unscoped(styles$J71ProvedoresDeDiretiva, _debugComponentUrl));
      if (import2.isDevMode) {
        import6.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J71ProvedoresDeDiretivaNgFactory = ComponentFactory<import1.J71ProvedoresDeDiretiva>('j71-provedores-de-diretiva', viewFactory_J71ProvedoresDeDiretivaHost0);
ComponentFactory<import1.J71ProvedoresDeDiretiva> get J71ProvedoresDeDiretivaNgFactory {
  return _J71ProvedoresDeDiretivaNgFactory;
}

ComponentFactory<import1.J71ProvedoresDeDiretiva> createJ71ProvedoresDeDiretivaFactory() {
  return ComponentFactory('j71-provedores-de-diretiva', viewFactory_J71ProvedoresDeDiretivaHost0);
}

class _ViewJ71ProvedoresDeDiretiva1 extends import14.EmbeddedView<import1.J71ProvedoresDeDiretiva> {
  late dynamic _J71Preguicoso_1_7 = (import2.isDevMode
      ? import3.debugInjectorWrap(import1.J71Preguicoso, () {
          return import1.J71Preguicoso(((this.parentView!).parentView!).injectorGet(import1.J71Externo, (this.parentView!).parentIndex));
        })
      : import1.J71Preguicoso(((this.parentView!).parentView!).injectorGet(import1.J71Externo, (this.parentView!).parentIndex)));
  late dynamic _J71Rotulo_1_8 = ((this.parentView!).parentView!).injectorGet(import1.J71Externo, (this.parentView!).parentIndex);
  late final dynamic _J71Servico_1_5;
  late final import1.J71Espiao _J71Espiao_1_6;
  _ViewJ71ProvedoresDeDiretiva1(import15.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import9.document;
    final _el_0 = import2.unsafeCast(doc.createElement('p'));
    final _el_1 = import10.appendSpan(doc, _el_0);
    import10.setAttribute(_el_1, 'j71-espiao', '');
    this._J71Servico_1_5 = (import2.isDevMode
        ? import3.debugInjectorWrap(import1.J71Servico, () {
            return import1.J71Servico(((this.parentView!).parentView!).injectorGetOptional(import1.J71Config, (this.parentView!).parentIndex), import2.unsafeCast<ViewJ71ProvedoresDeDiretiva0>((this.parentView!))._J71Grupo_0_5);
          })
        : import1.J71Servico(((this.parentView!).parentView!).injectorGetOptional(import1.J71Config, (this.parentView!).parentIndex), import2.unsafeCast<ViewJ71ProvedoresDeDiretiva0>((this.parentView!))._J71Grupo_0_5));
    this._J71Espiao_1_6 = import1.J71Espiao(this._J71Servico_1_5);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_el_1, this._J71Espiao_1_6);
    }
    this.initRootNode(_el_0);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((1 == nodeIndex)) {
      if (identical(token, import1.J71Servico)) {
        return this._J71Servico_1_5;
      }
      if (identical(token, import1.J71Preguicoso)) {
        return this._J71Preguicoso_1_7;
      }
      if (identical(token, import1.J71Rotulo)) {
        return this._J71Rotulo_1_8;
      }
    }
    return notFoundResult;
  }
}

import14.EmbeddedView<void> viewFactory_J71ProvedoresDeDiretiva1(import15.RenderView parentView, int parentIndex) {
  return _ViewJ71ProvedoresDeDiretiva1(parentView, parentIndex);
}

final List<Object> styles$J71ProvedoresDeDiretivaHost = const [];

class _ViewJ71ProvedoresDeDiretivaHost0 extends import16.HostView<import1.J71ProvedoresDeDiretiva> {
  late import1.J71Externo _J71Externo_0_6 = import1.J71Externo();
  late import1.J71Config _J71Config_0_7 = import1.J71Config();
  @override
  void build() {
    this.componentView = ViewJ71ProvedoresDeDiretiva0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J71ProvedoresDeDiretiva();
    this.initRootNode(_el_0);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((0 == nodeIndex)) {
      if (identical(token, import1.J71Externo)) {
        return this._J71Externo_0_6;
      }
      if (identical(token, import1.J71Config)) {
        return this._J71Config_0_7;
      }
    }
    return notFoundResult;
  }
}

import16.HostView<import1.J71ProvedoresDeDiretiva> viewFactory_J71ProvedoresDeDiretivaHost0() {
  return _ViewJ71ProvedoresDeDiretivaHost0();
}

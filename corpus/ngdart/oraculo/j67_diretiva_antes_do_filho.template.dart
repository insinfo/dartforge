// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j67_diretiva_antes_do_filho.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j67_diretiva_antes_do_filho.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import8;
import 'package:ngdart/src/runtime/check_binding.dart' as import9;
import 'dart:core';
import 'package:ngdart/src/runtime/dom_helpers.dart' as import11;
import 'package:ngdart/src/devtools.dart' as import12;
import 'package:ngdart/src/meta/di_tokens.dart' as import13;

final List<Object> styles$J67Campo = const [];

class ViewJ67Campo0 extends import0.ComponentView<import1.J67Campo> {
  static import2.ComponentStyles? _componentStyles;
  ViewJ67Campo0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j67-campo'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j67_diretiva_antes_do_filho.dart' : null);
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
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J67Campo, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J67CampoNgFactory = ComponentFactory<import1.J67Campo>('j67-campo', viewFactory_J67CampoHost0);
ComponentFactory<import1.J67Campo> get J67CampoNgFactory {
  return _J67CampoNgFactory;
}

ComponentFactory<import1.J67Campo> createJ67CampoFactory() {
  return ComponentFactory('j67-campo', viewFactory_J67CampoHost0);
}

final List<Object> styles$J67CampoHost = const [];

class _ViewJ67CampoHost0 extends import8.HostView<import1.J67Campo> {
  @override
  void build() {
    this.componentView = ViewJ67Campo0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J67Campo();
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    bool firstCheck = this.firstCheck;
    if ((!import9.debugThrowIfChanged)) {
      if (firstCheck) {
        this.component.ngAfterContentInit();
      }
    }
    this.componentView.detectChanges();
  }

  @override
  void destroyInternal() {
    this.component.ngOnDestroy();
  }
}

import8.HostView<import1.J67Campo> viewFactory_J67CampoHost0() {
  return _ViewJ67CampoHost0();
}

final List<Object> styles$J67DiretivaAntesDoFilho = const [];

class ViewJ67DiretivaAntesDoFilho0 extends import0.ComponentView<import1.J67DiretivaAntesDoFilho> {
  late List<Object> _j67Marcas_1_6 = [this._J67Marca_1_5];
  late List<Object> _j67Marcas_2_7 = [this._J67Marca_2_5];
  late final ViewJ67Campo0 _compView_0;
  late final import1.J67Marca _J67Marca_0_5;
  late final import1.J67Campo _J67Campo_0_6;
  late final List<Object> _j67Marcas_0_7;
  late final import1.J67Depois _J67Depois_0_8;
  late final import1.J67Marca _J67Marca_1_5;
  late final ViewJ67Campo0 _compView_2;
  late final import1.J67Marca _J67Marca_2_5;
  late final import1.J67Campo _J67Campo_2_6;
  Object? _expr_1;
  static import2.ComponentStyles? _componentStyles;
  ViewJ67DiretivaAntesDoFilho0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j67-diretiva-antes-do-filho'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j67_diretiva_antes_do_filho.dart' : null);
  }

  @override
  void build() {
    final _ctx = this.ctx;
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = ViewJ67Campo0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    import11.setAttribute(_el_0, 'j67-depois', 'd');
    import11.setAttribute(_el_0, 'j67-marca', 'm');
    this._J67Marca_0_5 = import1.J67Marca();
    this._J67Campo_0_6 = import1.J67Campo();
    this._j67Marcas_0_7 = [this._J67Marca_0_5];
    this._J67Depois_0_8 = import1.J67Depois(this._j67Marcas_0_7);
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_el_0, this._J67Marca_0_5);
      import12.Inspector.instance.registerDirective(_el_0, this._J67Depois_0_8);
    }
    final doc = import6.document;
    final _el_1 = import5.unsafeCast(doc.createElement('span'));
    import11.setAttribute(_el_1, 'j67-marca', 'dentro');
    this._J67Marca_1_5 = import1.J67Marca();
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_el_1, this._J67Marca_1_5);
    }
    this._compView_0.createAndProject(this._J67Campo_0_6, [
      <Object>[_el_1]
    ]);
    this._compView_2 = ViewJ67Campo0(this, 2);
    final _el_2 = this._compView_2.rootElement;
    parentRenderNode.append(_el_2);
    import11.setAttribute(_el_2, 'j67-marca', '');
    this._J67Marca_2_5 = import1.J67Marca();
    this._J67Campo_2_6 = import1.J67Campo();
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_el_2, this._J67Marca_2_5);
    }
    this._compView_2.createAndProject(this._J67Campo_2_6, [const <Object>[]]);
    _el_0.addEventListener('click', this.eventHandler0(_ctx.clicou));
    final subscription_0 = this._J67Marca_0_5.mudou.listen(this.eventHandler1(this._handleEvent_0));
    final subscription_1 = this._J67Campo_0_6.escolheu.listen(this.eventHandler1(this._handleEvent_1));
    this.initSubscriptions([subscription_0, subscription_1]);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((nodeIndex <= 1)) {
      if ((identical(token, const import13.MultiToken<Object>('j67Marcas')) && (1 == nodeIndex))) {
        return this._j67Marcas_1_6;
      }
      if (identical(token, const import13.MultiToken<Object>('j67Marcas'))) {
        return this._j67Marcas_0_7;
      }
    }
    if ((identical(token, const import13.MultiToken<Object>('j67Marcas')) && (2 == nodeIndex))) {
      return this._j67Marcas_2_7;
    }
    return notFoundResult;
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    bool firstCheck = this.firstCheck;
    if (firstCheck) {
      if (import12.isDevToolsEnabled) {
        import12.Inspector.instance.recordInput(this._J67Marca_0_5, 'j67-marca', 'm');
      }
      this._J67Marca_0_5.rotulo = 'm' /* REF:package:corpus_ngdart/src/j67_diretiva_antes_do_filho.html:11:24 */;
    }
    final currVal_1 = _ctx.nome;
    if (import9.checkBinding(this._expr_1, currVal_1, 'nome', 'package:corpus_ngdart/src/j67_diretiva_antes_do_filho.html')) {
      if (import12.isDevToolsEnabled) {
        import12.Inspector.instance.recordInput(this._J67Campo_0_6, 'rotulo', currVal_1);
      }
      this._J67Campo_0_6.rotulo = currVal_1 /* REF:package:corpus_ngdart/src/j67_diretiva_antes_do_filho.html:25:40 */;
      this._expr_1 = currVal_1;
    }
    if (firstCheck) {
      if (import12.isDevToolsEnabled) {
        import12.Inspector.instance.recordInput(this._J67Depois_0_8, 'j67-depois', 'd');
      }
      this._J67Depois_0_8.valor = 'd' /* REF:package:corpus_ngdart/src/j67_diretiva_antes_do_filho.html:52:66 */;
      if (import12.isDevToolsEnabled) {
        import12.Inspector.instance.recordInput(this._J67Marca_1_5, 'j67-marca', 'dentro');
      }
      this._J67Marca_1_5.rotulo = 'dentro' /* REF:package:corpus_ngdart/src/j67_diretiva_antes_do_filho.html:161:179 */;
      if (import12.isDevToolsEnabled) {
        import12.Inspector.instance.recordInput(this._J67Marca_2_5, 'j67-marca', '');
      }
      this._J67Marca_2_5.rotulo = '' /* REF:package:corpus_ngdart/src/j67_diretiva_antes_do_filho.html:212:221 */;
    }
    if ((!import9.debugThrowIfChanged)) {
      if (firstCheck) {
        this._J67Marca_1_5.ngAfterContentInit();
        this._J67Marca_0_5.ngAfterContentInit();
        this._J67Campo_0_6.ngAfterContentInit();
        this._J67Marca_2_5.ngAfterContentInit();
        this._J67Campo_2_6.ngAfterContentInit();
      }
    }
    this._compView_0.detectChanges();
    this._compView_2.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_0.destroyInternalState();
    this._compView_2.destroyInternalState();
    this._J67Marca_1_5.ngOnDestroy();
    this._J67Marca_0_5.ngOnDestroy();
    this._J67Campo_0_6.ngOnDestroy();
    this._J67Marca_2_5.ngOnDestroy();
    this._J67Campo_2_6.ngOnDestroy();
  }

  void _handleEvent_0($event) {
    final _ctx = this.ctx;
    _ctx.ultimo = $event;
  }

  void _handleEvent_1($event) {
    final _ctx = this.ctx;
    _ctx.numero = $event;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J67DiretivaAntesDoFilho, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J67DiretivaAntesDoFilhoNgFactory = ComponentFactory<import1.J67DiretivaAntesDoFilho>('j67-diretiva-antes-do-filho', viewFactory_J67DiretivaAntesDoFilhoHost0);
ComponentFactory<import1.J67DiretivaAntesDoFilho> get J67DiretivaAntesDoFilhoNgFactory {
  return _J67DiretivaAntesDoFilhoNgFactory;
}

ComponentFactory<import1.J67DiretivaAntesDoFilho> createJ67DiretivaAntesDoFilhoFactory() {
  return ComponentFactory('j67-diretiva-antes-do-filho', viewFactory_J67DiretivaAntesDoFilhoHost0);
}

final List<Object> styles$J67DiretivaAntesDoFilhoHost = const [];

class _ViewJ67DiretivaAntesDoFilhoHost0 extends import8.HostView<import1.J67DiretivaAntesDoFilho> {
  @override
  void build() {
    this.componentView = ViewJ67DiretivaAntesDoFilho0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J67DiretivaAntesDoFilho();
    this.initRootNode(_el_0);
  }
}

import8.HostView<import1.J67DiretivaAntesDoFilho> viewFactory_J67DiretivaAntesDoFilhoHost0() {
  return _ViewJ67DiretivaAntesDoFilhoHost0();
}

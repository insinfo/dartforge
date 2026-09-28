// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j73_atributo_vazio_no_filho.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j73_atributo_vazio_no_filho.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;
import 'package:ngdart/src/devtools.dart' as import10;

final List<Object> styles$J73Filho = const [];

class ViewJ73Filho0 extends import0.ComponentView<import1.J73Filho> {
  static import2.ComponentStyles? _componentStyles;
  ViewJ73Filho0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j73-filho'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j73_atributo_vazio_no_filho.dart' : null);
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
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J73Filho, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J73FilhoNgFactory = ComponentFactory<import1.J73Filho>('j73-filho', viewFactory_J73FilhoHost0);
ComponentFactory<import1.J73Filho> get J73FilhoNgFactory {
  return _J73FilhoNgFactory;
}

ComponentFactory<import1.J73Filho> createJ73FilhoFactory() {
  return ComponentFactory('j73-filho', viewFactory_J73FilhoHost0);
}

final List<Object> styles$J73FilhoHost = const [];

class _ViewJ73FilhoHost0 extends import9.HostView<import1.J73Filho> {
  @override
  void build() {
    this.componentView = ViewJ73Filho0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J73Filho();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J73Filho> viewFactory_J73FilhoHost0() {
  return _ViewJ73FilhoHost0();
}

final List<Object> styles$J73AtributoVazioNoFilho = const [];

class ViewJ73AtributoVazioNoFilho0 extends import0.ComponentView<import1.J73AtributoVazioNoFilho> {
  late final ViewJ73Filho0 _compView_0;
  late final import1.J73Filho _J73Filho_0_5;
  static import2.ComponentStyles? _componentStyles;
  ViewJ73AtributoVazioNoFilho0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j73-atributo-vazio-no-filho'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j73_atributo_vazio_no_filho.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = ViewJ73Filho0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    import7.setAttribute(_el_0, 'ativo', '');
    import7.setAttribute(_el_0, 'classe', '');
    import7.setAttribute(_el_0, 'outro', '');
    this._J73Filho_0_5 = import1.J73Filho();
    this._compView_0.create(this._J73Filho_0_5);
  }

  @override
  void detectChangesInternal() {
    bool firstCheck = this.firstCheck;
    if (firstCheck) {
      if (import10.isDevToolsEnabled) {
        import10.Inspector.instance.recordInput(this._J73Filho_0_5, 'classe', '');
      }
      this._J73Filho_0_5.classe = '' /* REF:package:corpus_ngdart/src/j73_atributo_vazio_no_filho.html:11:20 */;
      if (import10.isDevToolsEnabled) {
        import10.Inspector.instance.recordInput(this._J73Filho_0_5, 'ativo', true);
      }
      this._J73Filho_0_5.ativo = true /* REF:package:corpus_ngdart/src/j73_atributo_vazio_no_filho.html:21:26 */;
      if (import10.isDevToolsEnabled) {
        import10.Inspector.instance.recordInput(this._J73Filho_0_5, 'outro', '');
      }
      this._J73Filho_0_5.outro = '' /* REF:package:corpus_ngdart/src/j73_atributo_vazio_no_filho.html:27:35 */;
    }
    this._compView_0.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_0.destroyInternalState();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J73AtributoVazioNoFilho, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J73AtributoVazioNoFilhoNgFactory = ComponentFactory<import1.J73AtributoVazioNoFilho>('j73-atributo-vazio-no-filho', viewFactory_J73AtributoVazioNoFilhoHost0);
ComponentFactory<import1.J73AtributoVazioNoFilho> get J73AtributoVazioNoFilhoNgFactory {
  return _J73AtributoVazioNoFilhoNgFactory;
}

ComponentFactory<import1.J73AtributoVazioNoFilho> createJ73AtributoVazioNoFilhoFactory() {
  return ComponentFactory('j73-atributo-vazio-no-filho', viewFactory_J73AtributoVazioNoFilhoHost0);
}

final List<Object> styles$J73AtributoVazioNoFilhoHost = const [];

class _ViewJ73AtributoVazioNoFilhoHost0 extends import9.HostView<import1.J73AtributoVazioNoFilho> {
  @override
  void build() {
    this.componentView = ViewJ73AtributoVazioNoFilho0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J73AtributoVazioNoFilho();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J73AtributoVazioNoFilho> viewFactory_J73AtributoVazioNoFilhoHost0() {
  return _ViewJ73AtributoVazioNoFilhoHost0();
}

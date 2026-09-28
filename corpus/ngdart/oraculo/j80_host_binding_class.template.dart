// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j80_host_binding_class.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j80_host_binding_class.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/check_binding.dart' as import7;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import8;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import10;
import 'package:ngdart/src/devtools.dart' as import11;

final List<Object> styles$J80LinhaDoTempo = const [];

class ViewJ80LinhaDoTempo0 extends import0.ComponentView<import1.J80LinhaDoTempo> {
  Object? _expr_0;
  Object? _expr_1;
  Object? _expr_2;
  static import2.ComponentStyles? _componentStyles;
  ViewJ80LinhaDoTempo0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j80-linha-do-tempo'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j80_host_binding_class.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this.project(parentRenderNode, 0);
  }

  void detectHostChanges(bool firstCheck) {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.classeDoHospedeiro;
    if (import7.checkBinding(this._expr_0, currVal_0, null, null)) {
      this.updateChildClassNonHtml(this.rootElement, currVal_0);
      this._expr_0 = currVal_0;
    }
    final currVal_1 = _ctx.cor;
    if (import7.checkBinding(this._expr_1, currVal_1, null, null)) {
      this.rootElement.style.setProperty('--cor', currVal_1);
      this._expr_1 = currVal_1;
    }
    final currVal_2 = _ctx.papel;
    if (import7.checkBinding(this._expr_2, currVal_2, null, null)) {
      import8.updateAttribute(this.rootElement, 'role', currVal_2);
      this._expr_2 = currVal_2;
    }
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J80LinhaDoTempo, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J80LinhaDoTempoNgFactory = ComponentFactory<import1.J80LinhaDoTempo>('j80-linha-do-tempo', viewFactory_J80LinhaDoTempoHost0);
ComponentFactory<import1.J80LinhaDoTempo> get J80LinhaDoTempoNgFactory {
  return _J80LinhaDoTempoNgFactory;
}

ComponentFactory<import1.J80LinhaDoTempo> createJ80LinhaDoTempoFactory() {
  return ComponentFactory('j80-linha-do-tempo', viewFactory_J80LinhaDoTempoHost0);
}

final List<Object> styles$J80LinhaDoTempoHost = const [];

class _ViewJ80LinhaDoTempoHost0 extends import10.HostView<import1.J80LinhaDoTempo> {
  @override
  void build() {
    this.componentView = ViewJ80LinhaDoTempo0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J80LinhaDoTempo();
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    bool firstCheck = this.firstCheck;
    this.componentView.detectHostChanges(firstCheck);
    this.componentView.detectChanges();
  }
}

import10.HostView<import1.J80LinhaDoTempo> viewFactory_J80LinhaDoTempoHost0() {
  return _ViewJ80LinhaDoTempoHost0();
}

final List<Object> styles$J80HostBindingClass = const [];

class ViewJ80HostBindingClass0 extends import0.ComponentView<import1.J80HostBindingClass> {
  late final ViewJ80LinhaDoTempo0 _compView_0;
  late final import1.J80LinhaDoTempo _J80LinhaDoTempo_0_5;
  Object? _expr_0;
  static import2.ComponentStyles? _componentStyles;
  ViewJ80HostBindingClass0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j80-host-binding-class'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j80_host_binding_class.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = ViewJ80LinhaDoTempo0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this._J80LinhaDoTempo_0_5 = import1.J80LinhaDoTempo();
    final _text_1 = import8.createText('x');
    this._compView_0.createAndProject(this._J80LinhaDoTempo_0_5, [
      <Object>[_text_1]
    ]);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    bool firstCheck = this.firstCheck;
    final currVal_0 = _ctx.modo;
    if (import7.checkBinding(this._expr_0, currVal_0, 'modo', 'asset:corpus_ngdart/lib/src/j80_host_binding_class.dart')) {
      if (import11.isDevToolsEnabled) {
        import11.Inspector.instance.recordInput(this._J80LinhaDoTempo_0_5, 'modo', currVal_0);
      }
      this._J80LinhaDoTempo_0_5.modo = currVal_0 /* REF:asset:corpus_ngdart/lib/src/j80_host_binding_class.dart:572:585 */;
      this._expr_0 = currVal_0;
    }
    this._compView_0.detectHostChanges(firstCheck);
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
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J80HostBindingClass, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J80HostBindingClassNgFactory = ComponentFactory<import1.J80HostBindingClass>('j80-host-binding-class', viewFactory_J80HostBindingClassHost0);
ComponentFactory<import1.J80HostBindingClass> get J80HostBindingClassNgFactory {
  return _J80HostBindingClassNgFactory;
}

ComponentFactory<import1.J80HostBindingClass> createJ80HostBindingClassFactory() {
  return ComponentFactory('j80-host-binding-class', viewFactory_J80HostBindingClassHost0);
}

final List<Object> styles$J80HostBindingClassHost = const [];

class _ViewJ80HostBindingClassHost0 extends import10.HostView<import1.J80HostBindingClass> {
  @override
  void build() {
    this.componentView = ViewJ80HostBindingClass0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J80HostBindingClass();
    this.initRootNode(_el_0);
  }
}

import10.HostView<import1.J80HostBindingClass> viewFactory_J80HostBindingClassHost0() {
  return _ViewJ80HostBindingClassHost0();
}

// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'd05_usa_ciclo.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'd05_usa_ciclo.dart' as import1;
import 'd05_filho_ciclo.template.dart' as import2;
import 'd05_filho_ciclo.dart' as import3;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import4;
import 'package:ngdart/src/core/linker/views/view.dart' as import5;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import6;
import 'package:ngdart/src/utilities.dart' as import7;
import 'dart:html' as import8;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import9;
import 'package:ngdart/src/devtools.dart' as import10;
import 'package:ngdart/src/runtime/check_binding.dart' as import11;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import13;

final List<Object> styles$D05UsaCiclo = const [];

class ViewD05UsaCiclo0 extends import0.ComponentView<import1.D05UsaCiclo> {
  late final import2.ViewD05FilhoCiclo0 _compView_0;
  late final import3.D05FilhoCiclo _D05FilhoCiclo_0_5;
  Object? _expr_1;
  static import4.ComponentStyles? _componentStyles;
  ViewD05UsaCiclo0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('d05-usa-ciclo'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/d05_usa_ciclo.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = import2.ViewD05FilhoCiclo0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    import9.setAttribute(_el_0, 'titulo', 'fixo');
    this._D05FilhoCiclo_0_5 = import3.D05FilhoCiclo();
    this._compView_0.create(this._D05FilhoCiclo_0_5);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    bool changed = false;
    bool firstCheck = this.firstCheck;
    changed = false;
    if (firstCheck) {
      if (import10.isDevToolsEnabled) {
        import10.Inspector.instance.recordInput(this._D05FilhoCiclo_0_5, 'titulo', 'fixo');
      }
      this._D05FilhoCiclo_0_5.titulo = 'fixo' /* REF:package:corpus_ngdart/src/d05_usa_ciclo.html:17:30 */;
      changed = true;
    }
    final currVal_1 = _ctx.total;
    if (import11.checkBinding(this._expr_1, currVal_1, 'total', 'package:corpus_ngdart/src/d05_usa_ciclo.html')) {
      if (import10.isDevToolsEnabled) {
        import10.Inspector.instance.recordInput(this._D05FilhoCiclo_0_5, 'contador', currVal_1);
      }
      this._D05FilhoCiclo_0_5.contador = currVal_1 /* REF:package:corpus_ngdart/src/d05_usa_ciclo.html:31:49 */;
      changed = true;
      this._expr_1 = currVal_1;
    }
    if (changed) {
      this._D05FilhoCiclo_0_5.ngAfterChanges();
    }
    if (((!import11.debugThrowIfChanged) && firstCheck)) {
      this._D05FilhoCiclo_0_5.ngOnInit();
    }
    if ((!import11.debugThrowIfChanged)) {
      this._D05FilhoCiclo_0_5.ngDoCheck();
    }
    if ((!import11.debugThrowIfChanged)) {
      if (firstCheck) {
        this._D05FilhoCiclo_0_5.ngAfterContentInit();
      }
      this._D05FilhoCiclo_0_5.ngAfterContentChecked();
    }
    this._compView_0.detectChanges();
    if ((!import11.debugThrowIfChanged)) {
      if (firstCheck) {
        this._D05FilhoCiclo_0_5.ngAfterViewInit();
      }
      this._D05FilhoCiclo_0_5.ngAfterViewChecked();
    }
  }

  @override
  void destroyInternal() {
    this._compView_0.destroyInternalState();
    this._D05FilhoCiclo_0_5.ngOnDestroy();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$D05UsaCiclo, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _D05UsaCicloNgFactory = ComponentFactory<import1.D05UsaCiclo>('d05-usa-ciclo', viewFactory_D05UsaCicloHost0);
ComponentFactory<import1.D05UsaCiclo> get D05UsaCicloNgFactory {
  return _D05UsaCicloNgFactory;
}

ComponentFactory<import1.D05UsaCiclo> createD05UsaCicloFactory() {
  return ComponentFactory('d05-usa-ciclo', viewFactory_D05UsaCicloHost0);
}

final List<Object> styles$D05UsaCicloHost = const [];

class _ViewD05UsaCicloHost0 extends import13.HostView<import1.D05UsaCiclo> {
  @override
  void build() {
    this.componentView = ViewD05UsaCiclo0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.D05UsaCiclo();
    this.initRootNode(_el_0);
  }
}

import13.HostView<import1.D05UsaCiclo> viewFactory_D05UsaCicloHost0() {
  return _ViewD05UsaCicloHost0();
}

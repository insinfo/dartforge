// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i73_usa_consulta_read.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i73_usa_consulta_read.dart' as import1;
import 'i68_content_child_formas.template.dart' as import2;
import 'i68_content_child_formas.dart' as import3;
import 'i68_marca.dart' as import4;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import5;
import 'package:ngdart/src/core/linker/views/view.dart' as import6;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import7;
import 'package:ngdart/src/utilities.dart' as import8;
import 'dart:html' as import9;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import10;
import 'package:ngdart/src/devtools.dart' as import11;
import 'package:ngdart/src/runtime/check_binding.dart' as import12;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import14;

final List<Object> styles$I73UsaConsultaRead = const [];

class ViewI73UsaConsultaRead0 extends import0.ComponentView<import1.I73UsaConsultaRead> {
  late final import2.ViewI68ContentChildFormas0 _compView_0;
  late final import3.I68ContentChildFormas _I68ContentChildFormas_0_5;
  late final import4.I68Marca _I68Marca_1_5;
  static import5.ComponentStyles? _componentStyles;
  ViewI73UsaConsultaRead0(import6.View parentView, int parentIndex) : super(parentView, parentIndex, import7.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import8.unsafeCast(import9.document.createElement('i73-usa-consulta-read'));
  }
  static String? get _debugComponentUrl {
    return (import8.isDevMode ? 'asset:corpus_ngdart/lib/src/i73_usa_consulta_read.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = import2.ViewI68ContentChildFormas0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this._I68ContentChildFormas_0_5 = import3.I68ContentChildFormas();
    final doc = import9.document;
    final _el_1 = import8.unsafeCast(doc.createElement('div'));
    import10.setAttribute(_el_1, 'i68-marca', '');
    this._I68Marca_1_5 = import4.I68Marca();
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_el_1, this._I68Marca_1_5);
    }
    this._I68ContentChildFormas_0_5.marcas = [this._I68Marca_1_5];
    this._I68ContentChildFormas_0_5.elementos = [_el_1];
    this._I68ContentChildFormas_0_5.marca = this._I68Marca_1_5;
    this._I68ContentChildFormas_0_5.itens = [];
    this._compView_0.createAndProject(this._I68ContentChildFormas_0_5, [
      <Object>[_el_1]
    ]);
  }

  @override
  void detectChangesInternal() {
    bool firstCheck = this.firstCheck;
    if ((!import12.debugThrowIfChanged)) {
      if (firstCheck) {
        this._I68ContentChildFormas_0_5.ngAfterContentInit();
      }
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
      _componentStyles = (styles = import5.ComponentStyles.unscoped(styles$I73UsaConsultaRead, _debugComponentUrl));
      if (import8.isDevMode) {
        import5.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I73UsaConsultaReadNgFactory = ComponentFactory<import1.I73UsaConsultaRead>('i73-usa-consulta-read', viewFactory_I73UsaConsultaReadHost0);
ComponentFactory<import1.I73UsaConsultaRead> get I73UsaConsultaReadNgFactory {
  return _I73UsaConsultaReadNgFactory;
}

ComponentFactory<import1.I73UsaConsultaRead> createI73UsaConsultaReadFactory() {
  return ComponentFactory('i73-usa-consulta-read', viewFactory_I73UsaConsultaReadHost0);
}

final List<Object> styles$I73UsaConsultaReadHost = const [];

class _ViewI73UsaConsultaReadHost0 extends import14.HostView<import1.I73UsaConsultaRead> {
  @override
  void build() {
    this.componentView = ViewI73UsaConsultaRead0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I73UsaConsultaRead();
    this.initRootNode(_el_0);
  }
}

import14.HostView<import1.I73UsaConsultaRead> viewFactory_I73UsaConsultaReadHost0() {
  return _ViewI73UsaConsultaReadHost0();
}

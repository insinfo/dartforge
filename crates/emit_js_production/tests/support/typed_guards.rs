//! Exercise emitted descriptor guards against actual physical brands.

pub fn helper_expression(js: &str) -> &str {
    let start = js.find("const t$typedReadGuard").expect("captured guard");
    let helper = &js[start..];
    let helper = &helper[helper.find(" = ").unwrap() + 3..];
    &helper[..helper.find("\n})()").unwrap() + 5]
}

pub fn guard_checks(guard: &str, helper: &str) -> String {
    let mut names = Vec::new();
    let mut brands = Vec::new();
    let mut temporaries = Vec::new();
    let brand_names = [
        "Float32Array",
        "Float64Array",
        "Int8Array",
        "Uint8Array",
        "Uint8ClampedArray",
        "Int16Array",
        "Uint16Array",
        "Int32Array",
        "Uint32Array",
    ];
    let helper_name = guard
        .split(" = ")
        .nth(1)
        .unwrap()
        .split('(')
        .next()
        .unwrap();
    for part in guard.split(&format!(" = {helper_name}(")).skip(1) {
        let arguments = part.split(')').next().unwrap();
        let (name, brand) = arguments.split_once(',').unwrap();
        names.push(name.trim());
        brands.push(brand_names[brand.trim().parse::<usize>().unwrap()]);
    }
    for part in guard.split(" = ").take(names.len()) {
        temporaries.push(part.rsplit('(').next().unwrap().trim());
    }
    let values = brands
        .iter()
        .map(|brand| format!("new globalThis.{brand}(2)"))
        .collect::<Vec<_>>()
        .join(", ");
    let parameters = names
        .iter()
        .map(|name| dartforge_emit_js::js::string_literal(name))
        .collect::<Vec<_>>()
        .join(", ");
    let body = format!("let {}; return ({guard});", temporaries.join(", "));
    let mut checks = format!(
        "{{\nconst captured = {helper};\nconst invoke = new Function({parameters}, {}, {});\nconst check = (...values) => invoke(...values, captured);\nconst values = [{values}];\nif (!check(...values)) throw Error('fixed mixed brands refused');\n",
        dartforge_emit_js::js::string_literal(helper_name),
        dartforge_emit_js::js::string_literal(&body)
    );
    for (index, brand) in brands.iter().enumerate() {
        checks.push_str(&format!(r#"
{{
const Brand = globalThis.{brand};
const bad = [new DataView(new ArrayBuffer(16)), new (class extends Brand {{}})(2)];
if (typeof BigInt64Array !== 'undefined') bad.push(new BigInt64Array(2));
const ownLength = new Brand(2);
Object.defineProperty(ownLength, 'length', {{get() {{throw Error('own length read');}}}});
bad.push(ownLength);
const ownBuffer = new Brand(2);
Object.defineProperty(ownBuffer, 'buffer', {{get() {{throw Error('own buffer read');}}}});
bad.push(ownBuffer);
if (typeof SharedArrayBuffer !== 'undefined') bad.push(new Brand(new SharedArrayBuffer(16)));
if (Object.getOwnPropertyDescriptor(ArrayBuffer.prototype, 'resizable')) bad.push(new Brand(new ArrayBuffer(16, {{maxByteLength: 32}})));
for (const value of bad) {{
  const args = values.slice(); args[{index}] = value;
  if (check(...args)) throw Error('unsafe physical brand accepted');
}}
}}
"#));
    }
    if brands.len() == 2 {
        checks.push_str(&format!("const short = values.slice(); short[1] = new globalThis.{}(1); if (check(...short)) throw Error('short secondary accepted');\n", brands[1]));
    }
    checks.push_str("}\n");
    checks
}

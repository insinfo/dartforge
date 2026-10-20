//! Link-time inventories originate in verified fixed-image ARC certificates.
//! Registration is a consequence of code reachability, never its evidence.

use super::*;

#[cfg(test)]
mod tests;

pub(crate) fn enabled() -> bool {
    std::env::var("DARTFORGE_ARC_PORT_PRUNING").as_deref() != Ok("0")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Port {
    pub target: u32,
    pub kind: String,
    pub contract: i64,
    pub declaration: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Inventory {
    pub registration: u32,
    pub ports: Vec<Port>,
}

fn signature(declaration: &str) -> Option<(&str, &str, Vec<&str>)> {
    let body = declaration.strip_prefix("declare ")?;
    let (ret, rest) = body.split_once(" @")?;
    let (name, params) = rest.split_once('(')?;
    if name.is_empty() || name.as_bytes()[0].is_ascii_digit() || !name.bytes().all(super::name_char)
    {
        return None;
    }
    let params = params.strip_suffix(')')?;
    let valid = |ty| {
        matches!(
            ty,
            "i64" | "i8" | "i1" | "double" | "ptr" | "<4 x float>" | "<4 x i32>" | "<2 x double>"
        )
    };
    let params: Vec<_> = if params.is_empty() {
        vec![]
    } else {
        params.split(", ").collect()
    };
    (valid(ret) && params.iter().all(|ty| valid(*ty))).then_some((ret, name, params))
}

fn typed_signature(mut abi: i64, ret: &str, params: &[&str]) -> bool {
    if abi <= 1 || params.first() != Some(&"i64") {
        return false;
    }
    let mut types = Vec::new();
    while abi > 1 && types.len() < 13 {
        let ty = match abi % 5 {
            1 | 2 => "i64",
            3 => "double",
            4 => "i1",
            _ => return false,
        };
        types.push(ty);
        abi /= 5;
    }
    if abi != 1 || types.is_empty() {
        return false;
    }
    let result = types.remove(0);
    types.reverse();
    result == ret && params[1..] == types
}

pub(crate) fn validate(summary: &Summary) -> Result<(), String> {
    let mut registrations = HashSet::new();
    for inventory in &summary.arc_ports {
        let name = summary
            .names
            .get(inventory.registration as usize)
            .ok_or("ARC inventory registration outside the names table")?;
        if !name.starts_with("df.arc.ports.")
            || !name.bytes().all(super::name_char)
            || !registrations.insert(name)
        {
            return Err("invalid or duplicate ARC inventory registration".into());
        }
        let mut contracts = HashSet::new();
        for port in &inventory.ports {
            let name = summary
                .names
                .get(port.target as usize)
                .ok_or("ARC inventory target outside the names table")?;
            let (ret, target, params) =
                signature(&port.declaration).ok_or("invalid ARC port declaration")?;
            let valid = match port.kind.as_str() {
                "uniform" => port.contract == 0 && ret == "i64" && params == ["i64", "ptr", "ptr"],
                "word" => {
                    matches!(port.contract, 0..=5 | 7)
                        && ret == "i64"
                        && params.len() as i64 == port.contract
                        && params.iter().all(|ty| matches!(*ty, "i64" | "ptr"))
                }
                "typed" => typed_signature(port.contract, ret, &params),
                _ => false,
            };
            if name != target
                || !valid
                || !contracts.insert((port.target, &port.kind, port.contract))
            {
                return Err("invalid or duplicate ARC port contract".into());
            }
        }
    }
    Ok(())
}

pub(crate) fn read_ir(summary: &mut Summary, ir: &str) {
    for line in ir.lines() {
        if let Some(name) = line.strip_prefix("; df.arc-ports ") {
            let registration = summary.id(name);
            summary.arc_ports.push(Inventory {
                registration,
                ports: vec![],
            });
        }
        if let Some(data) = line.strip_prefix("; df.arc-port ") {
            let mut fields = data.splitn(4, '\t');
            let registration = summary.id(fields.next().expect("ARC inventory name"));
            let kind = fields.next().expect("ARC port kind").to_string();
            let contract = fields
                .next()
                .expect("ARC port contract")
                .parse()
                .expect("ARC port integer contract");
            let declaration = fields.next().expect("ARC port declaration").to_string();
            let target = summary.id(signature(&declaration).expect("ARC port signature").1);
            summary
                .arc_ports
                .iter_mut()
                .find(|i| i.registration == registration)
                .expect("ARC inventory before its records")
                .ports
                .push(Port {
                    target,
                    kind,
                    contract,
                    declaration,
                });
        }
    }
    validate(summary).expect("compiler emitted invalid deferred ARC ports");
}

pub(crate) fn assemble(
    output: &mut String,
    summaries: &[&Summary],
    reach: &Reach,
    prune: bool,
    declared: &mut HashSet<String>,
) -> Result<(), String> {
    let nodes: HashMap<_, _> = reach
        .names
        .iter()
        .enumerate()
        .map(|(i, n)| (n.as_str(), i as u32))
        .collect();
    let mut registrations = HashSet::new();
    let mut signatures = std::collections::BTreeMap::new();
    let mut word_contracts = HashMap::new();
    for summary in summaries {
        validate(summary)?;
        for inventory in &summary.arc_ports {
            let name = &summary.names[inventory.registration as usize];
            if !registrations.insert(name) {
                return Err(format!("duplicate ARC link registration: {name}"));
            }
            writeln!(output, "define void @{}() {{", name).unwrap();
            for (i, port) in inventory.ports.iter().enumerate() {
                let target = &summary.names[port.target as usize];
                if prune
                    && !nodes
                        .get(target.as_str())
                        .is_some_and(|&node| reach.live(node))
                {
                    continue;
                }
                if let Some(old) = signatures.insert(target, &port.declaration)
                    && old != &port.declaration
                {
                    return Err(format!("conflicting ARC port signatures: {target}"));
                }
                if port.kind != "typed" {
                    let arity = if port.kind == "uniform" {
                        3
                    } else {
                        port.contract
                    };
                    let uniform = port.kind == "uniform";
                    if let Some(old) = word_contracts.insert((target, arity), uniform)
                        && old != uniform
                    {
                        return Err(format!("conflicting ARC word/uniform contracts: {target}"));
                    }
                }
                writeln!(output, "  %port{i} = ptrtoint ptr @{} to i64", target).unwrap();
                let (runtime, contract) = match port.kind.as_str() {
                    "uniform" => ("dartforge_arc_register_uniform_owned_v1", String::new()),
                    "word" => (
                        "dartforge_arc_register_return_owned_v1",
                        format!(", i64 {}", port.contract),
                    ),
                    "typed" => (
                        "dartforge_arc_register_typed_v1",
                        format!(", i64 {}", port.contract),
                    ),
                    _ => unreachable!("validated ARC kind"),
                };
                writeln!(
                    output,
                    "  %registered{i} = call i64 @{runtime}(i64 %port{i}{contract})"
                )
                .unwrap();
            }
            output.push_str("  ret void\n}\n");
        }
    }
    for (target, declaration) in signatures {
        if declared.insert(target.clone()) {
            writeln!(output, "{declaration}").unwrap();
        }
    }
    Ok(())
}

//! Structural validation for the completion register.
//!
//! This deliberately stops at references and graph consistency.  It does not
//! authenticate evidence, inspect tools, or compute a release verdict.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path},
};

use serde::de::DeserializeOwned;

use super::schema::*;
use crate::kanban_backlog::KanbanBacklog;

fn load<T: DeserializeOwned>(root: &Path, rel: &str) -> Result<T, String> {
    let path = root.join(rel);
    let text = fs::read_to_string(&path).map_err(|e| format!("{rel}: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("{rel}: {e}"))
}
fn blank(s: &str) -> bool {
    s.trim().is_empty()
}
fn dup<T: Ord + Clone>(xs: &[T]) -> Vec<T> {
    let mut seen = BTreeSet::new();
    let mut out = BTreeSet::new();
    for x in xs {
        if !seen.insert(x.clone()) {
            out.insert(x.clone());
        }
    }
    out.into_iter().collect()
}
fn unique_ids<T>(ids: impl IntoIterator<Item = (String, T)>, kind: &str, errors: &mut Vec<String>) {
    let mut seen = BTreeSet::new();
    for (id, _) in ids {
        if id.trim().is_empty() {
            errors.push(format!("{kind} has blank id"));
        } else if !seen.insert(id.clone()) {
            errors.push(format!("duplicate {kind} id `{id}`"));
        }
    }
}
fn cycle(graph: &BTreeMap<String, Vec<String>>) -> Option<Vec<String>> {
    fn visit(
        n: &str,
        g: &BTreeMap<String, Vec<String>>,
        state: &mut BTreeMap<String, u8>,
        stack: &mut Vec<String>,
    ) -> Option<Vec<String>> {
        match state.get(n).copied().unwrap_or(0) {
            1 => {
                let p = stack.iter().position(|x| x == n).unwrap_or(0);
                return Some(stack[p..].to_vec());
            }
            2 => return None,
            _ => {}
        }
        state.insert(n.to_string(), 1);
        stack.push(n.to_string());
        let mut next = g.get(n).cloned().unwrap_or_default();
        next.sort();
        for d in next {
            if let Some(c) = visit(&d, g, state, stack) {
                return Some(c);
            }
        }
        stack.pop();
        state.insert(n.to_string(), 2);
        None
    }
    let mut state = BTreeMap::new();
    let mut keys: Vec<_> = graph.keys().cloned().collect();
    keys.sort();
    for k in keys {
        if let Some(c) = visit(&k, graph, &mut state, &mut Vec::new()) {
            return Some(c);
        }
    }
    None
}
fn check_ref_list(
    owner: &str,
    field: &str,
    xs: &[String],
    known: &BTreeSet<String>,
    errors: &mut Vec<String>,
) {
    for d in dup(xs) {
        errors.push(format!("{owner}.{field} duplicate reference `{d}`"));
    }
    for x in xs {
        if !known.contains(x) {
            errors.push(format!("{owner}.{field} unknown reference `{x}`"));
        }
    }
}
fn check_string_list(owner: &str, field: &str, xs: &[String], errors: &mut Vec<String>) {
    if xs.iter().any(|x| blank(x)) {
        errors.push(format!("{owner}.{field} contains blank reference"));
    }
    for d in dup(xs) {
        errors.push(format!("{owner}.{field} duplicate reference `{d}`"));
    }
}
fn safe_source(root: &Path, owner: &str, src: &SourceRef, errors: &mut Vec<String>) {
    if blank(&src.path) {
        errors.push(format!("{owner}.source_refs path is blank"));
        return;
    }
    if blank(&src.identity) {
        errors.push(format!("{owner}.source_refs identity is blank"));
    }
    let p = Path::new(&src.path);
    if p.is_absolute()
        || p.components()
            .any(|c| matches!(c, Component::ParentDir | Component::Prefix(_)))
    {
        errors.push(format!(
            "{owner}.source_refs path `{}` escapes root",
            src.path
        ));
        return;
    }
    let root = match root.canonicalize() {
        Ok(v) => v,
        Err(e) => {
            errors.push(format!("root: {e}"));
            return;
        }
    };
    match root.join(p).canonicalize() {
        Ok(real) if real.starts_with(&root) && real.is_file() => {}
        Ok(real) if real.starts_with(&root) => errors.push(format!(
            "{owner}.source_refs path `{}` is not a file",
            src.path
        )),
        Ok(_) => errors.push(format!(
            "{owner}.source_refs path `{}` escapes root",
            src.path
        )),
        Err(e) => errors.push(format!(
            "{owner}.source_refs path `{}` missing: {e}",
            src.path
        )),
    }
}

/// Validate register references, invariants, and dependency graphs.
pub fn validate_register_structure(root: &Path) -> Result<Vec<String>, String> {
    let reqs: RequirementsDocument = load(root, "plans/completion/requirements.json")?;
    let matrix: MatrixDocument = load(root, "plans/completion/matrix.json")?;
    let scenarios: ScenariosDocument = load(root, "plans/completion/scenarios.json")?;
    let deps: DependenciesDocument = load(root, "plans/completion/dependencies.json")?;
    let defects: DefectsDocument = load(root, "plans/completion/defects.json")?;
    let backlog = KanbanBacklog::from_file(&root.join("plans/kanban/legion-ga-backlog.toml"))
        .map_err(|e| format!("plans/kanban/legion-ga-backlog.toml: {e}"))?;
    let mut e = Vec::new();
    if reqs.requirements.is_empty() {
        e.push("requirements is empty".into());
    }
    if matrix.configurations.is_empty() {
        e.push("configurations is empty".into());
    }
    if scenarios.scenarios.is_empty() {
        e.push("scenarios is empty".into());
    }
    if deps.packages.is_empty() {
        e.push("packages is empty".into());
    }
    unique_ids(
        reqs.requirements.iter().map(|x| (x.id.clone(), x)),
        "requirement",
        &mut e,
    );
    unique_ids(
        matrix.configurations.iter().map(|x| (x.id.clone(), x)),
        "configuration",
        &mut e,
    );
    unique_ids(
        scenarios.scenarios.iter().map(|x| (x.id.clone(), x)),
        "scenario",
        &mut e,
    );
    unique_ids(
        deps.packages.iter().map(|x| (x.package_id.clone(), x)),
        "package",
        &mut e,
    );
    unique_ids(
        defects.defects.iter().map(|x| (x.id.clone(), x)),
        "defect",
        &mut e,
    );
    let rids: BTreeSet<_> = reqs.requirements.iter().map(|x| x.id.clone()).collect();
    let cids: BTreeSet<_> = matrix.configurations.iter().map(|x| x.id.clone()).collect();
    let sids: BTreeSet<_> = scenarios.scenarios.iter().map(|x| x.id.clone()).collect();
    let pids: BTreeSet<_> = deps.packages.iter().map(|x| x.package_id.clone()).collect();
    let dids: BTreeSet<_> = defects.defects.iter().map(|x| x.id.clone()).collect();
    let mut legacy = BTreeSet::new();
    for r in &reqs.requirements {
        if blank(&r.title) {
            e.push(format!("{}.title is blank", r.id));
        }
        if blank(&r.owner_role) {
            e.push(format!("{}.owner_role is blank", r.id));
        }
        if blank(&r.package_id) {
            e.push(format!("{}.package_id is blank", r.id));
        }
        if r.source_refs.is_empty() {
            e.push(format!("{}.source_refs is empty", r.id));
        }
        for s in &r.source_refs {
            safe_source(root, &r.id, s, &mut e);
        }
        for x in dup(&r
            .source_refs
            .iter()
            .map(|s| format!("{}::{}", s.path, s.identity))
            .collect::<Vec<_>>())
        {
            e.push(format!("{}.source_refs duplicate `{x}`", r.id));
        }
        for x in &r.legacy_ids {
            if blank(x) {
                e.push(format!("{}.legacy_ids contains blank reference", r.id));
            }
            if !legacy.insert(x.clone()) { /* shared legacy ids are permitted */ }
        }
        check_string_list(&r.id, "legacy_ids", &r.legacy_ids, &mut e);
        check_ref_list(&r.id, "depends_on", &r.depends_on, &rids, &mut e);
        check_ref_list(&r.id, "scenario_ids", &r.scenario_ids, &sids, &mut e);
        check_ref_list(
            &r.id,
            "configuration_ids",
            &r.configuration_ids,
            &cids,
            &mut e,
        );
        check_ref_list(&r.id, "defect_ids", &r.defect_ids, &dids, &mut e);
        if !pids.contains(&r.package_id) {
            e.push(format!("{}.package_id unknown `{}`", r.id, r.package_id));
        }
        if r.kind == RequirementKind::Product
            && (matches!(r.stage, Stage::S6) || r.package_id.starts_with("S6"))
        {
            e.push(format!(
                "{} product requirement cannot be owned by S6",
                r.id
            ));
        }
        if r.kind == RequirementKind::Product && !r.protected_product_ids.is_empty() {
            e.push(format!(
                "{}.protected_product_ids must be empty for product",
                r.id
            ));
        }
        if r.kind == RequirementKind::Internal && r.protected_product_ids.is_empty() {
            e.push(format!("{}.protected_product_ids is empty", r.id));
        }
        for x in &r.protected_product_ids {
            if !rids.contains(x) {
                e.push(format!("{}.protected_product_ids unknown `{x}`", r.id));
            } else if !reqs
                .requirements
                .iter()
                .any(|target| target.id == *x && target.kind == RequirementKind::Product)
            {
                e.push(format!(
                    "{}.protected_product_ids target `{x}` is not a product requirement",
                    r.id
                ));
            }
            if x == &r.id {
                e.push(format!("{}.protected_product_ids self-reference", r.id));
            }
        }
        check_string_list(
            &r.id,
            "protected_product_ids",
            &r.protected_product_ids,
            &mut e,
        );
        if r.required && (r.scenario_ids.is_empty() || r.configuration_ids.is_empty()) {
            e.push(format!(
                "{} required row has empty scenario/configuration coverage",
                r.id
            ));
        }
        if matches!(r.acceptance, Acceptance::Accepted)
            && !matches!(r.implementation, Implementation::Implemented)
        {
            e.push(format!(
                "{} accepted while implementation is not implemented",
                r.id
            ));
        }
    }
    let mut rg = BTreeMap::new();
    for r in &reqs.requirements {
        rg.insert(r.id.clone(), r.depends_on.clone());
        if r.depends_on.iter().any(|x| x == &r.id) {
            e.push(format!("{}.depends_on self-reference", r.id));
        }
    }
    if let Some(c) = cycle(&rg) {
        e.push(format!("requirement dependency cycle: {}", c.join(" -> ")));
    }
    for task in backlog
        .epics
        .iter()
        .flat_map(|x| x.features.iter())
        .flat_map(|x| x.tasks.iter())
    {
        if !reqs
            .requirements
            .iter()
            .any(|r| r.legacy_ids.iter().any(|id| id == &task.id))
        {
            e.push(format!("kanban task `{}` has unknown legacyID", task.id));
        }
    }
    let task_ids: BTreeSet<String> = backlog
        .epics
        .iter()
        .flat_map(|x| x.features.iter())
        .flat_map(|x| x.tasks.iter())
        .map(|x| x.id.clone())
        .collect();
    for r in &reqs.requirements {
        for legacy_id in &r.legacy_ids {
            if !task_ids.contains(legacy_id) {
                e.push(format!(
                    "{}.legacy_ids unknown Kanban task `{legacy_id}`",
                    r.id
                ));
            }
        }
    }
    for p in &deps.packages {
        if blank(&p.owner_role) {
            e.push(format!("{}.owner_role is blank", p.package_id));
        }
        check_string_list(
            &p.package_id,
            "external_prerequisites",
            &p.external_prerequisites,
            &mut e,
        );
        check_ref_list(
            &p.package_id,
            "requirement_ids",
            &p.requirement_ids,
            &rids,
            &mut e,
        );
        let owned: BTreeSet<_> = p.requirement_ids.iter().collect();
        for r in &reqs.requirements {
            if r.package_id == p.package_id && !owned.contains(&r.id) {
                e.push(format!(
                    "{} does not list owned requirement {}",
                    p.package_id, r.id
                ));
            }
        }
        if !pids.contains(&p.package_id) {
            continue;
        }
        if !p
            .milestones
            .iter()
            .any(|m| m.id == format!("{}:implemented", p.package_id))
        {
            e.push(format!("{} missing implemented milestone", p.package_id));
        }
        if !p
            .milestones
            .iter()
            .any(|m| m.id == format!("{}:accepted", p.package_id))
        {
            e.push(format!("{} missing accepted milestone", p.package_id));
        }
        for m in &p.milestones {
            if blank(&m.id) {
                e.push(format!("{} has milestone with blank id", p.package_id));
            }
            if !m.id.starts_with(&(p.package_id.clone() + ":")) {
                e.push(format!(
                    "{} milestone `{}` has wrong package prefix",
                    p.package_id, m.id
                ));
            }
            if m.deliverable_refs.is_empty() || m.deliverable_refs.iter().any(|x| blank(x)) {
                e.push(format!(
                    "{} milestone {} has blank deliverable_refs",
                    p.package_id, m.id
                ));
            }
            check_string_list(
                &format!("{} milestone {}", p.package_id, m.id),
                "deliverable_refs",
                &m.deliverable_refs,
                &mut e,
            );
        }
    }
    let mut mg = BTreeMap::new();
    for p in &deps.packages {
        for m in &p.milestones {
            if mg.insert(m.id.clone(), m.depends_on.clone()).is_some() {
                e.push(format!("duplicate milestone id `{}`", m.id));
            }
            for d in dup(&m.depends_on) {
                e.push(format!("milestone {} duplicate dependency `{d}`", m.id));
            }
            if m.depends_on.iter().any(|x| x == &m.id) {
                e.push(format!("milestone {} self-reference", m.id));
            }
        }
    }
    let mids: BTreeSet<_> = mg.keys().cloned().collect();
    for (m, ds) in &mg {
        for d in ds {
            if !mids.contains(d) {
                e.push(format!("milestone {} unknown dependency `{d}`", m));
            }
        }
    }
    if let Some(c) = cycle(&mg) {
        e.push(format!("milestone dependency cycle: {}", c.join(" -> ")));
    }
    for s in &scenarios.scenarios {
        if s.requirement_ids.is_empty() {
            e.push(format!("{}.requirement_ids is empty", s.id));
        }
        if s.configuration_ids.is_empty() {
            e.push(format!("{}.configuration_ids is empty", s.id));
        }
        check_ref_list(&s.id, "requirement_ids", &s.requirement_ids, &rids, &mut e);
        check_ref_list(
            &s.id,
            "configuration_ids",
            &s.configuration_ids,
            &cids,
            &mut e,
        );
        if s.steps.is_empty() || s.steps.iter().any(|x| blank(x)) {
            e.push(format!("{}.steps is empty or blank", s.id));
        }
        if s.external_oracles.is_empty() {
            e.push(format!("{}.external_oracles is empty", s.id));
        }
        if s.recovery_cases.is_empty() {
            e.push(format!("{}.recovery_cases is empty", s.id));
        }
        if blank(&s.sensitive_artifact_policy) {
            e.push(format!("{}.sensitive_artifact_policy is blank", s.id));
        }
        let mut checks = BTreeSet::new();
        for c in s.external_oracles.iter().chain(s.recovery_cases.iter()) {
            if blank(&c.id) || blank(&c.description) {
                e.push(format!("{}.check {} has blank id/description", s.id, c.id));
            }
            if !checks.insert(c.id.clone()) {
                e.push(format!("{}.check duplicate id `{}`", s.id, c.id));
            }
        }
        for r in &s.requirement_ids {
            if let Some(row) = reqs.requirements.iter().find(|x| &x.id == r) {
                for c in &s.configuration_ids {
                    if !row.configuration_ids.contains(c) {
                        e.push(format!(
                            "{} scenario {} configuration {} not linked by requirement",
                            r, s.id, c
                        ));
                    }
                }
            }
        }
    }
    for r in &reqs.requirements {
        for s in &r.scenario_ids {
            if !scenarios
                .scenarios
                .iter()
                .any(|x| &x.id == s && x.requirement_ids.contains(&r.id))
            {
                e.push(format!("{} scenario link {} is not bidirectional", r.id, s));
            }
        }
        let covered: BTreeSet<_> = scenarios
            .scenarios
            .iter()
            .filter(|s| s.requirement_ids.contains(&r.id))
            .flat_map(|s| s.configuration_ids.iter().cloned())
            .collect();
        for c in &r.configuration_ids {
            if !covered.contains(c) {
                e.push(format!(
                    "{} configuration {} lacks scenario coverage",
                    r.id, c
                ));
            }
        }
        for d in &r.defect_ids {
            if !defects
                .defects
                .iter()
                .any(|x| &x.id == d && x.requirement_ids.contains(&r.id))
            {
                e.push(format!("{} defect link {} is not bidirectional", r.id, d));
            }
        }
    }
    for s in &scenarios.scenarios {
        for r in &s.requirement_ids {
            if !reqs
                .requirements
                .iter()
                .any(|x| &x.id == r && x.scenario_ids.contains(&s.id))
            {
                e.push(format!(
                    "{} requirement link {} is not bidirectional",
                    s.id, r
                ));
            }
        }
    }
    for c in &matrix.configurations {
        if blank(&c.architecture)
            || blank(&c.hardware)
            || blank(&c.project_category)
            || blank(&c.owner_approval_ref)
        {
            e.push(format!(
                "{} has blank architecture/hardware/project_category/owner_approval_ref",
                c.id
            ));
        }
        if c.tool_versions.is_empty() || c.tool_versions.iter().any(|(n, v)| blank(n) || blank(v)) {
            e.push(format!("{}.tool_versions is empty or blank", c.id));
        }
    }
    for d in &defects.defects {
        if d.requirement_ids.is_empty() {
            e.push(format!("{}.requirement_ids is empty", d.id));
        }
        check_ref_list(&d.id, "requirement_ids", &d.requirement_ids, &rids, &mut e);
        check_string_list(
            &d.id,
            "verification_run_ids",
            &d.verification_run_ids,
            &mut e,
        );
        if !sids.contains(&d.scenario_id) {
            e.push(format!("{}.scenario_id unknown `{}`", d.id, d.scenario_id));
        }
        if !cids.contains(&d.configuration_id) {
            e.push(format!(
                "{}.configuration_id unknown `{}`",
                d.id, d.configuration_id
            ));
        }
        if !pids.contains(&d.repair_package_id) {
            e.push(format!(
                "{}.repair_package_id unknown `{}`",
                d.id, d.repair_package_id
            ));
        }
        if d.requirement_ids.iter().any(|r| {
            !reqs
                .requirements
                .iter()
                .any(|x| &x.id == r && x.defect_ids.contains(&d.id))
        }) {
            e.push(format!("{} requirement link is not bidirectional", d.id));
        }
        if d.reproduction.is_empty()
            || d.reproduction.iter().any(|x| blank(x))
            || blank(&d.expected)
            || blank(&d.observed)
            || blank(&d.owner)
        {
            e.push(format!(
                "{} reproduction/expected/observed/owner is blank",
                d.id
            ));
        }
        if matches!(d.status, DefectStatus::Closed) && d.verification_run_ids.is_empty() {
            e.push(format!(
                "{} closed defect has no verification_run_ids",
                d.id
            ));
        }
        if !d.requirement_ids.iter().all(|r| {
            scenarios.scenarios.iter().any(|s| {
                s.id == d.scenario_id
                    && s.requirement_ids.contains(r)
                    && s.configuration_ids.contains(&d.configuration_id)
            })
        }) {
            e.push(format!(
                "{} scenario/configuration does not cover all requirements",
                d.id
            ));
        }
    }
    e.sort();
    e.dedup();
    Ok(e)
}

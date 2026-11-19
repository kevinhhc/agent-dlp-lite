use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, path::Path};

pub const DEFAULT_POLICY: &str = include_str!("../examples/policy-pack.yaml");
pub const MAX_POLICY_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Allow,
    Warn,
    Deny,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pack {
    pub schema_version: u32,
    pub name: String,
    pub inspection_failure: Action,
    pub limits: Limits,
    pub rules: Vec<Rule>,
    pub flows: Vec<Flow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    pub max_event_bytes: usize,
    pub max_script_bytes: usize,
    pub max_total_script_bytes: usize,
    pub max_scripts: usize,
    pub max_depth: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub id: String,
    pub kind: Kind,
    pub pattern: String,
    pub action: Action,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default)]
    pub events: Vec<String>,
}
fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Secret,
    Pii,
    SensitiveFile,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Flow {
    pub id: String,
    pub source: String,
    pub sink: String,
    pub action: Action,
    #[serde(default = "yes")]
    pub enabled: bool,
}

pub struct CompiledPack {
    pub pack: Pack,
    pub rules: Vec<(Rule, Regex)>,
    pub flows: Vec<(Flow, Regex, Regex)>,
}

fn compile(pattern: &str) -> Result<Regex, String> {
    if pattern.len() > 4096 {
        return Err("pattern exceeds 4096 bytes".into());
    }
    let re = RegexBuilder::new(pattern)
        .size_limit(2 * 1024 * 1024)
        .dfa_size_limit(1024 * 1024)
        .build()
        .map_err(|_| "invalid or oversized regular expression")?;
    if re.is_match("") {
        return Err("patterns must not match empty input".into());
    }
    Ok(re)
}

impl CompiledPack {
    pub fn load(path: Option<&Path>) -> Result<Self, String> {
        let text = match path {
            Some(p) => crate::storage::read_text(p, MAX_POLICY_BYTES)?,
            None => DEFAULT_POLICY.to_owned(),
        };
        Self::parse(&text)
    }
    pub fn parse(text: &str) -> Result<Self, String> {
        if text.len() > MAX_POLICY_BYTES {
            return Err("policy exceeds size limit".into());
        }
        // Do not echo parser diagnostics: they may contain a policy's sensitive literals.
        let pack: Pack = serde_saphyr::from_str(text).map_err(|_| "invalid policy schema/YAML")?;
        if pack.schema_version != 1 {
            return Err("unsupported policy schema_version".into());
        }
        if pack.name.is_empty() || pack.name.len() > 128 {
            return Err("invalid pack name".into());
        }
        let l = &pack.limits;
        if !(1024..=1024 * 1024).contains(&l.max_event_bytes)
            || !(1024..=1024 * 1024).contains(&l.max_script_bytes)
            || !(l.max_script_bytes..=4 * 1024 * 1024).contains(&l.max_total_script_bytes)
            || !(1..=32).contains(&l.max_scripts)
            || !(1..=8).contains(&l.max_depth)
        {
            return Err("limits outside supported bounds".into());
        }
        if pack.rules.len() + pack.flows.len() > 128 {
            return Err("at most 128 rules/flows supported".into());
        }
        let mut ids = HashSet::new();
        for id in pack
            .rules
            .iter()
            .map(|r| &r.id)
            .chain(pack.flows.iter().map(|r| &r.id))
        {
            if id.is_empty()
                || id.len() > 64
                || !id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
                || !ids.insert(id.clone())
            {
                return Err("rule IDs must be unique, safe ASCII identifiers (1..64 bytes)".into());
            }
        }
        let mut rules = Vec::new();
        for rule in &pack.rules {
            if rule
                .events
                .iter()
                .any(|e| !crate::event::EVENTS.contains(&e.as_str()))
            {
                return Err("unsupported rule event".into());
            }
            let re = compile(&rule.pattern)?;
            if rule.enabled {
                rules.push((rule.clone(), re));
            }
        }
        let mut flows = Vec::new();
        for flow in &pack.flows {
            let source = compile(&flow.source)?;
            let sink = compile(&flow.sink)?;
            if flow.enabled {
                flows.push((flow.clone(), source, sink));
            }
        }
        Ok(Self { pack, rules, flows })
    }
    pub fn export(&self) -> Result<String, String> {
        serde_saphyr::to_string(&self.pack).map_err(|_| "could not serialize policy".into())
    }
}

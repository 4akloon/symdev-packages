//! `DependencyGraph`: `cargo metadata --format-version 1` output.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use serde_json::Value;

use super::Crate;
use crate::tool_error::{Result, ToolError};

/// The packages, the resolved graph and the workspace members of one `cargo metadata`.
pub struct DependencyGraph {
    packages: HashMap<String, Value>,
    /// Each node's dependencies: (package id, whether one of its kinds is normal).
    nodes: HashMap<String, Vec<(String, bool)>>,
    members: BTreeSet<String>,
}

impl DependencyGraph {
    pub fn new(metadata: &Value) -> Result<Self> {
        let missing = |what: &str| ToolError::new(format!("cargo metadata has no {what}"));
        let list = |value: Option<&Value>, what: &str| {
            value
                .and_then(Value::as_array)
                .cloned()
                .ok_or_else(|| missing(what))
        };
        let mut packages = HashMap::new();
        for package in list(metadata.get("packages"), "packages")? {
            packages.insert(Self::string(&package, "id")?, package);
        }
        let mut nodes = HashMap::new();
        for node in list(metadata.pointer("/resolve/nodes"), "resolve.nodes")? {
            let mut deps = Vec::new();
            for dep in list(node.get("deps"), "deps of a resolve node")? {
                let kinds = list(dep.get("dep_kinds"), "dep_kinds of a dependency")?;
                let normal = kinds
                    .iter()
                    .any(|k| k.get("kind").is_none_or(Value::is_null));
                deps.push((Self::string(&dep, "pkg")?, normal));
            }
            nodes.insert(Self::string(&node, "id")?, deps);
        }
        let members = list(metadata.get("workspace_members"), "workspace_members")?
            .iter()
            .map(|m| {
                m.as_str()
                    .map(String::from)
                    .ok_or_else(|| missing("workspace member id"))
            })
            .collect::<Result<_>>()?;
        Ok(Self {
            packages,
            nodes,
            members,
        })
    }

    pub fn crate_by_id(&self, id: &str) -> Result<Crate> {
        let package = self
            .packages
            .get(id)
            .ok_or_else(|| ToolError::new(format!("cargo metadata has no package `{id}`")))?;
        let optional = |key: &str| {
            package
                .get(key)
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(String::from)
        };
        let manifest = Self::string(package, "manifest_path")?;
        Ok(Crate {
            name: Self::string(package, "name")?,
            version: Self::string(package, "version")?,
            license: optional("license").unwrap_or_else(|| "NOASSERTION".into()),
            license_file: optional("license_file"),
            links: optional("links"),
            source: optional("source").unwrap_or_else(|| "path".into()),
            dir: Path::new(&manifest)
                .parent()
                .unwrap_or(Path::new(""))
                .to_path_buf(),
        })
    }

    /// The crates `root` reaches through normal dependencies, sorted by name and version
    /// (as strings), and how many workspace crates the walk passed through.
    pub fn third_party(&self, root: &str) -> Result<(Vec<Crate>, usize)> {
        let roots: Vec<&String> = self
            .members
            .iter()
            .filter(|id| {
                self.packages.get(*id).and_then(|p| p.get("name")) == Some(&Value::from(root))
            })
            .collect();
        if roots.len() != 1 {
            return Err(ToolError::new(format!(
                "the workspace has no package `{root}`; pass --package with the binary's package name"
            )));
        }
        let mut seen = BTreeSet::new();
        let mut stack = vec![roots[0].clone()];
        while let Some(id) = stack.pop() {
            if seen.insert(id.clone()) {
                let deps = self.nodes.get(&id).ok_or_else(|| {
                    ToolError::new(format!("cargo metadata has no resolve node for `{id}`"))
                })?;
                stack.extend(
                    deps.iter()
                        .filter(|(_, normal)| *normal)
                        .map(|(pkg, _)| pkg.clone()),
                );
            }
        }
        let mut crates = seen
            .difference(&self.members)
            .map(|id| self.crate_by_id(id))
            .collect::<Result<Vec<_>>>()?;
        crates.sort_by(|a, b| (&a.name, &a.version).cmp(&(&b.name, &b.version)));
        Ok((crates, seen.intersection(&self.members).count()))
    }

    fn string(value: &Value, key: &str) -> Result<String> {
        value
            .get(key)
            .and_then(Value::as_str)
            .map(String::from)
            .ok_or_else(|| {
                ToolError::new(format!("cargo metadata: an entry has no string `{key}`"))
            })
    }
}

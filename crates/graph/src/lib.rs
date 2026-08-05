//! `coreforge-graph`
//!
//! Build Graph (Phase 3).
//!
//! [`BuildGraph`] stores modules and the dependency relationships declared
//! via `coreforge.toml`'s `depends` field as a [`petgraph::graph::DiGraph`].
//! It does not know how to discover or parse modules (that is
//! `coreforge-inspector` and `coreforge-manifest`'s job) and does not know
//! how to run anything (that is `coreforge-scheduler` and
//! `coreforge-executor`'s job, Phase 4/5) - it is purely the graph structure
//! plus the graph-theoretic operations later phases need: a linear build
//! order and a leveled (parallelizable) build order.
//!
//! # Edge direction
//!
//! An edge is added from a dependency to its dependent (`dependency ->
//! dependent`). This means a normal topological sort of the graph already
//! produces a valid build order (dependencies first) without needing to
//! reverse anything.

mod error;

pub use error::{GraphError, Result};

use std::collections::{HashMap, HashSet};

use coreforge_core::{Module, ModuleId};
use petgraph::Direction;
use petgraph::algo::{tarjan_scc, toposort};
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;

/// A directed graph of [`Module`]s, linked by their `depends` relationships.
#[derive(Debug, Default)]
pub struct BuildGraph {
    graph: DiGraph<Module, ()>,
    index_of: HashMap<ModuleId, NodeIndex>,
}

impl BuildGraph {
    /// Creates an empty graph.
    #[must_use]
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            index_of: HashMap::new(),
        }
    }

    /// Builds a graph from a flat list of modules (as produced by
    /// [`coreforge_manifest::resolve_modules`]), linking every module's
    /// `depends` list as edges.
    ///
    /// Note that this does **not** check whether the resulting graph is
    /// acyclic - that check is deferred to [`BuildGraph::build_order`] /
    /// [`BuildGraph::build_levels`], so callers who only need
    /// [`BuildGraph::modules`] don't pay for it. Callers that want to fail
    /// fast on a cycle (as `coreforge-resolver` does) should call
    /// [`BuildGraph::build_order`] right after construction.
    ///
    /// # Errors
    ///
    /// Returns [`GraphError::DuplicateModule`] if two modules share an id,
    /// [`GraphError::UnknownDependency`] if a `depends` entry does not match
    /// any module in `modules`, or [`GraphError::SelfDependency`] if a
    /// module lists itself as a dependency.
    pub fn from_modules(modules: Vec<Module>) -> Result<Self> {
        let mut graph = Self::new();
        for module in modules {
            graph.add_module(module)?;
        }
        graph.link_dependencies()?;
        Ok(graph)
    }

    /// Adds a single module to the graph, without linking its dependencies.
    /// Prefer [`BuildGraph::from_modules`] unless you are building the graph
    /// incrementally.
    ///
    /// # Errors
    ///
    /// Returns [`GraphError::DuplicateModule`] if a module with the same id
    /// was already added.
    pub fn add_module(&mut self, module: Module) -> Result<NodeIndex> {
        if self.index_of.contains_key(&module.id) {
            return Err(GraphError::DuplicateModule(module.id));
        }
        let id = module.id.clone();
        let index = self.graph.add_node(module);
        self.index_of.insert(id, index);
        Ok(index)
    }

    /// Adds a `dependency -> dependent` edge for every entry in every
    /// module's `depends` list.
    fn link_dependencies(&mut self) -> Result<()> {
        let mut edges = Vec::new();

        for index in self.graph.node_indices() {
            let module = &self.graph[index];
            for dependency_id in &module.depends {
                if *dependency_id == module.id {
                    return Err(GraphError::SelfDependency(module.id.clone()));
                }
                let dependency_index = *self.index_of.get(dependency_id).ok_or_else(|| {
                    GraphError::UnknownDependency {
                        module: module.id.clone(),
                        dependency: dependency_id.clone(),
                    }
                })?;
                edges.push((dependency_index, index));
            }
        }

        for (from, to) in edges {
            self.graph.add_edge(from, to, ());
        }

        Ok(())
    }

    /// The number of modules in the graph.
    #[must_use]
    pub fn len(&self) -> usize {
        self.graph.node_count()
    }

    /// Whether the graph has no modules.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.graph.node_count() == 0
    }

    /// Looks up a module by id.
    #[must_use]
    pub fn module(&self, id: &ModuleId) -> Option<&Module> {
        self.index_of.get(id).map(|&index| &self.graph[index])
    }

    /// Iterates over every module in the graph, in an unspecified order.
    pub fn modules(&self) -> impl Iterator<Item = &Module> {
        self.graph.node_weights()
    }

    /// Returns a graph containing each requested module and all of its
    /// transitive dependencies.
    ///
    /// This is used when a caller builds a selected module instead of the
    /// entire repository graph: dependencies are retained, unrelated modules
    /// are excluded.
    ///
    /// # Errors
    ///
    /// Returns [`GraphError::ModuleNotFound`] if any requested id is absent.
    pub fn dependency_closure(&self, targets: &[ModuleId]) -> Result<Self> {
        let mut selected = HashSet::new();
        for target in targets {
            self.collect_dependencies(target, &mut selected)?;
        }

        let modules = self
            .graph
            .node_weights()
            .filter(|module| selected.contains(&module.id))
            .cloned()
            .collect();
        Self::from_modules(modules)
    }

    /// Returns a single, linear build order: every module appears after all
    /// of its (transitive) dependencies.
    ///
    /// # Errors
    ///
    /// Returns [`GraphError::CycleDetected`] if the graph is not a valid DAG.
    pub fn build_order(&self) -> Result<Vec<ModuleId>> {
        toposort(&self.graph, None)
            .map(|order| {
                order
                    .into_iter()
                    .map(|index| self.graph[index].id.clone())
                    .collect()
            })
            .map_err(|cycle| self.cycle_error_at(cycle.node_id()))
    }

    /// Groups modules into levels: every module in a given level has no
    /// dependency relationship with any other module in that same level and
    /// can therefore be built in parallel. A module only ever appears in a
    /// level after all levels containing its (transitive) dependencies.
    ///
    /// Within a level, module ids are sorted for deterministic output; the
    /// order between levels is always dependency-respecting.
    ///
    /// # Errors
    ///
    /// Returns [`GraphError::CycleDetected`] if the graph is not a valid DAG.
    pub fn build_levels(&self) -> Result<Vec<Vec<ModuleId>>> {
        let mut in_degree: HashMap<NodeIndex, usize> = self
            .graph
            .node_indices()
            .map(|index| {
                (
                    index,
                    self.graph
                        .edges_directed(index, Direction::Incoming)
                        .count(),
                )
            })
            .collect();

        let mut remaining: HashSet<NodeIndex> = self.graph.node_indices().collect();
        let mut levels = Vec::new();

        while !remaining.is_empty() {
            let this_level: Vec<NodeIndex> = remaining
                .iter()
                .copied()
                .filter(|index| in_degree[index] == 0)
                .collect();

            if this_level.is_empty() {
                // Every remaining node has at least one unsatisfied incoming
                // edge from another remaining node - the remainder is cyclic.
                let any_remaining = *remaining.iter().next().expect("remaining is non-empty");
                return Err(self.cycle_error_at(any_remaining));
            }

            for &index in &this_level {
                remaining.remove(&index);
                for edge in self.graph.edges_directed(index, Direction::Outgoing) {
                    if let Some(degree) = in_degree.get_mut(&edge.target()) {
                        *degree = degree.saturating_sub(1);
                    }
                }
            }

            let mut ids: Vec<ModuleId> = this_level
                .into_iter()
                .map(|index| self.graph[index].id.clone())
                .collect();
            ids.sort_by(|a, b| a.0.cmp(&b.0));
            levels.push(ids);
        }

        Ok(levels)
    }

    /// Finds the strongly connected component containing `offending` and
    /// turns it into a [`GraphError::CycleDetected`].
    fn cycle_error_at(&self, offending: NodeIndex) -> GraphError {
        for component in tarjan_scc(&self.graph) {
            if component.contains(&offending) {
                let ids: Vec<ModuleId> = component
                    .iter()
                    .map(|&index| self.graph[index].id.clone())
                    .collect();
                return GraphError::CycleDetected(ids);
            }
        }
        // Unreachable in practice: tarjan_scc partitions every node into
        // exactly one component, including singletons.
        GraphError::CycleDetected(vec![self.graph[offending].id.clone()])
    }

    fn collect_dependencies(
        &self,
        module_id: &ModuleId,
        selected: &mut HashSet<ModuleId>,
    ) -> Result<()> {
        if !selected.insert(module_id.clone()) {
            return Ok(());
        }

        let module = self
            .module(module_id)
            .ok_or_else(|| GraphError::ModuleNotFound(module_id.clone()))?;
        for dependency in &module.depends {
            self.collect_dependencies(dependency, selected)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8PathBuf;
    use coreforge_core::ModuleType;

    fn module(id: &str, depends: &[&str]) -> Module {
        Module {
            id: ModuleId::from(id),
            root: Utf8PathBuf::from(id),
            module_type: ModuleType::Cargo,
            depends: depends.iter().map(|d| ModuleId::from(*d)).collect(),
        }
    }

    #[test]
    fn build_order_respects_dependencies() {
        let graph = BuildGraph::from_modules(vec![
            module("editor", &["engine"]),
            module("engine", &[]),
            module("launcher", &["engine", "editor"]),
        ])
        .unwrap();

        let order = graph.build_order().unwrap();
        let pos = |id: &str| order.iter().position(|m| m.0 == id).unwrap();

        assert!(pos("engine") < pos("editor"));
        assert!(pos("engine") < pos("launcher"));
        assert!(pos("editor") < pos("launcher"));
    }

    #[test]
    fn build_levels_groups_independent_modules() {
        let graph = BuildGraph::from_modules(vec![
            module("rust", &[]),
            module("cpp", &[]),
            module("go", &[]),
            module("launcher", &["rust", "cpp", "go"]),
        ])
        .unwrap();

        let levels = graph.build_levels().unwrap();
        assert_eq!(levels.len(), 2);
        assert_eq!(levels[0].len(), 3);
        assert_eq!(levels[1], vec![ModuleId::from("launcher")]);
    }

    #[test]
    fn unknown_dependency_is_rejected() {
        let result = BuildGraph::from_modules(vec![module("editor", &["nonexistent"])]);
        assert!(matches!(
            result,
            Err(GraphError::UnknownDependency { dependency, .. }) if dependency.0 == "nonexistent"
        ));
    }

    #[test]
    fn dependency_closure_excludes_unrelated_modules() {
        let graph = BuildGraph::from_modules(vec![
            module("engine", &[]),
            module("editor", &["engine"]),
            module("docs", &[]),
        ])
        .unwrap();

        let selected = graph
            .dependency_closure(&[ModuleId::from("editor")])
            .unwrap();

        assert_eq!(selected.len(), 2);
        assert!(selected.module(&ModuleId::from("engine")).is_some());
        assert!(selected.module(&ModuleId::from("editor")).is_some());
        assert!(selected.module(&ModuleId::from("docs")).is_none());
    }

    #[test]
    fn self_dependency_is_rejected() {
        let result = BuildGraph::from_modules(vec![module("editor", &["editor"])]);
        assert!(matches!(result, Err(GraphError::SelfDependency(id)) if id.0 == "editor"));
    }

    #[test]
    fn duplicate_module_is_rejected() {
        let result = BuildGraph::from_modules(vec![module("editor", &[]), module("editor", &[])]);
        assert!(matches!(result, Err(GraphError::DuplicateModule(id)) if id.0 == "editor"));
    }

    #[test]
    fn cycle_is_detected() {
        let graph = BuildGraph::from_modules(vec![
            module("a", &["b"]),
            module("b", &["c"]),
            module("c", &["a"]),
        ])
        .unwrap();

        let err = graph.build_order().unwrap_err();
        match err {
            GraphError::CycleDetected(ids) => {
                let mut names: Vec<&str> = ids.iter().map(|i| i.0.as_str()).collect();
                names.sort_unstable();
                assert_eq!(names, vec!["a", "b", "c"]);
            }
            other => panic!("expected CycleDetected, got {other:?}"),
        }
    }
}

//! Dataflow graphs that evaluate to a [`ringdesign_core::RingDesign`].
//!
//! The sand-cast ring is one height field over a swept band, and every
//! panel, template and MCP tool in the workspace builds it by calling the
//! same core API in some order. A graph makes that order a document: nodes
//! are those calls, wires carry their values, and evaluating the graph is
//! calling them. The core stays the model and the sinks; this crate is the
//! runtime above it and knows nothing about a window.
//!
//! Decisions fixed before the first line, recorded here because each one is
//! hard to change later:
//!
//! - **Implicit lists.** A pin fed `N` items runs its node `N` times, the
//!   shortest lists repeating their last item (longest-list matching). An
//!   empty list in is an empty list out. A failed item is a `Null` with an
//!   attributed error; its siblings continue. Nested lists pass whole.
//! - **A closed [`Value`](value::Value) enum** with `Arc` domain handles —
//!   profile, shank, head, layer, stack, design, mesh and the rest — never a
//!   JSON tree and never reflection. Coercions are a table with a test.
//! - **Stable node identity.** `NodeId` is a `u64` the graph hands out once;
//!   it is never a position in a list. The serde `Graph` is the truth and
//!   every editor is a view rebuilt from it.
//! - **Native evaluation** with a per-node recipe signature cache, so one
//!   edit re-runs one chain. Scripts (rhai) only run at expression pins and
//!   script nodes, never as a transpile of the whole graph.
//! - **Mode is a property of the graph.** `SandRing` evaluates to a design
//!   *with* its field verdict and refuses to write a file for a ring that
//!   will not release; `Free` adds the solid kernel and a mesh verifier.
//! - **A design carries its graph** — `RingDesign::graph`, live until baked,
//!   the `GroupLayer::recipe` pattern one level up — and standalone graph,
//!   cluster and preset files carry their own version ladder.
//! - **Everything a literal can size is capped**: list items per pin, nodes
//!   per graph, cluster depth.
//!
//! The modules fill in that order: [`value`], [`graph`], [`registry`],
//! [`eval`], then the node library under [`nodes`].

pub mod eval;
pub mod file;
pub mod graph;
pub mod nodes;
pub mod registry;
pub mod value;

/// Most items one pin accepts; a longer list is truncated with a warning.
pub const MAX_LIST_ITEMS: usize = 4096;
/// Most nodes one graph holds.
pub const MAX_NODES: usize = 4096;
/// Deepest cluster nesting an evaluation follows.
pub const MAX_CLUSTER_DEPTH: usize = 8;

#[cfg(test)]
mod tests {
    #[test]
    fn the_caps_are_finite_and_ordered() {
        assert!(super::MAX_LIST_ITEMS >= 256);
        assert!(super::MAX_NODES >= 256);
        assert!(super::MAX_CLUSTER_DEPTH >= 2);
    }
}

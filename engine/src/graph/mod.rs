//! Граф сущностей (Entity-Relation Graph) с K-hop обходом;
//! коннектом FLYCSR1 (мозг мухи FlyWire v783) как матрица A.

pub mod causal_linker;
pub mod chem_kernel;
pub mod connectome;
pub mod entity_graph;
pub mod flyops;
pub mod graph_asm;

pub use connectome::{Connectome, ConnectomeNodes, Edge, InEdges, KHopReport, SignFilter};
pub use entity_graph::{CodeSymbolRef, EntityGraph, IdentityPolicy, KnowledgeNode, RelationEdge};
pub use flyops::{
    CommonPartner, Direction, MotifReport, PageRankReport, PathHop, PathReport, PropagateReport,
    PropagateStep, RotorPair,
};
pub use graph_asm::{
    CompiledMachineGraph, ComputeEdge, DataflowCompiler, DataflowEdge, DataflowGraph,
    GraphMachineCompiler, LayerSpec, ModelComputeGraph, NodeAct, Reducer, WeightKind,
};


//! Property-based tests for interchange round-trips.

use proptest::prelude::*;

use svt_core::interchange::{
    InterchangeDocument, InterchangeEdge, InterchangeNode,
};
use svt_core::interchange_store;
use svt_core::model::*;
use svt_core::store::CozoStore;

fn arb_node_kind() -> impl Strategy<Value = NodeKind> {
    prop_oneof![
        Just(NodeKind::System),
        Just(NodeKind::Service),
        Just(NodeKind::Component),
        Just(NodeKind::Unit),
    ]
}

/// Generate a valid interchange document with N nodes in a flat hierarchy.
fn arb_document(max_nodes: usize) -> impl Strategy<Value = InterchangeDocument> {
    (1..=max_nodes)
        .prop_flat_map(|n| {
            proptest::collection::vec(arb_node_kind(), n).prop_map(move |kinds| {
                let mut nodes = Vec::new();
                let root_path = "/test".to_string();

                nodes.push(InterchangeNode {
                    canonical_path: root_path.clone(),
                    kind: NodeKind::System,
                    name: Some("test".to_string()),
                    sub_kind: Some("system".to_string()),
                    qualified_name: None,
                    language: None,
                    provenance: None,
                    source_ref: None,
                    metadata: None,
                    children: None,
                });

                let mut edges = vec![];

                for (i, kind) in kinds.iter().enumerate() {
                    let path = format!("/test/node-{}", i);
                    nodes.push(InterchangeNode {
                        canonical_path: path.clone(),
                        kind: *kind,
                        name: Some(format!("node-{}", i)),
                        sub_kind: Some("module".to_string()),
                        qualified_name: None,
                        language: None,
                        provenance: None,
                        source_ref: None,
                        metadata: None,
                        children: None,
                    });
                    edges.push(InterchangeEdge {
                        source: root_path.clone(),
                        target: path,
                        kind: EdgeKind::Contains,
                        metadata: None,
                    });
                }

                InterchangeDocument {
                    format: "svt/v1".to_string(),
                    kind: SnapshotKind::Design,
                    version: None,
                    metadata: None,
                    nodes,
                    edges,
                    constraints: vec![],
                }
            })
        })
}

proptest! {
    #[test]
    fn import_export_round_trip_preserves_node_count(doc in arb_document(10)) {
        let mut store = CozoStore::new_in_memory().unwrap();
        let version = interchange_store::load_into_store(&mut store, &doc).unwrap();

        let exported_yaml = interchange_store::export_yaml(&store, version).unwrap();
        let re_parsed = svt_core::interchange::parse_yaml(&exported_yaml).unwrap();

        prop_assert_eq!(re_parsed.nodes.len(), doc.nodes.len());
    }

    #[test]
    fn import_export_round_trip_preserves_edge_count(doc in arb_document(10)) {
        let mut store = CozoStore::new_in_memory().unwrap();
        let version = interchange_store::load_into_store(&mut store, &doc).unwrap();

        let exported_yaml = interchange_store::export_yaml(&store, version).unwrap();
        let re_parsed = svt_core::interchange::parse_yaml(&exported_yaml).unwrap();

        prop_assert_eq!(re_parsed.edges.len(), doc.edges.len());
    }
}

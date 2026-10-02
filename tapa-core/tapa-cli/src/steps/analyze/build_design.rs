//! The analysis-time check for a leaf top task.

use tapa_ir::{Graph, TaskLevel};

/// True when the top task in `graph` is a leaf-level task.
pub(super) fn is_top_leaf(graph: &Graph, top: &str) -> bool {
    graph
        .tasks
        .get(top)
        .is_some_and(|task| task.level == TaskLevel::Lower)
}

#[cfg(test)]
mod tests {
    use super::*;

    use serde_json::json;

    #[test]
    fn is_top_leaf_detects_lower_level() {
        let g: Graph = serde_json::from_value(json!({"target": "xilinx-hls", "tasks": {"T": {"level": "lower", "srcs": ["T.cpp"], "include_dirs": [], "defines": [], "synth": "hls", "readable_name": "T", "ports": [], "tasks": {}, "fifos": {}}}, "top": "T"})).unwrap();
        assert!(is_top_leaf(&g, "T"));
        let g: Graph = serde_json::from_value(json!({"target": "xilinx-hls", "tasks": {"T": {"level": "upper", "srcs": ["T.cpp"], "include_dirs": [], "defines": [], "synth": "hls", "readable_name": "T", "ports": [], "tasks": {}, "fifos": {}}}, "top": "T"})).unwrap();
        assert!(!is_top_leaf(&g, "T"));
        // Missing top is treated as upper for safety.
        assert!(!is_top_leaf(&g, "DoesNotExist"));
    }
}

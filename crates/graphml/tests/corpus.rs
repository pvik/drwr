//! Round-trips real GraphML files, such as a folder of yEd diagrams:
//!
//! ```sh
//! GRAPHML_CORPUS=~/diagrams cargo test -p graphml --test corpus -- --ignored
//! ```

use std::{
    env, fs,
    path::{Path, PathBuf},
};

use graphml::{Graph, GraphMl, yed::Keys};

#[test]
#[ignore = "needs GRAPHML_CORPUS, a list of .graphml files or folders separated like PATH"]
fn corpus_round_trips() {
    let paths = env::var_os("GRAPHML_CORPUS").expect("GRAPHML_CORPUS is not set");
    let mut files = Vec::new();
    for path in env::split_paths(&paths) {
        collect(&path, &mut files);
    }
    assert!(!files.is_empty(), "no .graphml files in GRAPHML_CORPUS");

    let mut graphics = 0;
    for file in &files {
        let text = fs::read_to_string(file).unwrap();
        let doc = GraphMl::parse(&text).unwrap_or_else(|err| panic!("{}: {err}", file.display()));
        if let Some(difference) = first_difference(&text, &doc.to_xml()) {
            panic!("{}: {difference}", file.display());
        }
        for graph in &doc.graphs {
            graphics += check_graphics(&Keys::find(&doc), graph, file);
        }
    }
    eprintln!(
        "{} files and {graphics} graphics round-tripped",
        files.len()
    );
}

fn collect(path: &Path, files: &mut Vec<PathBuf>) {
    if path.is_dir() {
        for entry in fs::read_dir(path).unwrap() {
            collect(&entry.unwrap().path(), files);
        }
    } else if path
        .extension()
        .is_some_and(|extension| extension == "graphml")
    {
        files.push(path.to_owned());
    }
}

/// Checks that every node's and edge's graphics are read without error, and
/// that writing them back unchanged leaves the element as it was.
fn check_graphics(keys: &Keys, graph: &Graph, file: &Path) -> usize {
    let mut count = 0;
    for node in &graph.nodes {
        let read = keys.node_graphics(node);
        let read = read.unwrap_or_else(|err| panic!("{}: node {}: {err}", file.display(), node.id));
        if let Some(graphics) = read {
            let mut copy = node.clone();
            keys.set_node_graphics(&mut copy, &graphics);
            assert!(
                copy == *node,
                "{}: node {} changed",
                file.display(),
                node.id
            );
            count += 1;
        }
        if let Some(graph) = &node.graph {
            count += check_graphics(keys, graph, file);
        }
    }
    for edge in &graph.edges {
        let id = edge.id.as_deref().unwrap_or("?");
        let read = keys.edge_graphics(edge);
        let read = read.unwrap_or_else(|err| panic!("{}: edge {id}: {err}", file.display()));
        if let Some(graphics) = read {
            let mut copy = edge.clone();
            keys.set_edge_graphics(&mut copy, &graphics);
            assert!(copy == *edge, "{}: edge {id} changed", file.display());
            count += 1;
        }
    }
    count
}

/// Describes the first line where `actual` differs from `expected`.
fn first_difference(expected: &str, actual: &str) -> Option<String> {
    if expected == actual {
        return None;
    }
    let mut actual_lines = actual.lines();
    for (number, expected_line) in expected.lines().enumerate() {
        match actual_lines.next() {
            Some(actual_line) if actual_line == expected_line => {}
            actual_line => {
                return Some(format!(
                    "line {} differs\nexpected: {expected_line:?}\n  actual: {actual_line:?}",
                    number + 1
                ));
            }
        }
    }
    Some("the output has extra lines or line endings differ".to_owned())
}

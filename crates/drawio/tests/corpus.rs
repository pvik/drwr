//! Converts real draw.io files, such as draw.io's own templates:
//!
//! ```sh
//! DRAWIO_CORPUS=~/diagrams cargo test -p drawio --test corpus -- --ignored --nocapture
//! ```
//!
//! With `DRAWIO_OUT` set to a folder, it writes each page there as GraphML.

use std::{
    collections::{BTreeMap, HashSet},
    env, fs,
    path::{Path, PathBuf},
};

use graphml::{GraphMl, yed::Keys};

#[test]
#[ignore = "needs DRAWIO_CORPUS, a list of draw.io files or folders separated like PATH"]
fn corpus_converts() {
    let paths = env::var_os("DRAWIO_CORPUS").expect("DRAWIO_CORPUS is not set");
    let mut files = Vec::new();
    for path in env::split_paths(&paths) {
        collect(&path, &mut files);
    }
    assert!(!files.is_empty(), "no draw.io files in DRAWIO_CORPUS");

    let out = env::var_os("DRAWIO_OUT").map(PathBuf::from);
    let (mut pages, mut nodes, mut edges) = (0, 0, 0);
    let mut notes: BTreeMap<String, usize> = BTreeMap::new();
    for file in &files {
        let text = fs::read_to_string(file).unwrap();
        if !drawio::is_drawio(&text) {
            continue;
        }
        let parsed =
            drawio::File::parse(&text).unwrap_or_else(|err| panic!("{}: {err}", file.display()));
        for (index, page) in parsed.pages().iter().enumerate() {
            let import = page.to_graphml();
            if let Some(out) = &out {
                let stem = file.file_stem().unwrap().to_string_lossy();
                let name = format!("{stem}-{}.graphml", index + 1);
                fs::write(out.join(name), import.graphml.to_xml()).unwrap();
            }
            let doc = &import.graphml;
            check(doc, file);
            assert_eq!(
                &GraphMl::parse(&doc.to_xml()).unwrap(),
                doc,
                "{}",
                file.display()
            );
            pages += 1;
            nodes += doc.graphs[0].nodes.len();
            edges += doc.graphs[0].edges.len();
            for note in import.notes {
                // Counted by kind: without numbers or the list after a colon.
                let kind = note.split(':').next().unwrap_or_default();
                let kind: String = kind.chars().filter(|c| !c.is_ascii_digit()).collect();
                *notes.entry(kind.trim().to_owned()).or_default() += 1;
            }
        }
    }
    eprintln!(
        "{} files, {pages} pages: {nodes} nodes, {edges} edges",
        files.len()
    );
    for (note, count) in notes {
        eprintln!("{count:5} pages: {note}");
    }
}

/// Checks every node has readable graphics and every edge joins two nodes.
fn check(doc: &GraphMl, file: &Path) {
    let keys = Keys::find(doc);
    let graph = &doc.graphs[0];
    let mut ids = HashSet::new();
    for node in &graph.nodes {
        assert!(
            ids.insert(node.id.as_str()),
            "{}: duplicate {}",
            file.display(),
            node.id
        );
        let graphics = keys.node_graphics(node).unwrap().unwrap();
        let g = graphics.geometry;
        let finite = [g.x, g.y, g.width, g.height].iter().all(|v| v.is_finite());
        assert!(
            finite && g.width > 0.0 && g.height > 0.0,
            "{}: {}",
            file.display(),
            node.id
        );
    }
    for edge in &graph.edges {
        assert!(ids.contains(edge.source.as_str()), "{}", file.display());
        assert!(ids.contains(edge.target.as_str()), "{}", file.display());
        let graphics = keys.edge_graphics(edge).unwrap().unwrap();
        let path = &graphics.path;
        let points = path.points.iter().chain([&path.source, &path.target]);
        assert!(
            points
                .into_iter()
                .all(|p| p.x.is_finite() && p.y.is_finite())
        );
    }
}

fn collect(path: &Path, files: &mut Vec<PathBuf>) {
    if path.is_dir() {
        let mut entries: Vec<_> = fs::read_dir(path).unwrap().flatten().collect();
        entries.sort_by_key(|entry| entry.path());
        for entry in entries {
            collect(&entry.path(), files);
        }
    } else if path
        .extension()
        .is_some_and(|ext| ext == "drawio" || ext == "dio" || ext == "xml")
    {
        files.push(path.to_owned());
    }
}

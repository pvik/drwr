//! Reads and writes [GraphML](http://graphml.graphdrawing.org/) documents.
//!
//! [`GraphMl`] models the GraphML structure and keeps the parts it doesn't
//! model, so documents are written back without losing content. Output is
//! formatted the way yEd formats it, so yEd's files round-trip byte for byte.
//! The [`yed`] module reads and edits the graphics yEd stores in `<data>`.
//!
//! ```
//! use graphml::{GraphMl, yed};
//!
//! let mut doc = yed::new_document();
//! let keys = yed::Keys::ensure(&mut doc);
//! let mut node = graphml::Node::new("n0");
//! let mut graphics = yed::NodeGraphics::new(yed::NodeKind::Shape(yed::ShapeType::Ellipse));
//! graphics.labels.push(yed::Label::new("Hello"));
//! keys.set_node_graphics(&mut node, &graphics);
//! doc.graphs[0].nodes.push(node);
//!
//! let text = doc.to_xml();
//! assert_eq!(GraphMl::parse(&text)?, doc);
//! # Ok::<(), graphml::Error>(())
//! ```

mod error;
mod model;
mod read;
mod write;
pub mod xml;
pub mod yed;

pub use error::{Error, Position};
pub use model::*;

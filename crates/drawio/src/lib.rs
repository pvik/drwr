//! Reads [draw.io](https://www.drawio.com) diagrams and converts their pages
//! to GraphML with yEd graphics, the closest yEd shape standing in for each
//! of draw.io's.
//!
//! ```
//! let text = r#"<mxfile><diagram name="Page-1"><mxGraphModel><root>
//!   <mxCell id="0"/><mxCell id="1" parent="0"/>
//!   <mxCell id="2" value="Hello" vertex="1" parent="1">
//!     <mxGeometry x="10" y="20" width="120" height="60" as="geometry"/>
//!   </mxCell>
//! </root></mxGraphModel></diagram></mxfile>"#;
//! assert!(drawio::is_drawio(text));
//! let file = drawio::File::parse(text)?;
//! let import = file.pages()[0].to_graphml();
//! assert_eq!(import.graphml.graphs[0].nodes.len(), 1);
//! # Ok::<(), drawio::Error>(())
//! ```

mod convert;
mod error;
mod file;
mod model;
mod route;
mod shape;
mod style;
mod text;

pub use convert::Import;
pub use error::Error;
pub use file::{File, Page, is_drawio};

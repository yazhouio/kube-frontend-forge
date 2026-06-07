extern crate self as swc_core;

pub mod common {
    pub use swc_common::*;
}

pub mod atoms {
    pub use swc_atoms::*;
}

pub mod ecma {
    pub mod ast {
        pub use swc_ecma_ast::*;
    }
}

pub mod action_graph;
pub mod ast;
pub mod builtins;
pub mod error;
pub mod generator;
pub mod imports;
pub mod model;
pub mod names;
pub mod registry;
pub mod rename;
pub mod value;

pub use error::{Error, Result};
pub use generator::ComponentGenerator;
pub use model::{PageConfig, unwrap_page_schema};

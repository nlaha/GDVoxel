use godot::prelude::*;

// local imports
mod chunk;
mod gpu;
mod manager;
mod threaded_generator;

pub use crate::chunk::*;
pub use crate::gpu::*;
pub use crate::manager::*;
pub use crate::threaded_generator::*;

struct GDVoxel;

#[gdextension]
unsafe impl ExtensionLibrary for GDVoxel {}

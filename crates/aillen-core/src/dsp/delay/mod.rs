pub mod tape;
pub mod granular;
pub mod stereo;
pub mod variable;

pub use tape::TapeDelay;
pub use granular::GranularDelay;
pub use stereo::{DelayMode, StereoDelay};
pub use variable::VariableDelay;

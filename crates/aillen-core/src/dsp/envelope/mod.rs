/// Linear Attack-Decay-Sustain-Release envelope generator.
pub mod adsr;
/// Exponential sweep decay envelope generator.
pub mod expon;

pub use adsr::{AdsrEnvelope, EnvelopeState};
pub use expon::ExponEnvelope;

pub mod two_op;
pub mod voice;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SynthMode {
    Additive,
    Am,
    Rm,
    Fm,
}

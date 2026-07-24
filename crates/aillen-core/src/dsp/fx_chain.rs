use crate::dsp::{Compressor, AmRingMod, StereoProcessor, AudioProcessor};
use crate::dsp::filter::DjFilter;

/// A sequential stereo effects processor applying:
/// 1. Ring Modulation
/// 2. DJ-style Low-Pass/High-Pass Filter
/// 3. Dynamic Range Compression
pub struct FxChain {
    /// Ring modulator left channel.
    pub ring_mod_l: AmRingMod,
    /// Ring modulator right channel.
    pub ring_mod_r: AmRingMod,
    /// DJ performance filter left channel.
    pub dj_filter_l: DjFilter,
    /// DJ performance filter right channel.
    pub dj_filter_r: DjFilter,
    /// Compressor left channel.
    pub compressor_l: Compressor,
    /// Compressor right channel.
    pub compressor_r: Compressor,
}

impl FxChain {
    /// Creates a new `FxChain` configured for the specified sample rate.
    pub fn new(sample_rate: f32) -> Self {
        Self {
            ring_mod_l: AmRingMod::new(sample_rate),
            ring_mod_r: AmRingMod::new(sample_rate),
            dj_filter_l: DjFilter::new(sample_rate),
            dj_filter_r: DjFilter::new(sample_rate),
            compressor_l: Compressor::new(sample_rate),
            compressor_r: Compressor::new(sample_rate),
        }
    }
}

impl StereoProcessor for FxChain {
    /// Processes a stereo frame sequentially through the Ring Modulators,
    /// DJ Filters, and then the Compressors.
    fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        // 1. Ring Modulator
        let rm_l = self.ring_mod_l.process(left);
        let rm_r = self.ring_mod_r.process(right);

        // 2. DJ Filter
        let filt_l = self.dj_filter_l.process(rm_l);
        let filt_r = self.dj_filter_r.process(rm_r);

        // 3. Compressor
        let comp_l = self.compressor_l.process(filt_l);
        let comp_r = self.compressor_r.process(filt_r);

        (comp_l, comp_r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fx_chain_bypass_by_default() {
        let mut chain = FxChain::new(44100.0);
        
        // By default, all FXs should be off/dry.
        // Send a frame through.
        let (out_l, out_r) = chain.process_stereo(0.5, 0.5);
        
        // Output should match input exactly
        assert!((out_l - 0.5).abs() < 1e-5);
        assert!((out_r - 0.5).abs() < 1e-5);
    }
    
    #[test]
    fn test_fx_chain_active() {
        let mut chain = FxChain::new(44100.0);
        
        // Turn on ring modulation and compressor
        chain.ring_mod_l.depth = 1.0;
        chain.ring_mod_l.frequency = 100.0;
        chain.compressor_l.ratio = 4.0;
        chain.compressor_l.threshold = -20.0;
        
        let (out_l, _) = chain.process_stereo(1.0, 1.0);
        
        // Left channel should be processed/changed
        assert!(out_l != 1.0);
    }
}

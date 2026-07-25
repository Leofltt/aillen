use crate::dsp::{Compressor, AmRingMod, StereoProcessor, AudioProcessor, ModulationSource, Distortion, DistortionMode};
use crate::dsp::filter::DjFilter;

/// A sequential stereo effects processor applying:
/// 1. Ring Modulation
/// 2. Distortion/Drive Saturation
/// 3. DJ-style Low-Pass/High-Pass Filter
/// 4. Dynamic Range Compression (with sidechaining support)
pub struct FxChain {
    /// Ring modulator left channel.
    pub ring_mod_l: AmRingMod,
    /// Ring modulator right channel.
    pub ring_mod_r: AmRingMod,
    /// Distortion left channel.
    pub distortion_l: Distortion,
    /// Distortion right channel.
    pub distortion_r: Distortion,
    /// DJ performance filter left channel.
    pub dj_filter_l: DjFilter,
    /// DJ performance filter right channel.
    pub dj_filter_r: DjFilter,
    /// Compressor left channel.
    pub compressor_l: Compressor,
    /// Compressor right channel.
    pub compressor_r: Compressor,
    /// When true, the compressor is sidechained by the external sidechain inputs.
    pub compressor_sidechain: bool,
}

impl FxChain {
    /// Creates a new `FxChain` configured for the specified sample rate.
    pub fn new(sample_rate: f32) -> Self {
        Self {
            ring_mod_l: AmRingMod::new(sample_rate),
            ring_mod_r: AmRingMod::new(sample_rate),
            distortion_l: Distortion::new(DistortionMode::Bypass, 1.0, 0.0),
            distortion_r: Distortion::new(DistortionMode::Bypass, 1.0, 0.0),
            dj_filter_l: DjFilter::new(sample_rate),
            dj_filter_r: DjFilter::new(sample_rate),
            compressor_l: Compressor::new(sample_rate),
            compressor_r: Compressor::new(sample_rate),
            compressor_sidechain: false,
        }
    }

    /// Processes a stereo frame sequentially with external sidechain signals.
    pub fn process_stereo_with_sidechain(
        &mut self,
        left: f32,
        right: f32,
        sidechain_l: f32,
        sidechain_r: f32,
    ) -> (f32, f32) {
        // 1. Ring Modulator
        let rm_l = if self.ring_mod_l.source == ModulationSource::Sidechain {
            self.ring_mod_l.process_sidechain(left, sidechain_l)
        } else {
            self.ring_mod_l.process(left)
        };
        let rm_r = if self.ring_mod_r.source == ModulationSource::Sidechain {
            self.ring_mod_r.process_sidechain(right, sidechain_r)
        } else {
            self.ring_mod_r.process(right)
        };

        // 2. Distortion/Saturation
        let dist_l = self.distortion_l.process(rm_l);
        let dist_r = self.distortion_r.process(rm_r);

        // 3. DJ Filter
        let filt_l = self.dj_filter_l.process(dist_l);
        let filt_r = self.dj_filter_r.process(dist_r);

        // 4. Compressor
        let comp_l = if self.compressor_sidechain {
            self.compressor_l.process_sidechain(filt_l, sidechain_l)
        } else {
            self.compressor_l.process(filt_l)
        };
        let comp_r = if self.compressor_sidechain {
            self.compressor_r.process_sidechain(filt_r, sidechain_r)
        } else {
            self.compressor_r.process(filt_r)
        };

        (comp_l, comp_r)
    }
}

impl StereoProcessor for FxChain {
    /// Processes a stereo frame sequentially through the Ring Modulators,
    /// DJ Filters, and then the Compressors (using internal self-sidechaining).
    fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        self.process_stereo_with_sidechain(left, right, 0.0, 0.0)
    }

    /// Processes a stereo frame sequentially with external sidechain signals.
    fn process_stereo_modulated(&mut self, left: f32, right: f32, sidechain_l: f32, sidechain_r: f32) -> (f32, f32) {
        self.process_stereo_with_sidechain(left, right, sidechain_l, sidechain_r)
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

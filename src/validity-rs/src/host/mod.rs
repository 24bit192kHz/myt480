//! Host-side fingerprint pipeline for the 06cb:009a: frame calibration and stacking,
//! minutiae extraction and MCC matching. Experimental: the chip stays the authority
//! unless the measured false-accept gate passes (see `eval`).

pub mod cli;
pub mod eval;
pub mod fft;
pub mod img;
pub mod mcc;
pub mod minutiae;
pub mod mosaic;
pub mod synth;

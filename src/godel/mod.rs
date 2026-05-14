//! Omega-5 Godel-machine substrate.
//!
//! This module starts with the typed observer. Later Omega-5 caps add
//! criteria, verification, proposal, application, and the closed loop.

pub mod observer;
pub mod hardware;
pub mod fabric;
pub mod criteria;
pub mod verifier;
pub mod proposer;
pub mod applicator;
pub mod runner;
pub mod verifier_v2;
pub mod applicator_v2;

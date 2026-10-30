//! Core types shared by every stage of the framefeed pipeline.
//!
//! This crate is dependency-free and pure on purpose: it contains the frame and
//! raster containers plus render planning (geometry, fit modes, height caps).
//! Nothing here touches a terminal, a clock, a file or a network socket, which
//! keeps the hot path easy to test and benchmark.

#![forbid(unsafe_code)]

mod frame;
mod geometry;

pub use frame::{Frame, InvalidBuffer, PixelFormat, Raster};
pub use geometry::{CellSize, Crop, DEFAULT_CELL, Fit, RenderPlan, Size, Viewport, plan_render};

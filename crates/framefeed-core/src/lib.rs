//! Core types shared by every stage of the framefeed pipeline.
//!
//! This crate is dependency-free and pure on purpose: it contains the frame and
//! raster containers, render planning (geometry, fit modes, height caps) and the
//! presentation pacer. Nothing here touches a terminal, a file or a network
//! socket, which keeps the hot path easy to test and benchmark.

#![forbid(unsafe_code)]

mod frame;
mod geometry;
mod pacer;

pub use frame::{Frame, InvalidBuffer, PixelFormat, Raster};
pub use geometry::{CellSize, Crop, DEFAULT_CELL, Fit, RenderPlan, Size, Viewport, plan_render};
pub use pacer::{Pace, Pacer};

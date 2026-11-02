//! Write a demo result file in OpenModelica MAT format for GUI development without OMC.
//!
//! `cargo run -p omola-gui --example demo_mat -- target/demo_res.mat`
//!
//! The values are an analytic bouncing ball, NOT an OpenModelica simulation. Use it only
//! with `scripts/fake-omc.sh` to exercise the GUI; never as compatibility evidence.

use omola_core::mat::{write_trajectory, ResultVariable, TrajectoryResult};

fn main() {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/demo_res.mat".into());
    let (g, e, dt) = (9.81_f64, 0.7_f64, 0.001_f64);
    let (mut h, mut v) = (1.0_f64, 0.0_f64);
    let mut time = Vec::new();
    let (mut hs, mut vs) = (Vec::new(), Vec::new());
    for k in 0..=3000 {
        time.push(k as f64 * dt);
        hs.push(h);
        vs.push(v);
        v -= g * dt;
        h += v * dt;
        if h <= 0.0 {
            h = 0.0;
            v *= -e;
        }
    }
    let n = time.len();
    let var = |name: &str, description: &str, is_parameter: bool| ResultVariable {
        name: name.into(),
        description: description.into(),
        is_parameter,
    };
    let result = TrajectoryResult {
        variables: vec![
            var("time", "Simulation time [s]", false),
            var("height", "Height above the ground (m)", false),
            var("velocity", "Vertical velocity (m/s)", false),
            var("gravity", "Gravitational acceleration (m/s2)", true),
            var("restitution", "Velocity retention at impact", true),
        ],
        values: vec![time.clone(), hs, vs, vec![g; n], vec![e; n]],
        time,
    };
    let bytes = write_trajectory(&result).expect("valid demo result");
    std::fs::write(&path, bytes).expect("write demo result");
    println!("wrote {path}");
}

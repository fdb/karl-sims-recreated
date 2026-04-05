//! JSON-serializable creature definitions for hand-crafted test creatures.
//!
//! These bypass the genome → phenotype pipeline and directly define
//! bodies, joints, and oscillator-driven torques.

use glam::DVec3;
use serde::{Deserialize, Serialize};

use crate::joint::Joint;
use crate::world::World;

// ---------------------------------------------------------------------------
// Definition types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatureDefinition {
    pub name: String,
    pub bodies: Vec<BodyDef>,
    pub joints: Vec<JointDef>,
    #[serde(default)]
    pub torques: Vec<TorqueDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BodyDef {
    /// Full dimensions [x, y, z] — halved internally for half-extents.
    pub dimensions: [f64; 3],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JointDef {
    pub parent: usize,
    pub child: usize,
    /// "revolute", "universal", or "spherical"
    #[serde(default = "default_joint_type")]
    pub joint_type: String,
    /// Primary rotation axis (for revolute/universal).
    #[serde(default = "default_axis_z")]
    pub axis: [f64; 3],
    /// Secondary axis (for universal joints).
    #[serde(default)]
    pub secondary_axis: [f64; 3],
    pub parent_anchor: [f64; 3],
    pub child_anchor: [f64; 3],
    #[serde(default = "default_angle_min")]
    pub angle_min: f64,
    #[serde(default = "default_angle_max")]
    pub angle_max: f64,
    #[serde(default = "default_damping")]
    pub damping: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TorqueDef {
    pub joint: usize,
    #[serde(default)]
    pub dof: usize,
    #[serde(default = "default_amplitude")]
    pub amplitude: f64,
    #[serde(default = "default_frequency")]
    pub frequency: f64,
    #[serde(default)]
    pub phase: f64,
}

fn default_joint_type() -> String { "revolute".into() }
fn default_axis_z() -> [f64; 3] { [0.0, 0.0, 1.0] }
fn default_angle_min() -> f64 { -1.0 }
fn default_angle_max() -> f64 { 1.0 }
fn default_damping() -> f64 { 0.3 }
fn default_amplitude() -> f64 { 2.0 }
fn default_frequency() -> f64 { 3.0 }

// ---------------------------------------------------------------------------
// Build a World from a definition
// ---------------------------------------------------------------------------

impl CreatureDefinition {
    /// Build a physics World from this definition.
    pub fn build_world(&self) -> World {
        let mut world = World::new();

        for body_def in &self.bodies {
            let he = DVec3::new(
                body_def.dimensions[0] * 0.5,
                body_def.dimensions[1] * 0.5,
                body_def.dimensions[2] * 0.5,
            );
            world.add_body(he);
        }

        world.root = 0;

        for jd in &self.joints {
            let pa = DVec3::from_slice(&jd.parent_anchor);
            let ca = DVec3::from_slice(&jd.child_anchor);
            let axis = DVec3::from_slice(&jd.axis).normalize_or_zero();

            let mut joint = match jd.joint_type.as_str() {
                "universal" => {
                    let sec = DVec3::from_slice(&jd.secondary_axis).normalize_or_zero();
                    Joint::universal(jd.parent, jd.child, pa, ca, axis, sec)
                }
                "spherical" => Joint::spherical(jd.parent, jd.child, pa, ca),
                _ => Joint::revolute(jd.parent, jd.child, pa, ca, axis),
            };

            joint.angle_min = [jd.angle_min; 3];
            joint.angle_max = [jd.angle_max; 3];
            joint.damping = jd.damping;
            world.add_joint(joint);
        }

        // Place child bodies so joint anchors coincide. Without this the
        // Rapier backend (which treats poses as authoritative) sees every
        // body at the origin and applies huge corrective impulses in frame 1.
        world.forward_kinematics();

        world
    }

    /// Apply oscillator torques based on current world time.
    pub fn apply_torques(&self, world: &mut World) {
        for td in &self.torques {
            if td.joint < world.torques.len() && td.dof < 3 {
                let t = world.time;
                world.torques[td.joint][td.dof] =
                    td.amplitude * (td.frequency * t * std::f64::consts::TAU + td.phase).sin();
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Simulation runner (used by CLI and tests)
// ---------------------------------------------------------------------------

/// Per-frame output from a simulation.
#[derive(Debug, Clone, Serialize)]
pub struct FrameRecord {
    pub frame: usize,
    pub time: f64,
    /// (x, y, z) for each body's center position.
    pub positions: Vec<[f64; 3]>,
}

/// Run a simulation and collect per-frame data.
pub fn simulate(
    def: &CreatureDefinition,
    environment: &str,
    gravity: f64,
    frames: usize,
    dt: f64,
) -> Vec<FrameRecord> {
    let mut world = def.build_world();

    match environment {
        "land" | "Land" => {
            world.water_enabled = false;
            world.gravity = DVec3::new(0.0, -gravity, 0.0);
            world.ground_enabled = true;
            // Raise root above ground
            world.set_root_transform(
                glam::DAffine3::from_translation(DVec3::new(0.0, 2.0, 0.0)),
            );
            world.forward_kinematics();
        }
        _ => {
            world.water_enabled = true;
            world.water_viscosity = 2.0;
            world.gravity = DVec3::ZERO;
        }
    }

    let mut records = Vec::with_capacity(frames + 1);

    // Frame 0: initial state
    records.push(collect_frame(&world, 0));

    for f in 1..=frames {
        def.apply_torques(&mut world);
        world.step(dt);
        records.push(collect_frame(&world, f));
    }

    records
}

fn collect_frame(world: &World, frame: usize) -> FrameRecord {
    let positions = world.transforms.iter().map(|t| {
        [t.translation.x, t.translation.y, t.translation.z]
    }).collect();

    FrameRecord {
        frame,
        time: world.time,
        positions,
    }
}

// ---------------------------------------------------------------------------
// Built-in creatures
// ---------------------------------------------------------------------------

/// Swimming starfish: 4 flippers, oscillating torques, designed for water.
pub fn swimmer_starfish() -> CreatureDefinition {
    CreatureDefinition {
        name: "Swimming Starfish".into(),
        bodies: vec![
            BodyDef { dimensions: [1.0, 0.6, 1.0] },   // center
            BodyDef { dimensions: [1.0, 0.16, 0.5] },   // +X flipper
            BodyDef { dimensions: [1.0, 0.16, 0.5] },   // -X flipper
            BodyDef { dimensions: [1.0, 0.16, 0.5] },   // +Z flipper
            BodyDef { dimensions: [1.0, 0.16, 0.5] },   // -Z flipper
        ],
        joints: vec![
            JointDef {
                parent: 0, child: 1, joint_type: "revolute".into(),
                axis: [0.0, 0.0, 1.0], secondary_axis: [0.0; 3],
                parent_anchor: [0.5, 0.0, 0.0], child_anchor: [-0.5, 0.0, 0.0],
                angle_min: -0.8, angle_max: 0.8, damping: 0.3,
            },
            JointDef {
                parent: 0, child: 2, joint_type: "revolute".into(),
                axis: [0.0, 0.0, 1.0], secondary_axis: [0.0; 3],
                parent_anchor: [-0.5, 0.0, 0.0], child_anchor: [0.5, 0.0, 0.0],
                angle_min: -0.8, angle_max: 0.8, damping: 0.3,
            },
            JointDef {
                parent: 0, child: 3, joint_type: "revolute".into(),
                axis: [1.0, 0.0, 0.0], secondary_axis: [0.0; 3],
                parent_anchor: [0.0, 0.0, 0.5], child_anchor: [0.0, 0.0, -0.25],
                angle_min: -0.8, angle_max: 0.8, damping: 0.3,
            },
            JointDef {
                parent: 0, child: 4, joint_type: "revolute".into(),
                axis: [1.0, 0.0, 0.0], secondary_axis: [0.0; 3],
                parent_anchor: [0.0, 0.0, -0.5], child_anchor: [0.0, 0.0, 0.25],
                angle_min: -0.8, angle_max: 0.8, damping: 0.3,
            },
        ],
        torques: vec![
            TorqueDef { joint: 0, dof: 0, amplitude: 2.0, frequency: 3.0, phase: 0.0 },
            TorqueDef { joint: 1, dof: 0, amplitude: 2.0, frequency: 3.0, phase: std::f64::consts::PI },
            TorqueDef { joint: 2, dof: 0, amplitude: 2.0, frequency: 3.0, phase: std::f64::consts::FRAC_PI_2 },
            TorqueDef { joint: 3, dof: 0, amplitude: 2.0, frequency: 3.0, phase: 3.0 * std::f64::consts::FRAC_PI_2 },
        ],
    }
}

/// Swimming snake: elongated chain that undulates through water.
pub fn swimmer_snake() -> CreatureDefinition {
    let mut bodies = vec![BodyDef { dimensions: [0.4, 0.3, 0.3] }]; // head
    let mut joints = Vec::new();
    let mut torques = Vec::new();
    let segments = 5;

    for i in 0..segments {
        bodies.push(BodyDef { dimensions: [0.5, 0.2, 0.25] });
        joints.push(JointDef {
            parent: i, child: i + 1,
            joint_type: "revolute".into(),
            axis: [0.0, 1.0, 0.0], // undulate left-right
            secondary_axis: [0.0; 3],
            parent_anchor: [0.2, 0.0, 0.0],
            child_anchor: [-0.25, 0.0, 0.0],
            angle_min: -0.6, angle_max: 0.6, damping: 0.2,
        });
        torques.push(TorqueDef {
            joint: i, dof: 0,
            amplitude: 3.0, frequency: 2.0,
            phase: i as f64 * std::f64::consts::FRAC_PI_2,
        });
    }

    CreatureDefinition {
        name: "Swimming Snake".into(),
        bodies,
        joints,
        torques,
    }
}

/// Land inchworm: a chain of segments that undulates along the ground.
/// Low center of gravity, inherently stable, uses friction to inch forward.
pub fn walker_inchworm() -> CreatureDefinition {
    let mut bodies = vec![BodyDef { dimensions: [0.4, 0.2, 0.3] }]; // head
    let mut joints = Vec::new();
    let mut torques = Vec::new();
    let segments = 4;

    for i in 0..segments {
        bodies.push(BodyDef { dimensions: [0.35, 0.18, 0.28] });
        joints.push(JointDef {
            parent: i, child: i + 1,
            joint_type: "revolute".into(),
            axis: [0.0, 0.0, 1.0], // vertical undulation
            secondary_axis: [0.0; 3],
            parent_anchor: [-0.2, 0.0, 0.0],
            child_anchor: [0.175, 0.0, 0.0],
            angle_min: -0.4, angle_max: 0.4, damping: 0.8,
        });
        // Traveling wave: each segment has a phase offset
        torques.push(TorqueDef {
            joint: i, dof: 0,
            amplitude: 0.3, frequency: 0.5,
            phase: i as f64 * std::f64::consts::FRAC_PI_2,
        });
    }

    CreatureDefinition {
        name: "Land Inchworm".into(),
        bodies,
        joints,
        torques,
    }
}

/// Sprawling lizard: low, wide body with legs that push backward.
/// Stable because the center of gravity is low and the base is wide.
pub fn walker_lizard() -> CreatureDefinition {
    CreatureDefinition {
        name: "Sprawling Lizard".into(),
        bodies: vec![
            BodyDef { dimensions: [0.8, 0.2, 0.4] },  // torso (flat and wide)
            BodyDef { dimensions: [0.5, 0.12, 0.15] }, // front-left leg (flat paddle)
            BodyDef { dimensions: [0.5, 0.12, 0.15] }, // front-right leg
            BodyDef { dimensions: [0.5, 0.12, 0.15] }, // back-left leg
            BodyDef { dimensions: [0.5, 0.12, 0.15] }, // back-right leg
        ],
        joints: vec![
            // Front-left: leg extends to the side, rotates around Y to sweep forward/back
            JointDef {
                parent: 0, child: 1, joint_type: "revolute".into(),
                axis: [0.0, 1.0, 0.0], secondary_axis: [0.0; 3],
                parent_anchor: [0.3, -0.05, 0.2],
                child_anchor: [-0.05, 0.0, -0.075],
                angle_min: -0.6, angle_max: 0.6, damping: 0.8,
            },
            // Front-right
            JointDef {
                parent: 0, child: 2, joint_type: "revolute".into(),
                axis: [0.0, 1.0, 0.0], secondary_axis: [0.0; 3],
                parent_anchor: [0.3, -0.05, -0.2],
                child_anchor: [-0.05, 0.0, 0.075],
                angle_min: -0.6, angle_max: 0.6, damping: 0.8,
            },
            // Back-left
            JointDef {
                parent: 0, child: 3, joint_type: "revolute".into(),
                axis: [0.0, 1.0, 0.0], secondary_axis: [0.0; 3],
                parent_anchor: [-0.3, -0.05, 0.2],
                child_anchor: [0.05, 0.0, -0.075],
                angle_min: -0.6, angle_max: 0.6, damping: 0.8,
            },
            // Back-right
            JointDef {
                parent: 0, child: 4, joint_type: "revolute".into(),
                axis: [0.0, 1.0, 0.0], secondary_axis: [0.0; 3],
                parent_anchor: [-0.3, -0.05, -0.2],
                child_anchor: [0.05, 0.0, 0.075],
                angle_min: -0.6, angle_max: 0.6, damping: 0.8,
            },
        ],
        torques: vec![
            // Trot gait: diagonal pairs in phase, sweep legs forward/back
            TorqueDef { joint: 0, dof: 0, amplitude: 0.5, frequency: 0.8, phase: 0.0 },
            TorqueDef { joint: 1, dof: 0, amplitude: 0.5, frequency: 0.8, phase: std::f64::consts::PI },
            TorqueDef { joint: 2, dof: 0, amplitude: 0.5, frequency: 0.8, phase: std::f64::consts::PI },
            TorqueDef { joint: 3, dof: 0, amplitude: 0.5, frequency: 0.8, phase: 0.0 },
        ],
    }
}

/// Get a built-in creature by name.
pub fn builtin(name: &str) -> Option<CreatureDefinition> {
    match name {
        "swimmer-starfish" => Some(swimmer_starfish()),
        "swimmer-snake" => Some(swimmer_snake()),
        "walker-inchworm" => Some(walker_inchworm()),
        "walker-lizard" => Some(walker_lizard()),
        _ => None,
    }
}

/// List all built-in creature names.
pub fn builtin_names() -> &'static [&'static str] {
    &["swimmer-starfish", "swimmer-snake", "walker-inchworm", "walker-lizard"]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_creatures_build_successfully() {
        for name in builtin_names() {
            let def = builtin(name).unwrap();
            let world = def.build_world();
            assert!(!world.bodies.is_empty(), "{name}: no bodies");
            assert_eq!(world.bodies.len(), def.bodies.len(), "{name}: body count mismatch");
            assert_eq!(world.joints.len(), def.joints.len(), "{name}: joint count mismatch");
        }
    }

    #[test]
    fn swimmer_starfish_swims_in_water() {
        let def = swimmer_starfish();
        let records = simulate(&def, "water", 0.0, 120, 1.0 / 60.0);
        let start = records[0].positions[0];
        let end = records.last().unwrap().positions[0];
        let dist = ((end[0] - start[0]).powi(2) + (end[1] - start[1]).powi(2) + (end[2] - start[2]).powi(2)).sqrt();
        assert!(dist > 0.01, "Starfish should move in water, dist={dist}");
    }

    #[test]
    fn walker_inchworm_falls_under_gravity() {
        let def = walker_inchworm();
        let records = simulate(&def, "land", 9.81, 300, 1.0 / 60.0);

        // Find first NaN or explosion
        for r in &records {
            let ry = r.positions[0][1];
            if !ry.is_finite() || ry.abs() > 100.0 {
                eprintln!("DIVERGED at frame {}, time={:.3}s, root_y={:.4}", r.frame, r.time, ry);
                // Print surrounding frames
                let start = if r.frame > 5 { r.frame - 5 } else { 0 };
                for rr in &records[start..r.frame.min(records.len())] {
                    eprintln!("  frame {:3} t={:.3}: root=({:.4}, {:.4}, {:.4})",
                        rr.frame, rr.time, rr.positions[0][0], rr.positions[0][1], rr.positions[0][2]);
                }
                break;
            }
        }

        let start_y = records[0].positions[0][1];
        let end_y = records.last().unwrap().positions[0][1];
        assert!(
            end_y < start_y,
            "Inchworm should fall: start_y={start_y:.3}, end_y={end_y:.3}"
        );
        let min_y: f64 = records.iter().map(|r| r.positions[0][1]).fold(f64::INFINITY, f64::min);
        assert!(min_y > -1.0, "Inchworm should not fall through ground: min_y={min_y:.3}");
    }

    #[test]
    fn walker_lizard_stays_bounded() {
        let def = walker_lizard();
        let records = simulate(&def, "land", 9.81, 300, 1.0 / 60.0);
        for r in &records {
            for pos in &r.positions {
                let mag = (pos[0] * pos[0] + pos[1] * pos[1] + pos[2] * pos[2]).sqrt();
                assert!(
                    mag < 100.0,
                    "Body position exploded at frame {}: [{:.1}, {:.1}, {:.1}]",
                    r.frame, pos[0], pos[1], pos[2]
                );
            }
        }
    }

    #[test]
    fn json_roundtrip() {
        let def = swimmer_starfish();
        let json = serde_json::to_string_pretty(&def).unwrap();
        let parsed: CreatureDefinition = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, def.name);
        assert_eq!(parsed.bodies.len(), def.bodies.len());
        assert_eq!(parsed.joints.len(), def.joints.len());
        assert_eq!(parsed.torques.len(), def.torques.len());
    }
}

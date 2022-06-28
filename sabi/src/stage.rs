use std::time::Duration;

use bevy::core::Time;
use bevy::ecs::prelude::*;

/// This type will be available as a resource, while a fixed timestep stage
/// runs, to provide info about the current status of the fixed timestep.
///
/// If you modify the step value, the fixed timestep driver stage will
/// reconfigure itself to respect it. Your new timestep duration will be
/// used starting from the next update cycle.
pub struct NetworkSimulationInfo {
    pub step: Duration,
    pub accumulator: Duration,
}

impl NetworkSimulationInfo {
    /// The time duration of each timestep
    pub fn timestep(&self) -> Duration {
        self.step
    }
    /// The number of steps per second (Hz)
    pub fn rate(&self) -> f64 {
        1.0 / self.step.as_secs_f64()
    }
    /// The amount of time left over from the last timestep
    pub fn remaining(&self) -> Duration {
        self.accumulator
    }
    /// How much has the main game update "overstepped" the fixed timestep?
    /// (how many more (fractional) timesteps are left over in the accumulator)
    pub fn overstep(&self) -> f64 {
        self.accumulator.as_secs_f64() / self.step.as_secs_f64()
    }
}

/// A Stage that runs a number of child stages with a fixed timestep
///
/// You can set the timestep duration. Every frame update, the time delta
/// will be accumulated, and the child stages will run when it goes over
/// the timestep threshold. If multiple timesteps have been accumulated,
/// the child stages will be run multiple times.
///
/// You can add multiple child stages, allowing you to use `Commands` in
/// your fixed timestep systems, and have their effects applied.
///
/// A good place to add the `NetworkSimulationStage` is usually before
/// `CoreStage::Update`.
pub struct NetworkSimulationStage {
    step: Duration,
    accumulator: Duration,
    stages: Vec<Box<dyn Stage>>,
}

impl NetworkSimulationStage {
    /// Helper to create a `NetworkSimulationStage` with a single child stage
    pub fn from_stage<S: Stage>(timestep: Duration, stage: S) -> Self {
        Self::new(timestep).with_stage(stage)
    }

    /// Create a new empty `NetworkSimulationStage` with no child stages
    pub fn new(timestep: Duration) -> Self {
        Self {
            step: timestep,
            accumulator: Duration::default(),
            stages: Vec::new(),
        }
    }

    /// Add a child stage
    pub fn add_stage<S: Stage>(&mut self, stage: S) {
        self.stages.push(Box::new(stage));
    }

    /// Builder method for adding a child stage
    pub fn with_stage<S: Stage>(mut self, stage: S) -> Self {
        self.add_stage(stage);
        self
    }
}

impl Stage for NetworkSimulationStage {
    fn run(&mut self, world: &mut World) {
        self.accumulator += {
            let time = world.get_resource::<Time>();
            if let Some(time) = time {
                time.delta()
            } else {
                return;
            }
        };

        while self.accumulator >= self.step {
            self.accumulator -= self.step;
            for stage in self.stages.iter_mut() {
                world.insert_resource(NetworkSimulationInfo {
                    step: self.step,
                    accumulator: self.accumulator,
                });
                stage.run(world);
                if let Some(info) = world.remove_resource::<NetworkSimulationInfo>() {
                    // update our actual step duration, in case the user has
                    // modified it in the info resource
                    self.step = info.step;
                    self.accumulator = info.accumulator;
                }
            }
        }
    }
}

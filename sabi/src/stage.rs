use std::time::Duration;

use bevy::core::Time;
use bevy::ecs::prelude::*;
use bevy::ecs::schedule::IntoSystemDescriptor;
use bevy::prelude::*;

use crate::protocol::NetworkTick;

/// This type will be available as a resource, while a fixed timestep stage
/// runs, to provide info about the current status of the fixed timestep.
///
/// If you modify the step value, the fixed timestep driver stage will
/// reconfigure itself to respect it. Your new timestep duration will be
/// used starting from the next update cycle.
#[derive(Debug, Clone)]
pub struct NetworkSimulationInfo {
    pub step: Duration,
    pub accumulator: Duration,

    pub accel: bool,
    pub accel_step: Duration,
}

impl NetworkSimulationInfo {
    pub fn new(timestep: Duration) -> Self {
        Self {
            step: timestep,
            accumulator: Duration::default(),

            accel: true,
            accel_step: Duration::ZERO,
        }
    }
    /// The time duration of each timestep
    pub fn static_timestep(&self) -> Duration {
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

    pub fn accel(&mut self, percentage: f64) {
        self.accel = true;
        self.accel_step = self.step.mul_f64(percentage);
    }

    pub fn decel(&mut self, percentage: f64) {
        self.accel = false;
        self.accel_step = self.step.mul_f64(percentage);
    }

    pub fn timestep(&self) -> Duration {
        if self.accel {
            self.step.saturating_add(self.accel_step)
        } else {
            self.step.saturating_sub(self.accel_step)
        }
    }
}

#[derive(Debug, StageLabel, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NetworkStage;

#[derive(Debug, StageLabel, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NetworkCoreStage {
    First,
    PreUpdate,
    Update,
    PostUpdate,
    Last,
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
    pub info: NetworkSimulationInfo,
    /// Rewind the simulation back to the saved snapshot.
    pub rewind: SystemStage,
    /// Apply updates received from the server if any.
    pub apply_updates: SystemStage,
    /// Meta schedule, we want these to run on the timestep, but never replayed.
    pub meta: SystemStage,
    /// Game simulation that will be rewound.
    pub schedule: Schedule,
}

impl NetworkSimulationStage {
    /// Create a new empty `NetworkSimulationStage` with no child stages
    pub fn new(timestep: Duration) -> Self {
        Self {
            info: NetworkSimulationInfo::new(timestep),
            rewind: SystemStage::parallel(),
            apply_updates: SystemStage::parallel(),
            meta: SystemStage::parallel(),
            schedule: Schedule::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Rewind(pub NetworkTick);

impl Stage for NetworkSimulationStage {
    fn run(&mut self, world: &mut World) {
        if let Some(info) = world.get_resource::<NetworkSimulationInfo>() {
            if self.info.accel != info.accel || self.info.accel_step != info.accel_step {
                self.info.accel = info.accel;
                self.info.accel_step = info.accel_step;
            }
        }

        let mut catchup_frames = 0;
        let mut accumulated_frames = 0;

        self.info.accumulator += {
            let time = world.get_resource::<Time>();
            if let Some(time) = time {
                time.delta()
            } else {
                return;
            }
        };

        while self.info.accumulator >= self.info.timestep() {
            self.info.accumulator -= self.info.timestep();

            self.schedule.run(world);
            self.meta.run(world);
            accumulated_frames += 1;
        }

        // TODO: handle the edge case where we don't have a snapshot
        let current_tick = world
            .get_resource::<NetworkTick>()
            .expect("expected network tick")
            .clone();
        if let Some(rewind) = world.get_resource::<Rewind>() {
            let rewind_tick = rewind.0.clone();

            if rewind_tick.tick() < current_tick.tick() {
                world.insert_resource(rewind_tick);

                self.rewind.run(world);
                self.apply_updates.run(world);

                for tick in rewind_tick.tick()..current_tick.tick() {
                    self.schedule.run(world);
                    self.apply_updates.run(world);
                    catchup_frames += 1;
                }
            }

            let resimmed_current_tick = world
                .get_resource::<NetworkTick>()
                .expect("expected network tick")
                .clone();
            assert_eq!(current_tick.tick(), resimmed_current_tick.tick());

            world.remove_resource::<Rewind>();
        }
    }
}

pub trait NetworkSimulationAppExt {
    fn get_network_stage(&mut self) -> &mut NetworkSimulationStage;
    fn add_network_stage<S: Stage>(&mut self, label: impl StageLabel, stage: S) -> &mut Self;
    fn add_network_stage_after<S: Stage>(
        &mut self,
        target: impl StageLabel,
        label: impl StageLabel,
        stage: S,
    ) -> &mut Self;
    fn add_network_stage_before<S: Stage>(
        &mut self,
        target: impl StageLabel,
        label: impl StageLabel,
        stage: S,
    ) -> &mut Self;

    fn network_stage<T: Stage, F: FnOnce(&mut T) -> &mut T>(
        &mut self,
        label: impl StageLabel,
        func: F,
    ) -> &mut Self;

    fn add_network_system<Params>(
        &mut self,
        system: impl IntoSystemDescriptor<Params>,
    ) -> &mut Self;

    fn add_network_system_set(&mut self, system_set: SystemSet) -> &mut Self;

    fn add_system_to_network_stage<Params>(
        &mut self,
        stage_label: impl StageLabel,
        system: impl IntoSystemDescriptor<Params>,
    ) -> &mut Self;

    fn add_system_set_to_network_stage(
        &mut self,
        stage_label: impl StageLabel,
        system_set: SystemSet,
    ) -> &mut Self;

    fn add_rewind_network_system<Params>(
        &mut self,
        system: impl IntoSystemDescriptor<Params>,
    ) -> &mut Self;

    fn add_apply_update_network_system<Params>(
        &mut self,
        system: impl IntoSystemDescriptor<Params>,
    ) -> &mut Self;

    fn add_meta_network_system<Params>(
        &mut self,
        system: impl IntoSystemDescriptor<Params>,
    ) -> &mut Self;
}

impl NetworkSimulationAppExt for App {
    fn get_network_stage(&mut self) -> &mut NetworkSimulationStage {
        self.schedule
            .get_stage_mut(&NetworkStage)
            .expect("expected NetworkStage")
    }

    fn add_network_stage<S: Stage>(&mut self, label: impl StageLabel, stage: S) -> &mut Self {
        self.get_network_stage().schedule.add_stage(label, stage);
        self
    }

    fn add_network_stage_after<S: Stage>(
        &mut self,
        target: impl StageLabel,
        label: impl StageLabel,
        stage: S,
    ) -> &mut Self {
        self.get_network_stage()
            .schedule
            .add_stage_after(target, label, stage);
        self
    }

    fn add_network_stage_before<S: Stage>(
        &mut self,
        target: impl StageLabel,
        label: impl StageLabel,
        stage: S,
    ) -> &mut Self {
        self.get_network_stage()
            .schedule
            .add_stage_before(target, label, stage);
        self
    }

    fn network_stage<T: Stage, F: FnOnce(&mut T) -> &mut T>(
        &mut self,
        label: impl StageLabel,
        func: F,
    ) -> &mut Self {
        self.get_network_stage().schedule.stage(label, func);
        self
    }

    fn add_network_system<Params>(
        &mut self,
        system: impl IntoSystemDescriptor<Params>,
    ) -> &mut Self {
        self.add_system_to_network_stage(NetworkCoreStage::Update, system)
    }

    fn add_network_system_set(&mut self, system_set: SystemSet) -> &mut Self {
        self.add_system_set_to_network_stage(NetworkCoreStage::Update, system_set)
    }

    fn add_system_to_network_stage<Params>(
        &mut self,
        stage_label: impl StageLabel,
        system: impl IntoSystemDescriptor<Params>,
    ) -> &mut Self {
        self.get_network_stage()
            .schedule
            .add_system_to_stage(stage_label, system);
        self
    }

    fn add_system_set_to_network_stage(
        &mut self,
        stage_label: impl StageLabel,
        system_set: SystemSet,
    ) -> &mut Self {
        self.schedule
            .add_system_set_to_stage(stage_label, system_set);
        self
    }

    fn add_rewind_network_system<Params>(
        &mut self,
        system: impl IntoSystemDescriptor<Params>,
    ) -> &mut Self {
        self.get_network_stage().rewind.add_system(system);
        self
    }

    fn add_apply_update_network_system<Params>(
        &mut self,
        system: impl IntoSystemDescriptor<Params>,
    ) -> &mut Self {
        self.get_network_stage().apply_updates.add_system(system);
        self
    }

    fn add_meta_network_system<Params>(
        &mut self,
        system: impl IntoSystemDescriptor<Params>,
    ) -> &mut Self {
        self.get_network_stage().meta.add_system(system);
        self
    }
}

use crate::physics::FluidDynamics;

pub(crate) const DELTA_TIME: f64 = 0.05;

pub struct Simulation {
  hours: f64,
  landscape: Vec<f64>,
  running: bool,
  fast_forward: bool,
  delta_time: f64,
  time: f64,
  fluid_dynamics: FluidDynamics,
}

impl Simulation {
  pub fn new() -> Self {
    Self {
      hours: 0.0,
      landscape: vec![],
      running: false,
      fast_forward: false,
      delta_time: DELTA_TIME,
      time: 0.0,
      fluid_dynamics: FluidDynamics::default(),
    }
  }

  pub fn start(&mut self, landscape: &[f64], hours: f64) {
    self.hours = hours;
    self.landscape = Vec::from(landscape);
    self.running = true;
    self.fast_forward = false;
    self.time = 0.0;
    self.fluid_dynamics.set_density(landscape);
  }

  pub fn pause(&mut self) {
    self.running = false;
    self.fast_forward = false;
  }

  pub fn resume(&mut self) {
    self.running = !self.is_finished();
    self.fast_forward = false;
  }

  pub fn step(&mut self) {
    let remaining_time = (self.hours - self.time).clamp(0.0, self.hours);
    let delta_time = f64::min(self.delta_time, remaining_time);
    self.time += delta_time;
    self.fluid_dynamics.add_density(delta_time);
    self.fluid_dynamics.step(delta_time);
    self.running = !self.is_finished();
  }

  pub fn start_forward(&mut self) {
    self.fast_forward = !self.is_finished();
    self.running = !self.is_finished();
  }

  pub fn forward(&mut self, hours: f64) {
    let start = self.get_time();
    while !self.is_finished() && self.get_time() - start < hours {
      self.step();
    }
  }

  #[inline]
  pub fn is_running(&self) -> bool {
    self.running
  }

  #[inline]
  pub fn is_fast_forward(&self) -> bool {
    self.fast_forward
  }

  #[inline]
  pub fn is_finished(&self) -> bool {
    self.time >= self.hours
  }

  #[inline]
  pub fn get_time(&self) -> f64 {
    self.time
  }

  #[inline]
  pub fn get_levels(&self) -> &[f64] {
    self.fluid_dynamics.get_density()
  }
}

#[cfg(test)]
pub mod tests {
  use core::panic;

  use assert_approx_eq::assert_approx_eq;

  use super::*;

  #[test]
  fn simulation_new() {
    let sim = Simulation::new();
    assert_approx_eq!(sim.hours, 0.0);
    assert_eq!(sim.landscape, Vec::<f64>::new());
    assert_approx_eq!(sim.delta_time, DELTA_TIME);
    assert_approx_eq!(sim.get_time(), 0.0);
    assert!(!sim.is_running());
    assert!(!sim.is_fast_forward());
  }

  #[test]
  fn simulation_start() {
    let mut sim = Simulation::new();
    sim.start(&[1.0, 2.0, 3.0, 4.0], 4.5);

    assert_approx_eq!(sim.hours, 4.5);
    assert_eq!(sim.landscape, vec![1.0, 2.0, 3.0, 4.0]);
    assert_approx_eq!(sim.delta_time, DELTA_TIME);
    assert_approx_eq!(sim.get_time(), 0.0);
    assert!(sim.is_running());
    assert!(!sim.is_fast_forward());
    assert_slice_approx_eq(sim.get_levels(), &[1.0, 2.0, 3.0, 4.0]);
  }

  #[test]
  fn simulation_pause() {
    let mut sim = Simulation::new();
    sim.running = true;
    sim.fast_forward = true;

    sim.pause();

    assert!(!sim.is_running());
    assert!(!sim.is_fast_forward());
  }

  #[test]
  fn simulation_resume_when_not_finished() {
    let mut sim = Simulation::new();
    sim.hours = 2.0;
    sim.running = false;
    sim.fast_forward = true;

    sim.resume();

    assert!(sim.is_running());
    assert!(!sim.is_fast_forward());
  }

  #[test]
  fn simulation_resume_when_finished() {
    let mut sim = Simulation::new();
    sim.running = false;
    sim.fast_forward = true;

    sim.resume();

    assert!(!sim.is_running());
    assert!(!sim.is_fast_forward());
  }

  #[test]
  fn simulation_step_adds_rain() {
    let mut sim = Simulation::new();
    sim.start(&[1.0, 1.0], 1.0);

    sim.step();

    assert_slice_approx_eq(sim.get_levels(), &[1.0 + DELTA_TIME, 1.0 + DELTA_TIME]);
  }

  #[test]
  fn simulation_step_diffuses_densities() {
    let mut sim = Simulation::new();
    sim.start(&[1.0, 8.0], 1.0);

    sim.step();

    let levels = sim.get_levels();
    assert!(levels[0] > 1.0);
    assert!(levels[1] < 8.0);
  }

  #[test]
  fn simulation_step_continues_running() {
    let mut sim = Simulation::new();
    sim.start(&[1.0, 1.0], 4.0);

    sim.step();

    assert!(sim.is_running());
    assert_approx_eq!(sim.get_time(), DELTA_TIME);
  }

  #[test]
  fn simulation_step_finishes() {
    let mut sim = Simulation::new();
    sim.start(&[1.0, 1.0], DELTA_TIME);

    sim.step();

    assert!(!sim.is_running());
    assert!(sim.is_finished());
    assert_approx_eq!(sim.get_time(), DELTA_TIME);
  }

  #[test]
  fn simulation_step_when_not_started() {
    let mut sim = Simulation::new();
    sim.hours = 4.0;
    sim.step();

    assert!(sim.is_running());
    assert_approx_eq!(sim.get_time(), DELTA_TIME);
    assert_slice_approx_eq(sim.get_levels(), &[]);
  }

  #[test]
  fn simulation_step_when_time_over_hours() {
    let mut sim = Simulation::new();
    sim.time = 1.5;
    sim.hours = 1.0;
    sim.step();

    assert!(!sim.is_running());
    assert_approx_eq!(sim.get_time(), 1.5);
    assert_slice_approx_eq(sim.get_levels(), &[]);
  }

  #[test]
  fn simulation_start_forward_when_not_finished() {
    let mut sim = Simulation::new();
    sim.hours = 4.0;
    sim.running = false;
    sim.fast_forward = false;

    sim.start_forward();

    assert!(sim.is_running());
    assert!(sim.is_fast_forward());
  }

  #[test]
  fn simulation_start_forward_when_finished() {
    let mut sim = Simulation::new();
    sim.time = 4.0;
    sim.hours = 4.0;
    sim.running = false;
    sim.fast_forward = false;

    sim.start_forward();

    assert!(!sim.is_running());
    assert!(!sim.is_fast_forward());
    assert_approx_eq!(sim.get_time(), 4.0);
  }

  #[test]
  fn simulation_forward_diffuses_densities() {
    let mut sim = Simulation::new();
    sim.start(&[1.0, 8.0], 1.0);

    sim.step();

    let levels = sim.get_levels();
    assert!(levels[0] > 1.0);
    assert!(levels[1] < 8.0);
  }

  #[test]
  fn simulation_forward_continues_running() {
    let mut sim = Simulation::new();
    sim.start(&[1.0, 1.0], 4.0);

    sim.forward(2.0);

    assert!(sim.is_running());
    assert_approx_eq!(sim.get_time(), 2.0);
  }

  #[test]
  fn simulation_forward_finishes() {
    let mut sim = Simulation::new();
    sim.start(&[1.0, 1.0], 4.0);

    sim.forward(4.0);

    assert_approx_eq!(sim.get_time(), 4.0);
    assert!(!sim.is_running());
    assert!(sim.is_finished());
  }

  #[test]
  fn simulation_forward_stops_running_when_finished() {
    let mut sim = Simulation::new();
    sim.start(&[1.0, 1.0], 4.0);

    sim.forward(6.0);

    assert_approx_eq!(sim.get_time(), 4.0);
    assert!(!sim.is_running());
    assert!(sim.is_finished());
  }

  pub fn assert_slice_approx_eq(a: &[f64], b: &[f64]) {
    assert_eq!(a.len(), b.len());
    let result = std::panic::catch_unwind(|| {
      a.iter()
        .zip(b.iter())
        .for_each(|(v1, v2)| assert_approx_eq!(*v1, *v2));
    });
    if result.is_err() {
      panic!("Different approx slices:\nleft:  {:?}\nright: {:?}", a, b);
    }
  }

  pub fn assert_slice_approx_eq_with_epsilon(a: &[f64], b: &[f64], epsilon: f64) {
    let result = std::panic::catch_unwind(|| {
      a.iter()
        .zip(b.iter())
        .for_each(|(v1, v2)| assert_approx_eq!(*v1, *v2, epsilon));
    });
    if result.is_err() {
      panic!(
        "Different approx slices with epsilon {}:\nleft:  {:?}\nright: {:?}",
        epsilon, a, b
      );
    }
  }
}

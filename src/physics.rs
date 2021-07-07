const DEFAULT_DIFFUSION: f64 = 0.4;

pub struct FluidDynamics {
  size: usize,
  density: Vec<f64>,
  density0: Vec<f64>,
  diffusion: f64,
}

impl Default for FluidDynamics {
  fn default() -> Self {
    Self {
      size: 0,
      density: vec![0.0; 2],
      density0: vec![0.0; 2],
      diffusion: DEFAULT_DIFFUSION,
    }
  }
}

impl FluidDynamics {
  pub fn set_density(&mut self, density: &[f64]) {
    self.size = density.len();
    let grid_size = density.len() + 2;
    self.density = vec![0.0; grid_size];
    self.density[1..=self.size]
      .iter_mut()
      .zip(density)
      .for_each(|(prev_value, new_value)| *prev_value = *new_value);
    self.density0 = vec![0.0; grid_size];
  }

  pub fn get_density(&self) -> &[f64] {
    &self.density[1..=self.size]
  }

  pub fn add_density(&mut self, value: f64) {
    self
      .density
      .iter_mut()
      .for_each(|prev_value| *prev_value += value)
  }

  pub fn step(&mut self, delta_time: f64) {
    Self::diffuse(
      self.size,
      self.density0.as_mut_slice(),
      self.density.as_slice(),
      self.diffusion,
      delta_time,
    );

    self.swap_density_buffers();
  }

  fn diffuse(size: usize, x: &mut [f64], x0: &[f64], diffusion: f64, delta_time: f64) {
    let a = delta_time * diffusion * size as f64;
    for _ in 0..20 {
      for i in 1..=size {
        x[i] = (x0[i] + a * (x[i - 1] + x[i + 1])) / (1.0 + 2.0 * a);
      }
      Self::set_boundaries(size, x);
    }
  }

  fn set_boundaries(size: usize, x: &mut [f64]) {
    x[0] = x[1];
    x[size + 1] = x[size];
  }

  fn swap_density_buffers(&mut self) {
    std::mem::swap(&mut self.density, &mut self.density0);
  }
}

#[cfg(test)]
mod tests {
  use assert_approx_eq::assert_approx_eq;

  use super::*;
  use crate::simulation::tests::{assert_slice_approx_eq, assert_slice_approx_eq_with_epsilon};

  #[test]
  fn fluid_dynamics_default() {
    let fluids = FluidDynamics::default();

    assert_eq!(fluids.size, 0);
    assert_slice_approx_eq(fluids.density.as_slice(), &[0.0; 2]);
    assert_approx_eq!(fluids.diffusion, DEFAULT_DIFFUSION);
  }

  #[test]
  fn fluid_dynamics_set_density() {
    let mut fluids = FluidDynamics::default();

    fluids.set_density(vec![3.0, 1.0, 6.0, 4.0, 8.0, 9.0].as_slice());

    assert_eq!(fluids.size, 6);
    assert_slice_approx_eq(
      fluids.density.as_slice(),
      &[0.0, 3.0, 1.0, 6.0, 4.0, 8.0, 9.0, 0.0],
    );
  }

  #[test]
  fn fluid_dynamics_get_density() {
    let mut fluids = FluidDynamics::default();

    fluids.set_density(vec![3.0, 1.0, 6.0, 4.0, 8.0, 9.0].as_slice());

    assert_slice_approx_eq(fluids.get_density(), &[3.0, 1.0, 6.0, 4.0, 8.0, 9.0]);
  }

  #[test]
  fn fluid_dynamics_add_density() {
    let mut fluids = FluidDynamics::default();
    fluids.set_density(vec![3.0, 1.0, 6.0, 4.0, 8.0, 9.0].as_slice());

    fluids.add_density(1.0);

    assert_slice_approx_eq(fluids.get_density(), &[4.0, 2.0, 7.0, 5.0, 9.0, 10.0]);
  }

  #[test]
  fn fluid_dynamics_step() {
    let mut fluids = FluidDynamics::default();
    fluids.set_density(vec![3.0, 1.0, 6.0, 4.0, 8.0, 9.0].as_slice());

    fluids.step(0.05);

    let total_density: f64 = fluids.get_density().iter().cloned().sum();

    assert_approx_eq!(total_density, 31.0, 0.1);

    assert_slice_approx_eq_with_epsilon(fluids.get_density(), &[2.8, 1.6, 5.4, 4.5, 7.7, 8.8], 0.1);
  }
}

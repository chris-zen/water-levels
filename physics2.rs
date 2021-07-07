
const DEFAULT_DIFFUSION: f64 = 0.4;

pub struct FluidDynamics {
  size: usize,
  density: Vec<f64>,
  velocity: Vec<f64>,
  diffusion: f64,
}

impl Default for FluidDynamics {
  fn default() -> Self {
    Self {
      size: 0,
      density: vec![0.0; 2],
      velocity: vec![0.0; 2],
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
    self.velocity = vec![0.0; grid_size];
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
    let mut density0 = vec![0.0; self.density.len()];

    Self::density_step(
      self.size,
      self.density.as_mut_slice(),
      density0.as_mut_slice(),
      self.diffusion,
      delta_time,
    );
  }

  fn density_step(
    size: usize,
    x: &mut [f64],
    x0: &mut [f64],
    diff: f64,
    delta_time: f64,
  ) {
    Self::add_source(x, x0, delta_time);
    // Self::printvec("add_src:", x);
    Self::diffuse(size, Boundary::Zero, x0, x, diff, delta_time);
    // Self::printvec("diffuse:", x0);
    Self::advect(size, Boundary::Zero, x, x0, v, delta_time);
    // Self::printvec("advect: ", x0);
  }

  fn add_source(x: &mut [f64], s: &[f64], delta_time: f64) {
    x.iter_mut()
      .zip(s.iter())
      .for_each(|(xi, si)| *xi += *si * delta_time);
  }

  fn diffuse(
    size: usize,
    boundary: Boundary,
    x: &mut [f64],
    x0: &[f64],
    diff: f64,
    delta_time: f64,
  ) {
    let a = delta_time * diff * size as f64;
    for _ in 0..20 {
      for i in 1..=size {
        x[i] = (x0[i] + a * (x[i - 1] + x[i + 1])) / (1.0 + 2.0 * a);
      }
      Self::set_boundaries(size, boundary, x);
    }
  }

  fn advect(
    size: usize,
    boundary: Boundary,
    d: &mut [f64],
    d0: &[f64],
    v: &[f64],
    delta_time: f64,
  ) {
    let delta_time0 = delta_time * size as f64;
    for i in 1..=size {
      let x = (i as f64 - delta_time0 * v[i]).clamp(0.5, size as f64 + 0.5);
      let i0 = x as usize;
      let i1 = i0 + 1;
      let s1 = x - i0 as f64;
      let s0 = 1.0 - s1;
      d[i] = s0 * d0[i0] + s1 * d0[i1];
    }
    Self::set_boundaries(size, boundary, d);
  }

  fn set_boundaries(size: usize, boundary: Boundary, x: &mut [f64]) {
    let (x_left, x_right) = match boundary {
      Boundary::Two => (-x[1], x[size]),
      _ => (x[1], x[size]),
    };
    x[0] = x_left;
    x[size + 1] = x_right;
  }

  fn printvec(title: &str, vec: &[f64]) {
    let d: Vec<String> = vec.iter().map(|v| format!("{:02.1}", v)).collect();
    println!("{} {}", title, d.join(", "))
  }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Boundary {
  Zero,
  One,
  Two,
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
    assert_slice_approx_eq(fluids.density.as_slice(), &[0.0, 3.0, 1.0, 6.0, 4.0, 8.0, 9.0, 0.0]);
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

    assert_slice_approx_eq_with_epsilon(
      fluids.get_density(),
      &[2.8, 1.6, 5.4, 4.5, 7.7, 8.8],
    0.1);
  }

  fn fmtvec(vec: &[f64]) -> String {
    let d: Vec<String> = vec
      .iter()
      .skip(1)
      .take(vec.len() - 2)
      .map(|v| format!("{:02.1}", v))
      .collect();
    d.join(", ")
  }
}

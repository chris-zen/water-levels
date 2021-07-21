type SegmentLevel = u32;

/// This allows to identify the different types of areas that can be derived from the analysis of a level
#[derive(Debug, PartialEq)]
enum Area {
  /// This represents the infinite walls in the extremes
  Boundary,
  /// This represents a depression in the terrain
  Sink {
    start: usize,
    end: usize,
    bottom: SegmentLevel,
  },
  /// This represents a plain surface that will spill water to the adjacent sinks
  Plain {
    start: usize,
    length: usize,
    sinks: usize,
  },
}

impl Area {
  pub fn width(&self) -> f64 {
    match self {
      Area::Boundary => 0.0,
      Area::Plain { length, sinks, .. } => *length as f64 / *sinks as f64,
      Area::Sink { start, end, .. } => (*end - *start + 1) as f64,
    }
  }
}

/// A Sink represents a depression in a fragment of the terrain [start, end] for a certain level range [bottom, top)
///
/// When analyzing the landscape, the information about the different levels of the terrain is translated into a tree hierarchy of Sinks.
/// Sinks can have children representing the water contained in the underlying sinks (under the bottom level).
/// Leaf Sinks represent water above a plain of terrain that does not connect with any other underlying sinks.
///
#[derive(Debug, PartialEq)]
struct Sink {
  weight: f64,
  start: usize,
  end: usize,
  top: SegmentLevel,
  bottom: SegmentLevel,
  capacity: f64,
  total_capacity: f64,
  water: f64,
  children: Vec<Sink>,
}

impl Sink {
  pub fn new(
    weight: f64,
    start: usize,
    end: usize,
    top: SegmentLevel,
    bottom: SegmentLevel,
    children: Vec<Sink>,
  ) -> Sink {
    let width = (end - start + 1) as f64;
    let capacity = width * (top - bottom) as f64;
    let children_capacity = children
      .iter()
      .map(|child| child.total_capacity)
      .fold(0.0, |accum, child_total_capacity| {
        accum + child_total_capacity
      });
    let total_capacity = capacity + children_capacity;

    Sink {
      weight,
      start,
      end,
      top,
      bottom,
      capacity,
      total_capacity,
      water: 0.0,
      children,
    }
  }

  #[inline]
  pub fn is_full(&self) -> bool {
    self.water >= self.capacity
  }

  #[inline]
  pub fn width(&self) -> f64 {
    (self.end - self.start + 1) as f64
  }

  #[inline]
  pub fn total_water(&self) -> f64 {
    self.water
      + self
        .children
        .iter()
        .map(|child| child.total_water())
        .fold(0.0, |accum, water| accum + water)
  }
}

/// This simulates the flow of the water coming from the rain through a landscape
#[derive(Debug)]
pub struct WaterFlow {
  landscape: Vec<SegmentLevel>,
  water: Vec<f64>,
  root_sink: Option<Sink>,
}

impl WaterFlow {
  /// It builds the hierarchy of sinks for a landscape and returns a WaterFlow instance
  pub fn new(landscape: Vec<SegmentLevel>) -> WaterFlow {
    let water = vec![0.0; landscape.len()];

    let root_sink = (!landscape.is_empty()).then(|| {
      let end = landscape.len() - 1;
      let bottom = landscape.iter().cloned().max().unwrap_or(0);
      let children = Self::build_sinks_hierarchy(landscape.as_slice(), 0, end, bottom);

      Sink::new(1.0, 0, end, SegmentLevel::MAX, bottom, children)
    });

    WaterFlow {
      landscape,
      water,
      root_sink,
    }
  }

  /// It build the hierarchy of sinks for a region of the landscape under a certain segment level
  fn build_sinks_hierarchy(
    landscape: &[SegmentLevel],
    start: usize,
    end: usize,
    level: SegmentLevel,
  ) -> Vec<Sink> {
    let mut sinks = Vec::<Sink>::with_capacity((end - start + 3) / 2);

    let areas = Self::scan_areas(landscape, start, end, level);

    let total_width = (end - start + 1) as f64;

    let mut total_weight = 0.0;
    for index in 1..areas.len() - 1 {
      if let Area::Sink { start, end, bottom } = &areas[index] {
        let weight = Self::calculate_sink_weight(areas.as_slice(), index, total_width);
        total_weight += weight;
        let children = Self::build_sinks_hierarchy(landscape, *start, *end, *bottom);
        let sink = Sink::new(weight, *start, *end, level, *bottom, children);
        sinks.push(sink);
      }
    }

    // In case there are floating point errors that we need to compensate for
    if !sinks.is_empty() && total_weight < 1.0 {
      sinks[0].weight += 1.0 - total_weight;
    }

    sinks
  }

  /// Scan the areas of a landscape for a given region and up to a certain level
  /// The areas will include information about boundaries, plains and sinks.
  fn scan_areas(
    landscape: &[SegmentLevel],
    start: usize,
    end: usize,
    level: SegmentLevel,
  ) -> Vec<Area> {
    let mut areas = Vec::<Area>::with_capacity(landscape.len());
    areas.push(Area::Boundary);

    let mut index = start;
    while index <= end {
      let area = if landscape[index] == level {
        Self::scan_plain(&landscape, &mut index, end, level)
      } else {
        Self::scan_sink(&landscape, &mut index, end, level)
      };
      Self::push_area(&mut areas, area);
    }

    areas.push(Area::Boundary);
    areas
  }

  /// Scan information about a plain (contiguous segments with the same level)
  fn scan_plain(
    landscape: &[SegmentLevel],
    index: &mut usize,
    end: usize,
    level: SegmentLevel,
  ) -> Area {
    let start = *index;
    while *index <= end && landscape[*index] == level {
      *index += 1;
    }
    let length = *index - start;

    if length == 0 {
      Area::Boundary
    } else {
      Area::Plain {
        start,
        length,
        sinks: 0,
      }
    }
  }

  /// Scan information about a sink (a depression in the landscape)
  fn scan_sink(
    landscape: &[SegmentLevel],
    index: &mut usize,
    end: usize,
    level: SegmentLevel,
  ) -> Area {
    let start = *index;
    let mut bottom = 0;
    while *index <= end && landscape[*index] < level {
      bottom = bottom.max(landscape[*index]);
      *index += 1;
    }

    if start == *index {
      Area::Boundary
    } else {
      Area::Sink {
        start,
        end: *index - 1,
        bottom,
      }
    }
  }

  /// Push a new scanned area to a list of areas
  fn push_area(areas: &mut Vec<Area>, area: Area) {
    assert!(!areas.is_empty());
    let last_area = areas.last_mut().unwrap();

    match (last_area, &area) {
      (_, Area::Boundary) => (),

      (Area::Boundary, _) => areas.push(area),

      (Area::Plain { sinks, .. }, Area::Sink { .. }) => {
        *sinks += 1;
        areas.push(area);
      }

      (Area::Sink { .. }, Area::Plain { start, length, .. }) => {
        areas.push(Area::Plain {
          start: *start,
          length: *length,
          sinks: 1,
        });
      }

      _ => (),
    };
  }

  /// Calculate the proportion of water that will flow through the sink from the rain respect to the total landscape width,
  /// which comes from the sink region itself plus half the region of the contiguous plains.
  fn calculate_sink_weight(areas: &[Area], index: usize, total_width: f64) -> f64 {
    let left_width = areas[index - 1].width();
    let right_width = areas[index + 1].width();
    let width = areas[index].width() + left_width + right_width;
    width / total_width
  }

  /// Return the total levels of the segments including terrain plus water levels
  pub fn total_levels(&self) -> Vec<f64> {
    self
      .landscape
      .iter()
      .zip(self.water.iter())
      .map(|(segment_level, water_level)| *segment_level as f64 + *water_level)
      .collect()
  }

  /// Simulate the flow of water for some hours of rain
  /// This operation is not accumulative and will update the internal state according to this simulation.
  pub fn rain(&mut self, hours: f64) {
    if let Some(sink) = self.root_sink.as_mut() {
      let total_water = self.landscape.len() as f64 * hours;
      Self::fill_sink_with_water(self.landscape.as_slice(), sink, total_water);

      self.water.fill(0.0);
      Self::flood_water_to_landscape(self.landscape.as_slice(), self.water.as_mut_slice(), sink);
    }
  }

  /// Calculate the flow of certain amount of water through the sinks hierarchy
  fn fill_sink_with_water(landscape: &[SegmentLevel], sink: &mut Sink, amount: f64) -> f64 {
    let num_children = sink.children.len();
    let mut excess = vec![0.0; num_children];

    let (mut children_amount, total_excess) =
      Self::fill_downstream_sinks_with_water(landscape, sink, amount, excess.as_mut_slice());

    children_amount +=
      Self::spill_excess_water_through_sinks(landscape, &mut excess, total_excess, sink);

    let remaining = amount - children_amount;
    let sink_amount = f64::min(sink.capacity - sink.water, remaining);
    sink.water += sink_amount;

    children_amount + sink_amount
  }

  /// Push water downstream through the hierarchy of sinks
  fn fill_downstream_sinks_with_water(
    landscape: &[SegmentLevel],
    sink: &mut Sink,
    amount: f64,
    excess: &mut [f64],
  ) -> (f64, f64) {
    let mut total_filled = 0.0;
    let mut total_excess = 0.0;

    // We need to compensate for possible floating point errors
    let total_quota = sink
      .children
      .iter()
      .map(|child| amount * child.weight)
      .fold(0.0, |acc, quota| acc + quota);
    let mut quota_error = amount - total_quota;

    for (child, sink_excess) in sink.children.iter_mut().zip(excess.iter_mut()) {
      if !child.is_full() {
        let quota = amount * child.weight + quota_error;
        quota_error = 0.0;
        let filled = Self::fill_sink_with_water(landscape, child, quota);
        *sink_excess = quota - filled;
        total_excess += *sink_excess;
        total_filled += filled;
      }
    }

    (total_filled, total_excess)
  }

  /// Try to spill excess water from the downstream sinks into contiguous sinks,
  /// and finally add the remaining excess to the parent sink.
  fn spill_excess_water_through_sinks(
    landscape: &[SegmentLevel],
    excess: &mut [f64],
    total_excess: f64,
    sink: &mut Sink,
  ) -> f64 {
    let mut total_spilled = 0.0;
    if total_excess > 0.0 && sink.children.len() > 1 {
      for (index, sink_excess) in excess.iter_mut().enumerate() {
        if *sink_excess > 0.0 {
          let children = sink.children.as_slice();
          let left_capacity = Self::find_spill_capacity(children, index, -1);
          let right_capacity = Self::find_spill_capacity(children, index, 1);
          if left_capacity + right_capacity > 0.0 {
            let (left_water, right_water) =
              Self::spilled_amount(*sink_excess, left_capacity, right_capacity);
            let children = sink.children.as_mut_slice();
            let left_spilled =
              Self::spill_water(landscape, children, index as isize, -1, left_water);
            let right_spilled =
              Self::spill_water(landscape, children, index as isize, 1, right_water);
            let spilled = left_spilled + right_spilled;
            *sink_excess -= spilled;
            total_spilled += spilled;
          }
        }
      }
    }
    total_spilled
  }

  /// Before we can spill excess water to both sides of a sink,
  /// we need to know the total capacity available in the contiguous sinks
  fn find_spill_capacity(sinks: &[Sink], index: usize, direction: isize) -> f64 {
    let mut index = index as isize;
    let mut capacity = 0.0;
    index += direction;
    while index >= 0 && (index as usize) < sinks.len() {
      let sink = &sinks[index as usize];
      capacity += sink.total_capacity - sink.total_water();
      index += direction;
    }
    capacity
  }

  /// To calculate the right amount of water that will spill in each direction
  /// we calculate proportions from the available capacities
  /// and treat them as a 2D vector that can be normalized.
  fn spilled_amount(sink_excess: f64, left_capacity: f64, right_capacity: f64) -> (f64, f64) {
    let left_proportion = f64::min(sink_excess, left_capacity) / sink_excess;
    let right_proportion = f64::min(sink_excess, right_capacity) / sink_excess;
    let modulo = f64::sqrt(left_proportion * left_proportion + right_proportion * right_proportion);
    let left_water = sink_excess * left_proportion / modulo;
    let right_water = sink_excess * right_proportion / modulo;
    (left_water, right_water)
  }

  /// Spill a certain amount of water towards the contiguous sinks in a certain direction
  fn spill_water(
    landscape: &[SegmentLevel],
    sinks: &mut [Sink],
    index: isize,
    direction: isize,
    mut amount: f64,
  ) -> f64 {
    let mut total_spilled = 0.0;
    let mut index = index as isize;
    index += direction;
    while amount > 0.0 && index >= 0 && (index as usize) < sinks.len() {
      let sink = &mut sinks[index as usize];
      if sink.total_capacity - sink.total_water() > 0.0 {
        let spill_amount = if sink.children.is_empty() {
          Self::fill_sink_with_water(landscape, sink, amount)
        } else {
          let index = if direction == -1 {
            sink.children.len() as isize
          } else {
            -1
          };
          let children_amount =
            Self::spill_water(landscape, &mut sink.children, index, direction, amount);
          Self::fill_sink_with_water(landscape, sink, amount - children_amount) + children_amount
        };
        total_spilled += spill_amount;
        amount -= spill_amount;
      }
      index += direction;
    }
    total_spilled
  }

  /// Once all the sinks have been filled with water we need to flood that water into the segments of the landscape.
  /// We do it recursively from the leafs towards the upper sinks.
  fn flood_water_to_landscape(terrain: &[SegmentLevel], water: &mut [f64], sink: &mut Sink) {
    for child in sink.children.iter_mut() {
      Self::flood_water_to_landscape(terrain, water, child);
    }

    if sink.water > 0.0 {
      let segment_amount = sink.water / sink.width();

      let mut remaining = sink.water;
      sink.water = 0.0;

      let mut lower_level = f64::MAX;
      let mut lower_offset = 0usize;
      let range = sink.start..=sink.end;
      let segments = water[range.clone()]
        .iter_mut()
        .zip(terrain[range].iter())
        .enumerate();
      for (offset, (water_level, terrain_level)) in segments {
        *water_level += segment_amount;
        remaining -= segment_amount;
        let level = *terrain_level as f64 + *water_level;
        if level < lower_level {
          lower_level = level;
          lower_offset = offset;
        }
      }

      // Check for f64 rounding errors and flood the remaining water
      // into the segment with the lower level. That's important to
      // conserve the total amount of water constant.
      if remaining > 0.0 {
        water[sink.start + lower_offset] += remaining;
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use assert_approx_eq::assert_approx_eq;
  use rand::thread_rng;
  use rand::Rng;

  use super::*;
  use crate::simulation::tests::assert_slice_approx_eq_with_epsilon;

  #[test]
  fn water_flow_new_with_empty_terrain() {
    let water_flow = WaterFlow::new(vec![]);
    assert!(water_flow.total_levels().is_empty())
  }

  #[test]
  fn water_flow_new_initializes_landscape_and_water_levels() {
    let water_flow = WaterFlow::new(vec![6, 4, 5, 9, 9, 2, 6, 5, 9, 7]);

    assert_eq!(water_flow.landscape, vec![6, 4, 5, 9, 9, 2, 6, 5, 9, 7]);
    assert!(water_flow.water.iter().all(|value| *value == 0.0))
  }

  #[test]
  fn water_flow_new_builds_the_hierarchy_of_sinks() {
    let water_flow = WaterFlow::new(vec![6, 4, 5, 9, 9, 2, 6, 5, 9, 7]);

    assert_eq!(
      water_flow.root_sink,
      Some(Sink {
        weight: 1.0,
        start: 0,
        end: 9,
        top: 4294967295,
        bottom: 9,
        capacity: 42949672860.0,
        total_capacity: 42949672888.0,
        water: 0.0,
        children: vec![
          Sink {
            weight: 0.4,
            start: 0,
            end: 2,
            top: 9,
            bottom: 6,
            capacity: 9.0,
            total_capacity: 12.0,
            water: 0.0,
            children: vec![Sink {
              weight: 1.0,
              start: 1,
              end: 2,
              top: 6,
              bottom: 5,
              capacity: 2.0,
              total_capacity: 3.0,
              water: 0.0,
              children: vec![Sink {
                weight: 1.0,
                start: 1,
                end: 1,
                top: 5,
                bottom: 4,
                capacity: 1.0,
                total_capacity: 1.0,
                water: 0.0,
                children: vec![],
              },],
            },],
          },
          Sink {
            weight: 0.45,
            start: 5,
            end: 7,
            top: 9,
            bottom: 6,
            capacity: 9.0,
            total_capacity: 14.0,
            water: 0.0,
            children: vec![
              Sink {
                weight: 0.5,
                start: 5,
                end: 5,
                top: 6,
                bottom: 2,
                capacity: 4.0,
                total_capacity: 4.0,
                water: 0.0,
                children: vec![],
              },
              Sink {
                weight: 0.5,
                start: 7,
                end: 7,
                top: 6,
                bottom: 5,
                capacity: 1.0,
                total_capacity: 1.0,
                water: 0.0,
                children: vec![],
              },
            ],
          },
          Sink {
            weight: 0.15,
            start: 9,
            end: 9,
            top: 9,
            bottom: 7,
            capacity: 2.0,
            total_capacity: 2.0,
            water: 0.0,
            children: vec![],
          },
        ],
      })
    )
  }

  #[test]
  fn water_flow_rain_fill_simple_hierarchy() {
    let mut water_flow = WaterFlow::new(vec![6, 4, 5, 9]);

    water_flow.rain(4.0);

    assert_slice_approx_eq_with_epsilon(water_flow.water.as_slice(), &[4.0, 6.0, 5.0, 1.0], 0.1);
  }

  #[test]
  fn water_flow_rain_fill_and_spill_binary_hierarchy() {
    let mut water_flow = WaterFlow::new(vec![2, 6, 5, 9]);

    water_flow.rain(2.0);

    assert_slice_approx_eq_with_epsilon(water_flow.water.as_slice(), &[5.0, 1.0, 2.0, 0.0], 0.1);
  }

  #[test]
  fn water_flow_rain_spill_equally_to_the_sides() {
    let mut water_flow = WaterFlow::new(vec![1, 4, 4, 3, 4, 4, 1]);

    water_flow.rain(1.0);

    assert_slice_approx_eq_with_epsilon(
      water_flow.water.as_slice(),
      &[3.0, 0.0, 0.0, 1.0, 0.0, 0.0, 3.0],
      0.1,
    );
  }

  #[test]
  fn water_flow_rain_spill_with_recursion() {
    let mut water_flow = WaterFlow::new(vec![4, 1, 4, 6, 5]);

    water_flow.rain(2.0);

    assert_slice_approx_eq_with_epsilon(
      water_flow.water.as_slice(),
      &[2.0, 5.0, 2.0, 0.0, 1.0],
      0.1,
    );
  }

  #[test]
  fn water_flow_rain_spill_with_recursion_and_fill_up() {
    let mut water_flow = WaterFlow::new(vec![4, 7, 5, 8, 6, 9, 7]);

    water_flow.rain(2.0);

    assert_slice_approx_eq_with_epsilon(
      water_flow.water.as_slice(),
      &[4.4, 1.4, 3.4, 0.4, 2.4, 0.0, 2.0],
      0.1,
    );
  }

  #[test]
  fn water_flow_rain_total_volume_is_conserved_within_an_error_interval() {
    let mut rng = thread_rng();
    for _ in 0..1000 {
      let size = rng.gen_range(1..100);
      let mut landscape = Vec::with_capacity(size);
      for _ in 0..size {
        landscape.push(rng.gen_range(0..20))
      }

      let mut water_flow = WaterFlow::new(landscape);

      let hours = rng.gen_range(1..10) as f64;
      water_flow.rain(hours);

      let volume = water_flow.water.iter().fold(0.0, |acc, &x| acc + x);
      let expected_volume = size as f64 * hours;
      assert_approx_eq!(volume, expected_volume, expected_volume * 0.07);
    }
  }
}

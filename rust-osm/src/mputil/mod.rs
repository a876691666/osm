//! Multi-polygon utility functions
//!
//! This module provides utilities for building and manipulating
//! multipolygon geometries from OSM data.

use geo_types::{Coord, LineString};

/// Orientation indicates the winding direction of a ring
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Orientation {
    #[default]
    None,
    /// Counter-clockwise (positive area)
    CCW,
    /// Clockwise (negative area)
    CW,
}

impl Orientation {
    /// Returns the opposite orientation
    pub fn reverse(self) -> Self {
        match self {
            Orientation::CCW => Orientation::CW,
            Orientation::CW => Orientation::CCW,
            Orientation::None => Orientation::None,
        }
    }
}

/// Segment is a section of a multipolygon with extra information
#[derive(Debug, Clone)]
pub struct Segment {
    pub index: u32,
    pub orientation: Orientation,
    pub reversed: bool,
    pub line: LineString<f64>,
}

impl Segment {
    /// Creates a new segment
    pub fn new(line: LineString<f64>) -> Self {
        Segment {
            index: 0,
            orientation: Orientation::None,
            reversed: false,
            line,
        }
    }

    /// Creates a new segment with orientation
    pub fn with_orientation(line: LineString<f64>, orientation: Orientation) -> Self {
        Segment {
            index: 0,
            orientation,
            reversed: false,
            line,
        }
    }

    /// Reverses the line string of the segment
    pub fn reverse(&mut self) {
        self.reversed = !self.reversed;
        let mut coords: Vec<Coord<f64>> = self.line.0.clone();
        coords.reverse();
        self.line = LineString::new(coords);
    }

    /// Returns the first point in the segment
    pub fn first(&self) -> Option<Coord<f64>> {
        self.line.0.first().copied()
    }

    /// Returns the last point in the segment
    pub fn last(&self) -> Option<Coord<f64>> {
        self.line.0.last().copied()
    }
}

/// MultiSegment is an ordered set of segments that form a continuous
/// section of a multipolygon
#[derive(Debug, Clone, Default)]
pub struct MultiSegment(pub Vec<Segment>);

impl MultiSegment {
    /// Creates a new empty MultiSegment
    pub fn new() -> Self {
        MultiSegment(Vec::new())
    }

    /// Returns the first point in the list of linestrings
    pub fn first(&self) -> Option<Coord<f64>> {
        self.0.first().and_then(|s| s.first())
    }

    /// Returns the last point in the list of linestrings
    pub fn last(&self) -> Option<Coord<f64>> {
        self.0.last().and_then(|s| s.last())
    }

    /// Converts a multisegment into a LineString
    pub fn line_string(&self) -> LineString<f64> {
        let length: usize = self.0.iter().map(|s| s.line.0.len()).sum();
        let mut coords = Vec::with_capacity(length);

        for s in &self.0 {
            coords.extend(s.line.0.iter().copied());
        }

        LineString::new(coords)
    }

    /// Converts the multisegment to a ring with the given orientation
    pub fn ring(&self, target_orientation: Orientation) -> Vec<Coord<f64>> {
        let length: usize = self.0.iter().map(|s| s.line.0.len()).sum();
        let mut ring = Vec::with_capacity(length);

        let mut have_orient = false;
        let mut should_reverse = false;

        for s in &self.0 {
            if s.orientation != Orientation::None {
                have_orient = true;
                if (s.orientation == target_orientation) == s.reversed {
                    should_reverse = true;
                }
            }
            ring.extend(s.line.0.iter().copied());
        }

        let ring_orientation = compute_orientation(&ring);

        if (have_orient && should_reverse)
            || (!have_orient && ring_orientation != target_orientation)
        {
            ring.reverse();
        }

        ring
    }

    /// Computes the orientation of a multisegment like if it was a ring
    pub fn orientation(&self) -> Orientation {
        let first = match self.first() {
            Some(p) => p,
            None => return Orientation::None,
        };

        let offset = first;
        let mut area = 0.0;
        let mut prev = first;

        for segment in &self.0 {
            for point in &segment.line.0 {
                area += (prev.x - offset.x) * (point.y - offset.y)
                    - (point.x - offset.x) * (prev.y - offset.y);
                prev = *point;
            }
        }

        if area > 0.0 {
            Orientation::CCW
        } else {
            Orientation::CW
        }
    }

    /// Returns true if the multisegment forms a closed ring
    pub fn is_closed(&self) -> bool {
        match (self.first(), self.last()) {
            (Some(first), Some(last)) => coords_equal(first, last),
            _ => false,
        }
    }

    /// Returns the number of segments
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns true if there are no segments
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Join will join a set of segments into a set of connected MultiSegments
pub fn join(mut segments: Vec<Segment>) -> Vec<MultiSegment> {
    let mut lists = Vec::new();
    segments = compact(segments);

    while let Some(seg) = segments.pop() {
        let mut current = MultiSegment(vec![seg]);

        // Keep adding segments until we form a ring or can't find more matches
        while !segments.is_empty() && !current.is_closed() {
            let first = match current.first() {
                Some(p) => p,
                None => break,
            };
            let last = match current.last() {
                Some(p) => p,
                None => break,
            };

            let mut found_at = None;

            for (i, segment) in segments.iter_mut().enumerate() {
                let seg_first = segment.first();
                let seg_last = segment.last();

                if let (Some(sf), Some(sl)) = (seg_first, seg_last) {
                    if coords_equal(last, sf) {
                        // Nice fit at the end of current
                        let mut seg = segment.clone();
                        if !seg.line.0.is_empty() {
                            seg.line.0.remove(0);
                        }
                        current.0.push(seg);
                        found_at = Some(i);
                        break;
                    } else if coords_equal(last, sl) {
                        // Reverse it and it'll fit at the end
                        segment.reverse();
                        let mut seg = segment.clone();
                        if !seg.line.0.is_empty() {
                            seg.line.0.remove(0);
                        }
                        current.0.push(seg);
                        found_at = Some(i);
                        break;
                    } else if coords_equal(first, sl) {
                        // Nice fit at the start of current
                        let mut seg = segment.clone();
                        if !seg.line.0.is_empty() {
                            seg.line.0.pop();
                        }
                        current.0.insert(0, seg);
                        found_at = Some(i);
                        break;
                    } else if coords_equal(first, sf) {
                        // Reverse it and it'll fit at the start
                        segment.reverse();
                        let mut seg = segment.clone();
                        if !seg.line.0.is_empty() {
                            seg.line.0.pop();
                        }
                        current.0.insert(0, seg);
                        found_at = Some(i);
                        break;
                    }
                }
            }

            match found_at {
                Some(i) => {
                    segments.remove(i);
                }
                None => break, // Invalid geometry (dangling way, unclosed ring)
            }
        }

        lists.push(current);
    }

    lists
}

/// Removes segments with less than 2 points
fn compact(ms: Vec<Segment>) -> Vec<Segment> {
    ms.into_iter().filter(|s| s.line.0.len() > 1).collect()
}

/// Computes the orientation of a ring
pub fn compute_orientation(ring: &[Coord<f64>]) -> Orientation {
    if ring.len() < 3 {
        return Orientation::None;
    }

    let offset = ring[0];
    let mut area = 0.0;
    let mut prev = ring[0];

    for point in ring.iter().skip(1) {
        area += (prev.x - offset.x) * (point.y - offset.y)
            - (point.x - offset.x) * (prev.y - offset.y);
        prev = *point;
    }

    if area > 0.0 {
        Orientation::CCW
    } else {
        Orientation::CW
    }
}

/// Checks if two coordinates are equal (within floating point tolerance)
fn coords_equal(a: Coord<f64>, b: Coord<f64>) -> bool {
    const EPSILON: f64 = 1e-9;
    (a.x - b.x).abs() < EPSILON && (a.y - b.y).abs() < EPSILON
}

/// Checks if a ring contains a point using ray casting
pub fn ring_contains_point(ring: &[Coord<f64>], point: Coord<f64>) -> bool {
    let x = point.x;
    let y = point.y;
    let mut inside = false;
    let mut j = ring.len() - 1;

    for i in 0..ring.len() {
        let xi = ring[i].x;
        let yi = ring[i].y;
        let xj = ring[j].x;
        let yj = ring[j].y;

        if ((yi > y) != (yj > y)) && (x < (xj - xi) * (y - yi) / (yj - yi) + xi) {
            inside = !inside;
        }

        j = i;
    }

    inside
}

/// Checks if an outer ring contains any point of an inner ring
pub fn polygon_contains(outer: &[Coord<f64>], inner: &[Coord<f64>]) -> bool {
    for point in inner {
        if ring_contains_point(outer, *point) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo_types::Coord;

    #[test]
    fn test_orientation_ccw() {
        // Counter-clockwise square
        let ring = vec![
            Coord { x: 0.0, y: 0.0 },
            Coord { x: 1.0, y: 0.0 },
            Coord { x: 1.0, y: 1.0 },
            Coord { x: 0.0, y: 1.0 },
            Coord { x: 0.0, y: 0.0 },
        ];
        assert_eq!(compute_orientation(&ring), Orientation::CCW);
    }

    #[test]
    fn test_orientation_cw() {
        // Clockwise square
        let ring = vec![
            Coord { x: 0.0, y: 0.0 },
            Coord { x: 0.0, y: 1.0 },
            Coord { x: 1.0, y: 1.0 },
            Coord { x: 1.0, y: 0.0 },
            Coord { x: 0.0, y: 0.0 },
        ];
        assert_eq!(compute_orientation(&ring), Orientation::CW);
    }

    #[test]
    fn test_ring_contains_point() {
        let ring = vec![
            Coord { x: 0.0, y: 0.0 },
            Coord { x: 10.0, y: 0.0 },
            Coord { x: 10.0, y: 10.0 },
            Coord { x: 0.0, y: 10.0 },
            Coord { x: 0.0, y: 0.0 },
        ];

        assert!(ring_contains_point(&ring, Coord { x: 5.0, y: 5.0 }));
        assert!(!ring_contains_point(&ring, Coord { x: 15.0, y: 5.0 }));
    }

    #[test]
    fn test_join_simple() {
        let seg1 = Segment::new(LineString::new(vec![
            Coord { x: 0.0, y: 0.0 },
            Coord { x: 1.0, y: 0.0 },
        ]));
        let seg2 = Segment::new(LineString::new(vec![
            Coord { x: 1.0, y: 0.0 },
            Coord { x: 1.0, y: 1.0 },
        ]));

        let result = join(vec![seg1, seg2]);
        assert_eq!(result.len(), 1);
        assert!(result[0].0.len() >= 1);
    }

    #[test]
    fn test_segment_reverse() {
        let mut seg = Segment::new(LineString::new(vec![
            Coord { x: 0.0, y: 0.0 },
            Coord { x: 1.0, y: 1.0 },
        ]));

        seg.reverse();

        assert!(seg.reversed);
        assert_eq!(seg.first(), Some(Coord { x: 1.0, y: 1.0 }));
        assert_eq!(seg.last(), Some(Coord { x: 0.0, y: 0.0 }));
    }
}

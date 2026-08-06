use std::f64::consts::PI;

use geo_types::*;

pub trait TileCover {
    fn tile_cover(&self, zoom: u8) -> Vec<(i32, i32, u8)>;
}

impl<T: CoordFloat> TileCover for Point<T> {
    /// Get a list of all tiles covering a given geometry
    fn tile_cover(&self, zoom: u8) -> Vec<(i32, i32, u8)> {
        vec![coord_to_tile(self.0, zoom)]
    }
}

impl<T: CoordFloat> TileCover for MultiPoint<T> {
    fn tile_cover(&self, zoom: u8) -> Vec<(i32, i32, u8)> {
        let mut tiles: Vec<_> = self
            .0
            .iter()
            .map(|point| coord_to_tile(point.0, zoom))
            .collect();

        tiles.sort();
        tiles.dedup();

        tiles
    }
}

impl<T: CoordFloat> TileCover for LineString<T> {
    fn tile_cover(&self, zoom: u8) -> Vec<(i32, i32, u8)> {
        let mut tiles: Vec<(i32, i32, u8)> = Vec::new();

        line_cover(&mut tiles, self, zoom, None);

        tiles.sort();
        tiles.dedup();

        tiles
    }
}

impl<T: CoordFloat> TileCover for Line<T> {
    fn tile_cover(&self, zoom: u8) -> Vec<(i32, i32, u8)> {
        let mut tiles: Vec<(i32, i32, u8)> = Vec::new();

        let linestring: LineString<T> = self.into();
        line_cover(&mut tiles, &linestring, zoom, None);

        tiles.sort();
        tiles.dedup();

        tiles
    }
}

impl<T: CoordFloat> TileCover for MultiLineString<T> {
    fn tile_cover(&self, zoom: u8) -> Vec<(i32, i32, u8)> {
        let mut tiles: Vec<(i32, i32, u8)> = Vec::new();

        for linestring in self.iter() {
            line_cover(&mut tiles, linestring, zoom, None);
        }

        tiles.sort();
        tiles.dedup();

        tiles
    }
}

impl<T: CoordFloat> TileCover for Polygon<T> {
    fn tile_cover(&self, zoom: u8) -> Vec<(i32, i32, u8)> {
        let mut tiles: Vec<(i32, i32, u8)> = Vec::new();

        poly_cover(&mut tiles, self, zoom);

        tiles.sort();
        tiles.dedup();

        tiles
    }
}

impl<T: CoordFloat> TileCover for MultiPolygon<T> {
    fn tile_cover(&self, zoom: u8) -> Vec<(i32, i32, u8)> {
        let mut tiles: Vec<(i32, i32, u8)> = Vec::new();

        for polygon in self.iter() {
            poly_cover(&mut tiles, polygon, zoom);
        }

        tiles.sort();
        tiles.dedup();

        tiles
    }
}

impl<T: CoordFloat> TileCover for Rect<T> {
    fn tile_cover(&self, zoom: u8) -> Vec<(i32, i32, u8)> {
        let min_tile = coord_to_tile(self.min(), zoom);
        let max_tile = coord_to_tile(self.max(), zoom);

        let tile_count: usize =
            ((max_tile.0 - min_tile.0 + 1) * (min_tile.1 - max_tile.1 + 1)) as usize;
        let mut tiles: Vec<(i32, i32, u8)> = Vec::with_capacity(tile_count);

        for x in min_tile.0..=max_tile.0 {
            for y in max_tile.1..=min_tile.1 {
                tiles.push((x, y, zoom));
            }
        }

        tiles
    }
}

impl<T: CoordFloat> TileCover for Triangle<T> {
    fn tile_cover(&self, zoom: u8) -> Vec<(i32, i32, u8)> {
        let mut tiles: Vec<(i32, i32, u8)> = Vec::new();

        let polygon: Polygon<T> = (*self).into();
        poly_cover(&mut tiles, &polygon, zoom);

        tiles.sort();
        tiles.dedup();

        tiles
    }
}

impl<T: CoordFloat> TileCover for GeometryCollection<T> {
    fn tile_cover(&self, zoom: u8) -> Vec<(i32, i32, u8)> {
        let mut tiles: Vec<(i32, i32, u8)> = Vec::new();

        for geometry in self.iter() {
            tiles.extend(geometry.tile_cover(zoom).into_iter());
        }

        tiles.sort();
        tiles.dedup();

        tiles
    }
}

impl<T: CoordFloat> TileCover for Geometry<T> {
    fn tile_cover(&self, zoom: u8) -> Vec<(i32, i32, u8)> {
        match self {
            &Geometry::Point(ref point) => point.tile_cover(zoom),
            &Geometry::MultiPoint(ref multipoint) => multipoint.tile_cover(zoom),
            &Geometry::Line(ref line) => line.tile_cover(zoom),
            &Geometry::LineString(ref linestring) => linestring.tile_cover(zoom),
            &Geometry::MultiLineString(ref multilinestring) => multilinestring.tile_cover(zoom),
            &Geometry::Polygon(ref polygon) => polygon.tile_cover(zoom),
            &Geometry::MultiPolygon(ref multipolygon) => multipolygon.tile_cover(zoom),
            &Geometry::GeometryCollection(ref gc) => gc.tile_cover(zoom),
            &Geometry::Rect(ref rect) => rect.tile_cover(zoom),
            &Geometry::Triangle(ref triangle) => triangle.tile_cover(zoom),
        }
    }
}

pub fn poly_cover<T: CoordFloat>(tiles: &mut Vec<(i32, i32, u8)>, polygon: &Polygon<T>, zoom: u8) {
    let mut intersections: Vec<(i32, i32)> = Vec::new();

    poly_cover_single(&mut intersections, tiles, &polygon.exterior(), zoom);

    for interior in polygon.interiors() {
        poly_cover_single(&mut intersections, tiles, &interior, zoom);
    }

    // sort by y, then x
    intersections.sort_by_key(|a| (a.1, a.0));

    let mut int_it = 0;
    while int_it < intersections.len() {
        // fill tiles between pairs of intersections
        let y = intersections[int_it].1;

        let mut x = intersections[int_it].0 + 1;
        while x < intersections[int_it + 1].0 {
            tiles.push((x, y, zoom));

            x = x + 1;
        }

        int_it = int_it + 2;
    }
}

fn poly_cover_single<T: CoordFloat>(
    intersections: &mut Vec<(i32, i32)>,
    tiles: &mut Vec<(i32, i32, u8)>,
    linestring: &LineString<T>,
    zoom: u8,
) {
    let mut ring: Vec<(i32, i32)> = Vec::new();

    line_cover(tiles, &linestring, zoom, Some(&mut ring));

    if ring.len() > 0 {
        let mut j = 0;
        let len = ring.len();
        let mut k = len - 1;

        while j < len {
            let m = (j + 1) % len;
            let y = ring[j].1;

            //Add Intersection if it's not local extrenum or Duplicate
            //      Not Local Mim                               Not Local Max
            if (y > ring[k].1 || y > ring[m].1)
                && (y < ring[k].1 || y < ring[m].1)
                && y != ring[m].1
            {
                intersections.push(ring[j]);
            }

            k = j;
            j = j + 1;
        }
    }
}

pub fn line_cover<T: CoordFloat>(
    tiles: &mut Vec<(i32, i32, u8)>,
    linestring: &LineString<T>,
    zoom: u8,
    mut ring: Option<&mut Vec<(i32, i32)>>,
) {
    let mut prev_x: Option<T> = None;
    let mut prev_y: Option<T> = None;
    let mut x: T;
    let mut y: T = T::zero();

    for line in linestring.lines() {
        let start = coord_to_tile_fraction(line.start, zoom);
        let stop = coord_to_tile_fraction(line.end, zoom);

        let x0 = start.0;
        let y0 = start.1;

        let x1 = stop.0;
        let y1 = stop.1;

        let dx = x1 - x0;
        let dy = y1 - y0;

        if dy == T::zero() && dx == T::zero() {
            continue;
        }

        let sx = if dx > T::zero() { T::one() } else { -T::one() };
        let sy = if dy > T::zero() { T::one() } else { -T::one() };

        x = x0.floor();
        y = y0.floor();

        let mut t_max_x = if dx == T::zero() {
            T::infinity()
        } else {
            (((if dx > T::zero() { T::one() } else { T::zero() }) + x - x0) / dx).abs()
        };

        let mut t_max_y = if dy == T::zero() {
            T::infinity()
        } else {
            (((if dy > T::zero() { T::one() } else { T::zero() }) + y - y0) / dy).abs()
        };

        let tdx = (sx / dx).abs();
        let tdy = (sy / dy).abs();

        if Some(x) != prev_x || Some(y) != prev_y {
            tiles.push((to_i32(x), to_i32(y), zoom));

            if ring != None && Some(y) != prev_y {
                match ring {
                    Some(ref mut r) => r.push((to_i32(x), to_i32(y))),
                    _ => (),
                };
            }

            prev_x = Some(x);
            prev_y = Some(y);
        }

        while t_max_x < T::one() || t_max_y < T::one() {
            if t_max_x < t_max_y {
                t_max_x = t_max_x + tdx;
                x = x + sx;
            } else {
                t_max_y = t_max_y + tdy;
                y = y + sy;
            }

            tiles.push((to_i32(x), to_i32(y), zoom));

            if ring != None && Some(y) != prev_y {
                match ring {
                    Some(ref mut r) => r.push((to_i32(x), to_i32(y))),
                    _ => (),
                };
            }
            prev_x = Some(x);
            prev_y = Some(y);
        }
    }

    if ring != None {
        match ring {
            Some(ref mut r) => {
                if r.len() > 0 && to_i32(y) == r[0].1 {
                    r.pop();
                }
            }
            _ => (),
        }
    }
}

pub fn get_children(tile: (i32, i32, u8)) -> Vec<(i32, i32, u8)> {
    vec![
        (tile.0 * 2, tile.1 * 2, tile.2 + 1),
        (tile.0 * 2 + 1, tile.1 * 2, tile.2 + 1),
        (tile.0 * 2 + 1, tile.1 * 2 + 1, tile.2 + 1),
        (tile.0 * 2, tile.1 * 2 + 1, tile.2 + 1),
    ]
}

pub fn get_parent(tile: (i32, i32, u8)) -> (i32, i32, u8) {
    (tile.0 >> 1, tile.1 >> 1, tile.2 - 1)
}

pub fn get_siblings(tile: (i32, i32, u8)) -> Vec<(i32, i32, u8)> {
    get_children(get_parent(tile))
}

/// Get the bounds of a tile, returned as a Rect<T>
pub fn tile_to_bbox<T: CoordFloat>(tile: (i32, i32, u8)) -> Rect<T> {
    Rect::new(
        coord! { x: tile_to_lon(tile.0, tile.2), y: tile_to_lat(tile.1 + 1, tile.2) },
        coord! { x: tile_to_lon(tile.0 + 1, tile.2), y: tile_to_lat(tile.1, tile.2) },
    )
}

/// Get the longitudinal value for a given tile corner
pub fn tile_to_lon<T: CoordFloat>(x: i32, z: u8) -> T {
    let two = T::one() + T::one();
    T::from(x).unwrap() / two.powi(z as i32) * T::from(360.0).unwrap() - T::from(180.0).unwrap()
}

/// Get the latitudinal value for a given tile corner
pub fn tile_to_lat<T: CoordFloat>(y: i32, z: u8) -> T {
    let two = T::one() + T::one();
    let pi = T::from(PI).unwrap();
    let n = pi - two * pi * T::from(y).unwrap() / two.powi(z as i32);
    (T::from(0.5).unwrap() * (n.exp() - (-n).exp()))
        .atan()
        .to_degrees()
}

/// Get the tile for a point at a specified zoom level
pub fn coord_to_tile<T: CoordFloat>(coord: Coord<T>, z: u8) -> (i32, i32, u8) {
    let tile_frac = coord_to_tile_fraction(coord, z);

    (
        to_i32(tile_frac.0.floor()),
        to_i32(tile_frac.1.floor()),
        tile_frac.2,
    )
}

/// Get the precise fractional tile location for a point at a zoom level
pub fn coord_to_tile_fraction<T: CoordFloat>(coord: Coord<T>, z: u8) -> (T, T, u8) {
    let sin = coord.y.to_radians().sin();
    let base: T = T::one() + T::one();

    let z2: T = base.powf(T::from(z).unwrap());
    let mut x = z2 * (coord.x / T::from(360.0).unwrap() + T::from(0.5).unwrap());
    let y = z2
        * (T::from(0.5).unwrap()
            - T::from(0.25).unwrap() * ((T::one() + sin) / (T::one() - sin)).ln()
                / T::from(PI).unwrap());

    // Wrap Tile X
    x = x % z2;
    if x < T::zero() {
        x = x + z2
    }

    (x, y, z)
}

fn to_i32<T: CoordFloat>(value: T) -> i32 {
    value.to_i32().unwrap_or_else(|| {
        if value.is_sign_negative() {
            i32::MIN
        } else {
            i32::MAX
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point() {
        let point = Point::new(-77.15664982795715, 38.87419791355846);
        assert_eq!(point.tile_cover(1), vec![(0, 0, 1)]);
        assert_eq!(point.tile_cover(2), vec![(1, 1, 2)]);
        assert_eq!(point.tile_cover(3), vec![(2, 3, 3)]);
        assert_eq!(point.tile_cover(4), vec![(4, 6, 4)]);

        let point = Point::new(-79.37969952821732, 38.8328422301817);
        assert_eq!(point.tile_cover(14), vec![(4579, 6271, 14)]);
    }

    #[test]
    fn test_points() {
        let points: MultiPoint<f64> = vec![
            (-84.48486328124999, 43.40504748787035),
            (-90.87890625, 39.90973623453719),
            (-84.55078125, 43.45291889355468),
            (-90.8349609375, 39.93711893299021),
        ]
        .into();
        assert_eq!(points.tile_cover(1), vec![(0, 0, 1)]);
        assert_eq!(points.tile_cover(2), vec![(0, 1, 2), (1, 1, 2)]);
        assert_eq!(points.tile_cover(3), vec![(1, 3, 3), (2, 2, 3)]);
        assert_eq!(points.tile_cover(4), vec![(3, 6, 4), (4, 5, 4)]);

        let points: MultiPoint<f64> = vec![(-79.37969952821732, 38.8328422301817)].into();

        assert_eq!(points.tile_cover(14), vec![(4579, 6271, 14)]);
    }

    #[test]
    fn test_line() {
        let line = LineString(vec![
            Coord {
                x: -106.21719360351562,
                y: 28.592359801121567,
            },
            Coord {
                x: -106.1004638671875,
                y: 28.791130513231813,
            },
            Coord {
                x: -105.87661743164062,
                y: 28.864519767126602,
            },
            Coord {
                x: -105.82374572753905,
                y: 28.60743139267596,
            },
        ]);

        assert_eq!(
            line.tile_cover(12),
            vec![
                (839, 1707, 12),
                (839, 1708, 12),
                (840, 1705, 12),
                (840, 1706, 12),
                (840, 1707, 12),
                (841, 1705, 12),
                (842, 1704, 12),
                (842, 1705, 12),
                (843, 1704, 12),
                (843, 1705, 12),
                (843, 1706, 12),
                (843, 1707, 12),
                (843, 1708, 12)
            ]
        )
    }

    #[test]
    fn test_line_2() {
        let line = LineString(vec![
            Coord {
                x: -79.37619924545288,
                y: 38.8345346107744,
            },
            Coord {
                x: -79.37287330627441,
                y: 38.83762675779815,
            },
            Coord {
                x: -79.37230467796326,
                y: 38.83820338656929,
            },
            Coord {
                x: -79.37211155891418,
                y: 38.83878001066818,
            },
        ]);

        assert_eq!(line.tile_cover(14), vec![(4579, 6271, 14),])
    }

    #[test]
    fn test_edge_line() {
        let line = LineString(vec![
            Coord {
                x: -80.160384,
                y: 32.766901,
            },
            Coord {
                x: -80.160216,
                y: 32.766845,
            },
            Coord {
                x: -80.159659,
                y: 32.766722,
            },
            Coord {
                x: -80.159356,
                y: 32.766633,
            },
            Coord {
                x: -80.159196,
                y: 32.766586,
            },
            Coord {
                x: -80.159096,
                y: 32.766571,
            },
            Coord {
                x: -80.159016,
                y: 32.766569,
            },
            Coord {
                x: -80.158947,
                y: 32.766581,
            },
            Coord {
                x: -80.158637,
                y: 32.766668,
            },
            Coord {
                x: -80.158527,
                y: 32.766691,
            },
            Coord {
                x: -80.158433,
                y: 32.766697,
            },
            Coord {
                x: -80.158367,
                y: 32.76669,
            },
            Coord {
                x: -80.158116,
                y: 32.766641,
            },
            Coord {
                x: -80.157565,
                y: 32.766507,
            },
            Coord {
                x: -80.157183,
                y: 32.766389,
            },
            Coord {
                x: -80.156946,
                y: 32.76633,
            },
            Coord {
                x: -80.156748,
                y: 32.766298,
            },
            Coord {
                x: -80.156657,
                y: 32.766279,
            },
            Coord {
                x: -80.156492,
                y: 32.766253,
            },
            Coord {
                x: -80.15626,
                y: 32.766181,
            },
            Coord {
                x: -80.156216,
                y: 32.766155,
            },
            Coord {
                x: -80.156166,
                y: 32.766118,
            },
            Coord {
                x: -80.156148,
                y: 32.7661,
            },
            Coord {
                x: -80.156125,
                y: 32.766052,
            },
            Coord {
                x: -80.156122,
                y: 32.766012,
            },
            Coord {
                x: -80.156131,
                y: 32.765974,
            },
            Coord {
                x: -80.156179,
                y: 32.765905,
            },
            Coord {
                x: -80.156198,
                y: 32.765856,
            },
            Coord {
                x: -80.15621,
                y: 32.765807,
            },
            Coord {
                x: -80.15625,
                y: 32.76548,
            },
            Coord {
                x: -80.156249,
                y: 32.765323,
            },
            Coord {
                x: -80.156235,
                y: 32.765284,
            },
            Coord {
                x: -80.156215,
                y: 32.765256,
            },
            Coord {
                x: -80.156181,
                y: 32.765226,
            },
        ]);

        assert_eq!(
            line.tile_cover(14),
            vec![(4543, 6612, 14), (4544, 6612, 14)]
        )
    }

    #[test]
    fn test_multiline() {
        let line = MultiLineString(vec![
            LineString(vec![
                Coord {
                    x: 11.3818359375,
                    y: 51.15178610143037,
                },
                Coord {
                    x: 7.998046875,
                    y: 50.0077390146369,
                },
                Coord {
                    x: 10.458984375,
                    y: 49.18170338770663,
                },
                Coord {
                    x: 5.2734375,
                    y: 46.6795944656402,
                },
            ]),
            LineString(vec![
                Coord {
                    x: 0.263671875,
                    y: 49.15296965617042,
                },
                Coord {
                    x: 3.076171875,
                    y: 50.0077390146369,
                },
                Coord {
                    x: 3.6474609374999996,
                    y: 48.60385760823255,
                },
                Coord {
                    x: 4.7900390625,
                    y: 49.095452162534826,
                },
                Coord {
                    x: 6.328125,
                    y: 48.48748647988415,
                },
                Coord {
                    x: 10.1513671875,
                    y: 48.07807894349862,
                },
                Coord {
                    x: 12.392578125,
                    y: 46.46813299215554,
                },
            ]),
        ]);

        assert_eq!(
            line.tile_cover(8),
            vec![
                (128, 87, 8),
                (129, 86, 8),
                (129, 87, 8),
                (130, 86, 8),
                (130, 87, 8),
                (130, 88, 8),
                (131, 87, 8),
                (131, 88, 8),
                (131, 90, 8),
                (132, 88, 8),
                (132, 89, 8),
                (132, 90, 8),
                (133, 86, 8),
                (133, 88, 8),
                (133, 89, 8),
                (134, 86, 8),
                (134, 87, 8),
                (134, 88, 8),
                (135, 85, 8),
                (135, 86, 8),
                (135, 87, 8),
                (135, 88, 8),
                (135, 89, 8),
                (136, 85, 8),
                (136, 89, 8),
                (136, 90, 8)
            ]
        );
    }

    #[test]
    fn test_polygon() {
        let poly = Polygon::new(
            LineString(vec![
                Coord {
                    x: 5.11962890625,
                    y: 20.46818922264095,
                },
                Coord {
                    x: 5.11962890625,
                    y: 20.7663868125152,
                },
                Coord {
                    x: 5.504150390625,
                    y: 20.7663868125152,
                },
                Coord {
                    x: 5.504150390625,
                    y: 20.46818922264095,
                },
                Coord {
                    x: 5.11962890625,
                    y: 20.46818922264095,
                },
            ]),
            Vec::<LineString<f64>>::new(),
        );

        assert_eq!(poly.tile_cover(8), vec![(131, 112, 8), (131, 113, 8)]);
    }

    #[test]
    fn test_polygon_building() {
        let poly = Polygon::new(
            LineString(vec![
                Coord {
                    x: -77.15269088745116,
                    y: 38.87153962460514,
                },
                Coord {
                    x: -77.1521383523941,
                    y: 38.871322446566325,
                },
                Coord {
                    x: -77.15196132659912,
                    y: 38.87159391901113,
                },
                Coord {
                    x: -77.15202569961546,
                    y: 38.87162315444336,
                },
                Coord {
                    x: -77.1519023180008,
                    y: 38.87179021382536,
                },
                Coord {
                    x: -77.15266406536102,
                    y: 38.8727758561868,
                },
                Coord {
                    x: -77.1527713537216,
                    y: 38.87274662122871,
                },
                Coord {
                    x: -77.15282499790192,
                    y: 38.87282179681094,
                },
                Coord {
                    x: -77.15323269367218,
                    y: 38.87267562199469,
                },
                Coord {
                    x: -77.15313613414764,
                    y: 38.87254197618533,
                },
                Coord {
                    x: -77.15270698070526,
                    y: 38.87236656567917,
                },
                Coord {
                    x: -77.1523904800415,
                    y: 38.87198233162923,
                },
                Coord {
                    x: -77.15269088745116,
                    y: 38.87153962460514,
                },
            ]),
            Vec::<LineString<f64>>::new(),
        );

        assert_eq!(
            poly.tile_cover(18),
            vec![
                (74890, 100305, 18),
                (74891, 100305, 18),
                (74891, 100306, 18)
            ]
        );
    }

    #[test]
    fn test_polygon_donut() {
        let poly = Polygon::new(
            LineString(vec![
                Coord {
                    x: -76.165286,
                    y: 45.479514,
                },
                Coord {
                    x: -76.140095,
                    y: 45.457437,
                },
                Coord {
                    x: -76.162348,
                    y: 45.444872,
                },
                Coord {
                    x: -76.168656,
                    y: 45.441087,
                },
                Coord {
                    x: -76.201963,
                    y: 45.420225,
                },
                Coord {
                    x: -76.213668,
                    y: 45.429276,
                },
                Coord {
                    x: -76.214261,
                    y: 45.429917,
                },
                Coord {
                    x: -76.227477,
                    y: 45.440383,
                },
                Coord {
                    x: -76.263056,
                    y: 45.467983,
                },
                Coord {
                    x: -76.245084,
                    y: 45.468609,
                },
                Coord {
                    x: -76.240206,
                    y: 45.471202,
                },
                Coord {
                    x: -76.238518,
                    y: 45.475254,
                },
                Coord {
                    x: -76.233483,
                    y: 45.507829,
                },
                Coord {
                    x: -76.227816,
                    y: 45.511836,
                },
                Coord {
                    x: -76.212117,
                    y: 45.51623,
                },
                Coord {
                    x: -76.191776,
                    y: 45.50154,
                },
                Coord {
                    x: -76.174016,
                    y: 45.486911,
                },
                Coord {
                    x: -76.165286,
                    y: 45.479514,
                },
            ]),
            vec![LineString(vec![
                Coord {
                    x: -76.227618,
                    y: 45.489247,
                },
                Coord {
                    x: -76.232113,
                    y: 45.486983,
                },
                Coord {
                    x: -76.232151,
                    y: 45.486379,
                },
                Coord {
                    x: -76.231812,
                    y: 45.485106,
                },
                Coord {
                    x: -76.230698,
                    y: 45.483236,
                },
                Coord {
                    x: -76.225664,
                    y: 45.477365,
                },
                Coord {
                    x: -76.223568,
                    y: 45.475174,
                },
                Coord {
                    x: -76.202829,
                    y: 45.458815,
                },
                Coord {
                    x: -76.200229,
                    y: 45.458822,
                },
                Coord {
                    x: -76.199069,
                    y: 45.459164,
                },
                Coord {
                    x: -76.188361,
                    y: 45.465784,
                },
                Coord {
                    x: -76.204505,
                    y: 45.479018,
                },
                Coord {
                    x: -76.215555,
                    y: 45.488534,
                },
                Coord {
                    x: -76.220249,
                    y: 45.492175,
                },
                Coord {
                    x: -76.221154,
                    y: 45.493315,
                },
                Coord {
                    x: -76.22631,
                    y: 45.490189,
                },
                Coord {
                    x: -76.226543,
                    y: 45.489754,
                },
                Coord {
                    x: -76.227618,
                    y: 45.489247,
                },
            ])],
        );

        assert_eq!(
            poly.tile_cover(16),
            vec![
                (18884, 23453, 16),
                (18884, 23454, 16),
                (18885, 23453, 16),
                (18885, 23454, 16),
                (18885, 23455, 16),
                (18886, 23453, 16),
                (18886, 23454, 16),
                (18886, 23455, 16),
                (18886, 23456, 16),
                (18887, 23453, 16),
                (18887, 23454, 16),
                (18887, 23455, 16),
                (18887, 23456, 16),
                (18887, 23457, 16),
                (18888, 23452, 16),
                (18888, 23453, 16),
                (18888, 23454, 16),
                (18888, 23455, 16),
                (18888, 23456, 16),
                (18888, 23457, 16),
                (18888, 23458, 16),
                (18889, 23444, 16),
                (18889, 23445, 16),
                (18889, 23446, 16),
                (18889, 23447, 16),
                (18889, 23448, 16),
                (18889, 23449, 16),
                (18889, 23450, 16),
                (18889, 23451, 16),
                (18889, 23452, 16),
                (18889, 23453, 16),
                (18889, 23454, 16),
                (18889, 23455, 16),
                (18889, 23456, 16),
                (18889, 23457, 16),
                (18889, 23458, 16),
                (18889, 23459, 16),
                (18890, 23442, 16),
                (18890, 23443, 16),
                (18890, 23444, 16),
                (18890, 23445, 16),
                (18890, 23446, 16),
                (18890, 23447, 16),
                (18890, 23448, 16),
                (18890, 23449, 16),
                (18890, 23450, 16),
                (18890, 23451, 16),
                (18890, 23452, 16),
                (18890, 23453, 16),
                (18890, 23454, 16),
                (18890, 23455, 16),
                (18890, 23456, 16),
                (18890, 23457, 16),
                (18890, 23458, 16),
                (18890, 23459, 16),
                (18890, 23460, 16),
                (18891, 23442, 16),
                (18891, 23443, 16),
                (18891, 23444, 16),
                (18891, 23445, 16),
                (18891, 23446, 16),
                (18891, 23447, 16),
                (18891, 23448, 16),
                (18891, 23450, 16),
                (18891, 23451, 16),
                (18891, 23452, 16),
                (18891, 23453, 16),
                (18891, 23454, 16),
                (18891, 23455, 16),
                (18891, 23456, 16),
                (18891, 23457, 16),
                (18891, 23458, 16),
                (18891, 23459, 16),
                (18891, 23460, 16),
                (18891, 23461, 16),
                (18891, 23462, 16),
                (18892, 23441, 16),
                (18892, 23442, 16),
                (18892, 23443, 16),
                (18892, 23444, 16),
                (18892, 23445, 16),
                (18892, 23446, 16),
                (18892, 23447, 16),
                (18892, 23448, 16),
                (18892, 23452, 16),
                (18892, 23453, 16),
                (18892, 23454, 16),
                (18892, 23455, 16),
                (18892, 23456, 16),
                (18892, 23457, 16),
                (18892, 23458, 16),
                (18892, 23459, 16),
                (18892, 23460, 16),
                (18892, 23461, 16),
                (18892, 23462, 16),
                (18892, 23463, 16),
                (18893, 23441, 16),
                (18893, 23442, 16),
                (18893, 23443, 16),
                (18893, 23444, 16),
                (18893, 23445, 16),
                (18893, 23446, 16),
                (18893, 23447, 16),
                (18893, 23448, 16),
                (18893, 23449, 16),
                (18893, 23453, 16),
                (18893, 23454, 16),
                (18893, 23455, 16),
                (18893, 23456, 16),
                (18893, 23457, 16),
                (18893, 23458, 16),
                (18893, 23459, 16),
                (18893, 23460, 16),
                (18893, 23461, 16),
                (18893, 23462, 16),
                (18893, 23463, 16),
                (18893, 23464, 16),
                (18894, 23441, 16),
                (18894, 23442, 16),
                (18894, 23443, 16),
                (18894, 23444, 16),
                (18894, 23445, 16),
                (18894, 23446, 16),
                (18894, 23447, 16),
                (18894, 23448, 16),
                (18894, 23449, 16),
                (18894, 23450, 16),
                (18894, 23454, 16),
                (18894, 23455, 16),
                (18894, 23456, 16),
                (18894, 23457, 16),
                (18894, 23458, 16),
                (18894, 23459, 16),
                (18894, 23460, 16),
                (18894, 23461, 16),
                (18894, 23462, 16),
                (18894, 23463, 16),
                (18894, 23464, 16),
                (18894, 23465, 16),
                (18895, 23442, 16),
                (18895, 23443, 16),
                (18895, 23444, 16),
                (18895, 23445, 16),
                (18895, 23446, 16),
                (18895, 23447, 16),
                (18895, 23448, 16),
                (18895, 23449, 16),
                (18895, 23450, 16),
                (18895, 23451, 16),
                (18895, 23455, 16),
                (18895, 23456, 16),
                (18895, 23457, 16),
                (18895, 23458, 16),
                (18895, 23459, 16),
                (18895, 23460, 16),
                (18895, 23461, 16),
                (18895, 23462, 16),
                (18895, 23463, 16),
                (18895, 23464, 16),
                (18895, 23465, 16),
                (18895, 23466, 16),
                (18896, 23443, 16),
                (18896, 23444, 16),
                (18896, 23445, 16),
                (18896, 23446, 16),
                (18896, 23447, 16),
                (18896, 23448, 16),
                (18896, 23449, 16),
                (18896, 23450, 16),
                (18896, 23451, 16),
                (18896, 23452, 16),
                (18896, 23455, 16),
                (18896, 23456, 16),
                (18896, 23457, 16),
                (18896, 23458, 16),
                (18896, 23459, 16),
                (18896, 23460, 16),
                (18896, 23461, 16),
                (18896, 23462, 16),
                (18896, 23463, 16),
                (18896, 23464, 16),
                (18896, 23465, 16),
                (18896, 23466, 16),
                (18897, 23444, 16),
                (18897, 23445, 16),
                (18897, 23446, 16),
                (18897, 23447, 16),
                (18897, 23448, 16),
                (18897, 23449, 16),
                (18897, 23450, 16),
                (18897, 23451, 16),
                (18897, 23452, 16),
                (18897, 23453, 16),
                (18897, 23454, 16),
                (18897, 23455, 16),
                (18897, 23456, 16),
                (18897, 23457, 16),
                (18897, 23458, 16),
                (18897, 23459, 16),
                (18897, 23460, 16),
                (18897, 23461, 16),
                (18897, 23462, 16),
                (18897, 23463, 16),
                (18897, 23464, 16),
                (18897, 23465, 16),
                (18898, 23445, 16),
                (18898, 23446, 16),
                (18898, 23447, 16),
                (18898, 23448, 16),
                (18898, 23449, 16),
                (18898, 23450, 16),
                (18898, 23451, 16),
                (18898, 23452, 16),
                (18898, 23453, 16),
                (18898, 23454, 16),
                (18898, 23455, 16),
                (18898, 23456, 16),
                (18898, 23457, 16),
                (18898, 23458, 16),
                (18898, 23459, 16),
                (18898, 23460, 16),
                (18898, 23461, 16),
                (18898, 23462, 16),
                (18898, 23463, 16),
                (18898, 23464, 16),
                (18899, 23446, 16),
                (18899, 23447, 16),
                (18899, 23448, 16),
                (18899, 23449, 16),
                (18899, 23450, 16),
                (18899, 23451, 16),
                (18899, 23452, 16),
                (18899, 23453, 16),
                (18899, 23454, 16),
                (18899, 23455, 16),
                (18899, 23456, 16),
                (18899, 23457, 16),
                (18899, 23458, 16),
                (18899, 23459, 16),
                (18899, 23460, 16),
                (18899, 23461, 16),
                (18899, 23462, 16),
                (18899, 23463, 16),
                (18900, 23447, 16),
                (18900, 23448, 16),
                (18900, 23449, 16),
                (18900, 23450, 16),
                (18900, 23451, 16),
                (18900, 23452, 16),
                (18900, 23453, 16),
                (18900, 23454, 16),
                (18900, 23455, 16),
                (18900, 23456, 16),
                (18900, 23457, 16),
                (18900, 23458, 16),
                (18900, 23459, 16),
                (18900, 23460, 16),
                (18900, 23461, 16),
                (18900, 23462, 16),
                (18901, 23449, 16),
                (18901, 23450, 16),
                (18901, 23451, 16),
                (18901, 23452, 16),
                (18901, 23453, 16),
                (18901, 23454, 16),
                (18901, 23455, 16),
                (18901, 23456, 16),
                (18901, 23457, 16),
                (18901, 23458, 16),
                (18901, 23459, 16),
                (18901, 23460, 16),
                (18901, 23461, 16),
                (18902, 23450, 16),
                (18902, 23451, 16),
                (18902, 23452, 16),
                (18902, 23453, 16),
                (18902, 23454, 16),
                (18902, 23455, 16),
                (18902, 23456, 16),
                (18902, 23457, 16),
                (18902, 23458, 16),
                (18902, 23459, 16),
                (18902, 23460, 16),
                (18903, 23451, 16),
                (18903, 23452, 16),
                (18903, 23453, 16),
                (18903, 23454, 16),
                (18903, 23455, 16),
                (18903, 23456, 16),
                (18903, 23457, 16),
                (18903, 23458, 16),
                (18903, 23459, 16),
                (18903, 23460, 16),
                (18904, 23452, 16),
                (18904, 23453, 16),
                (18904, 23454, 16),
                (18904, 23455, 16),
                (18904, 23456, 16),
                (18904, 23457, 16),
                (18904, 23458, 16),
                (18904, 23459, 16),
                (18905, 23454, 16),
                (18905, 23455, 16),
                (18905, 23456, 16),
                (18905, 23457, 16),
                (18905, 23458, 16),
                (18906, 23455, 16),
                (18906, 23456, 16),
                (18906, 23457, 16),
                (18907, 23456, 16)
            ]
        );
    }

    #[test]
    fn test_rect() {
        let rect = Rect::new(coord! { x: -30., y: 57. }, coord! { x: -28., y: 59. });
        let poly: Polygon<_> = rect.into();

        let single_tile_cover = rect.tile_cover(5);
        assert_eq!(single_tile_cover, vec![(13, 9, 5)]);
        assert_eq!(single_tile_cover, poly.tile_cover(5));

        let multi_tile_cover = rect.tile_cover(7);
        assert_eq!(
            multi_tile_cover,
            vec![
                (53, 37, 7),
                (53, 38, 7),
                (53, 39, 7),
                (54, 37, 7),
                (54, 38, 7),
                (54, 39, 7)
            ]
        );
        assert_eq!(multi_tile_cover, poly.tile_cover(7));
    }

    #[test]
    fn test_get_parent() {
        assert_eq!(get_parent((5, 10, 10)), (2, 5, 9))
    }

    #[test]
    fn test_get_siblings() {
        assert_eq!(
            get_siblings((5, 10, 10)),
            vec![(4, 10, 10), (5, 10, 10), (5, 11, 10), (4, 11, 10)]
        )
    }

    #[test]
    fn test_tile_to_bbox() {
        let bbox = tile_to_bbox((5, 10, 10));
        assert_eq!(bbox.min(), coord! { x: -178.2421875, y: 84.7060489350415 });
        assert_eq!(bbox.max(), coord! { x: -177.890625, y: 84.73838712095339 });
    }

    #[test]
    fn test_coord_to_tile_fraction() {
        assert_eq!(
            coord_to_tile_fraction(coord! { x: -95.93965530395508, y: 41.26000108568697 }, 9),
            (119.552490234375, 191.47119140625, 9)
        );
    }

    #[test]
    fn test_coord_to_tile() {
        assert_eq!(coord_to_tile(coord! { x: 0.0, y: 0.0 }, 10), (512, 512, 10));
        assert_eq!(
            coord_to_tile(coord! { x: -77.03239381313323, y: 38.91326516559442 }, 10),
            (292, 391, 10)
        );
    }

    #[test]
    fn test_coord_to_tile_cross_meridian_x() {
        assert_eq!(coord_to_tile(coord! { x: -180.0, y: 0.0 }, 0), (0, 0, 0));
        assert_eq!(coord_to_tile(coord! { x: -180.0, y: 85.0 }, 2), (0, 0, 2));
        assert_eq!(coord_to_tile(coord! { x: 180.0, y: 85.0 }, 2), (0, 0, 2));
        assert_eq!(coord_to_tile(coord! { x: -185.0, y: 85.0 }, 2), (3, 0, 2));
        assert_eq!(coord_to_tile(coord! { x: 185.0, y: 85.0 }, 2), (0, 0, 2));
    }

    #[test]
    fn test_coord_to_tile_cross_meridian_y() {
        assert_eq!(coord_to_tile(coord! { x: -175.0, y: -95.0 }, 2), (0, 3, 2));
        assert_eq!(coord_to_tile(coord! { x: -175.0, y: 95.0 }, 2), (0, 0, 2));
    }
}

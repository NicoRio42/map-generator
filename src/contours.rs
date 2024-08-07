use crate::{
    canvas::Canvas,
    config::Config,
    constants::{
        BROWN, CONTOUR_THICKNESS_MILLIMETTER, FORM_CONTOUR_DASH_INTERVAL_LENGTH,
        FORM_CONTOUR_DASH_LENGTH, FORM_CONTOUR_THICKNESS_MILLIMETTER, INCH,
        MASTER_CONTOUR_THICKNESS_MILLIMETTER,
    },
    tile::{NeighborTiles, Tile},
};
use shapefile::{dbase, Point};
use std::path::PathBuf;

struct Contour {
    elevation: f64,
    polyline: Vec<Point>,
}

pub fn render_contours_to_png(tile: &Tile, image_width: u32, image_height: u32, config: &Config) {
    println!("Rendering contours");

    let scale_factor = config.dpi_resolution / INCH;
    let contours_path = tile.dir_path.join("contours.shp");

    let contours =
        shapefile::read_as::<_, shapefile::Polyline, shapefile::dbase::Record>(contours_path)
            .expect("Could not open contours shapefile");

    let mut contours_layer_img = Canvas::new(image_width as i32, image_height as i32);
    let mut polylines: Vec<Contour> = vec![];

    for (polyline, record) in contours {
        let elevation = match record.get("elev") {
            Some(dbase::FieldValue::Numeric(Some(x))) => x,
            Some(_) => panic!("Expected 'elev' to be a numeric in polygon-dataset"),
            None => panic!("Field 'elev' is not within polygon-dataset"),
        };

        for part in polyline.parts() {
            polylines.push(Contour {
                elevation: *elevation,
                polyline: part.clone(), // TODO: maybe perf cost
            })
        }
    }

    for contour in &polylines {
        let is_normal_contour = contour.elevation as i32 % 5 == 0;
        let is_master_contour = contour.elevation as i32 % 25 == 0;

        if contour.polyline.len() < 2 {
            continue;
        }

        contours_layer_img.set_stroke_cap_round();
        contours_layer_img.set_color(BROWN);

        if is_master_contour {
            contours_layer_img.set_line_width(
                MASTER_CONTOUR_THICKNESS_MILLIMETTER * config.dpi_resolution * 10.0 / INCH,
            );

            draw_bezier_curve_from_polyline_on_image(
                &contour.polyline,
                &mut contours_layer_img,
                scale_factor,
                tile.min_x as u64,
                tile.min_y as u64,
                image_height,
            );
        } else if is_normal_contour {
            contours_layer_img.set_line_width(
                CONTOUR_THICKNESS_MILLIMETTER * config.dpi_resolution * 10.0 / INCH,
            );

            draw_bezier_curve_from_polyline_on_image(
                &contour.polyline,
                &mut contours_layer_img,
                scale_factor,
                tile.min_x as u64,
                tile.min_y as u64,
                image_height,
            );
        } else {
            let releveant_form_contours = keep_relevant_form_contours(contour, &polylines, config);

            contours_layer_img.set_line_width(
                FORM_CONTOUR_THICKNESS_MILLIMETTER * config.dpi_resolution * 10.0 / INCH,
            );

            contours_layer_img.set_dash(
                FORM_CONTOUR_DASH_LENGTH * config.dpi_resolution * 10.0 / INCH,
                FORM_CONTOUR_DASH_INTERVAL_LENGTH * config.dpi_resolution * 10.0 / INCH,
            );

            for relevant_form_contour in releveant_form_contours {
                draw_bezier_curve_from_polyline_on_image(
                    &relevant_form_contour,
                    &mut contours_layer_img,
                    scale_factor,
                    tile.min_x as u64,
                    tile.min_y as u64,
                    image_height,
                );
            }

            contours_layer_img.unset_dash();
        }
    }

    let contours_output_path = tile.dir_path.join("contours.png");
    let contours_output_path_str = contours_output_path.to_str().unwrap();

    contours_layer_img.save_as(&contours_output_path_str);
}

fn draw_bezier_curve_from_polyline_on_image(
    polyline: &Vec<Point>,
    image: &mut Canvas,
    scale_factor: f32,
    min_x: u64,
    min_y: u64,
    image_height: u32,
) {
    let mut points: Vec<(f32, f32)> = vec![];

    for point in polyline {
        points.push((
            (point.x as i64 - min_x as i64) as f32 * scale_factor,
            (image_height as f32 - ((point.y as i64 - min_y as i64) as f32 * scale_factor)),
        ))
    }

    image.draw_bezier_curve_from_polyline(&points);
}

struct TaggedPoint<'a> {
    point: &'a Point,
    should_be_kept: bool,
}

fn keep_relevant_form_contours(
    form_contour: &Contour,
    contours: &Vec<Contour>,
    config: &Config,
) -> Vec<Vec<Point>> {
    let mut relevant_form_lines: Vec<Vec<Point>> = vec![];
    // if form_contour.elevation != 2307.5 {
    //     return relevant_form_lines;
    // }
    let mut above_adjacent_contours: Vec<&Vec<Point>> = vec![];
    let mut below_adjacent_contours: Vec<&Vec<Point>> = vec![];
    let mut tagged_form_line_polyline: Vec<TaggedPoint> = vec![];

    for contour in contours {
        if contour.elevation == form_contour.elevation + 2.5 {
            above_adjacent_contours.push(&contour.polyline)
        }
        if contour.elevation == form_contour.elevation - 2.5 {
            below_adjacent_contours.push(&contour.polyline)
        }
    }

    if above_adjacent_contours.len() == 0 || below_adjacent_contours.len() == 0 {
        relevant_form_lines.push(form_contour.polyline.clone());
        return relevant_form_lines;
    }

    // Choosing witch points we keep
    for point_index in 0..form_contour.polyline.len() {
        let point = &form_contour.polyline[point_index];

        let mut distance_to_above_adjacent_contours =
            distance_point_to_polyline(point, above_adjacent_contours[0]);

        if above_adjacent_contours.len() > 1 {
            for index in 1..above_adjacent_contours.len() {
                let new_distance_to_above_adjacent_contour =
                    distance_point_to_polyline(point, above_adjacent_contours[index]);

                if new_distance_to_above_adjacent_contour < distance_to_above_adjacent_contours {
                    distance_to_above_adjacent_contours = new_distance_to_above_adjacent_contour;
                }
            }
        }

        let mut distance_to_below_adjacent_contours =
            distance_point_to_polyline(point, below_adjacent_contours[0]);

        if below_adjacent_contours.len() > 1 {
            for index in 1..below_adjacent_contours.len() {
                let new_distance_to_below_adjacent_contour =
                    distance_point_to_polyline(point, below_adjacent_contours[index]);

                if new_distance_to_below_adjacent_contour < distance_to_below_adjacent_contours {
                    distance_to_below_adjacent_contours = new_distance_to_below_adjacent_contour;
                }
            }
        }

        let normalized_distance_difference = ((distance_to_above_adjacent_contours
            - distance_to_below_adjacent_contours)
            / distance_to_above_adjacent_contours.min(distance_to_below_adjacent_contours))
        .abs();

        let should_be_kept = (distance_to_above_adjacent_contours
            > config.form_lines.min_distance_to_contour
            && distance_to_below_adjacent_contours > config.form_lines.min_distance_to_contour
            && normalized_distance_difference > config.form_lines.threshold)
            || distance_to_above_adjacent_contours > config.form_lines.max_distance_to_contour
            || distance_to_below_adjacent_contours > config.form_lines.max_distance_to_contour;

        tagged_form_line_polyline.push(TaggedPoint {
            point,
            should_be_kept,
        });
    }

    tagged_form_line_polyline = remove_gaps_from_tagged_form_line_polyline(
        tagged_form_line_polyline,
        config.form_lines.min_gap_length,
    );

    tagged_form_line_polyline = add_tails_to_tagged_form_line_polyline(
        tagged_form_line_polyline,
        config.form_lines.additional_tail_length,
    );

    let mut should_start_new_polyline = true;

    for tagged_point in tagged_form_line_polyline {
        if !tagged_point.should_be_kept {
            should_start_new_polyline = true;
            continue;
        }

        if !should_start_new_polyline {
            let len = relevant_form_lines.len();
            relevant_form_lines[len - 1].push(*tagged_point.point);
            continue;
        }

        relevant_form_lines.push(vec![*tagged_point.point]);
        should_start_new_polyline = false;
    }

    return relevant_form_lines;
}

fn remove_gaps_from_tagged_form_line_polyline(
    mut tagged_form_line_polyline: Vec<TaggedPoint>,
    min_gap_length: f64,
) -> Vec<TaggedPoint> {
    let mut current_gap_points_indexes: Vec<usize> = vec![];

    for index in 0..tagged_form_line_polyline.len() {
        let tagged_point = &tagged_form_line_polyline[index];

        let is_lonely_point_that_should_be_kept = tagged_point.should_be_kept
            && index != 0
            && index != tagged_form_line_polyline.len() - 1
            && !tagged_form_line_polyline[index - 1].should_be_kept
            && !tagged_form_line_polyline[index + 1].should_be_kept;

        if !tagged_point.should_be_kept || is_lonely_point_that_should_be_kept {
            if index != 0 && current_gap_points_indexes.len() == 0 {
                current_gap_points_indexes.push(index - 1);
            }

            current_gap_points_indexes.push(index);
            continue;
        }

        current_gap_points_indexes.push(index);
        let mut current_gap: Vec<Point> = vec![];

        for current_gap_point_index in &current_gap_points_indexes {
            current_gap.push(*tagged_form_line_polyline[*current_gap_point_index].point)
        }

        let gap_length = polyline_length(&current_gap);

        if gap_length < min_gap_length {
            for current_gap_point_index in current_gap_points_indexes {
                tagged_form_line_polyline[current_gap_point_index].should_be_kept = true;
            }
        }

        current_gap_points_indexes = vec![];
    }

    return tagged_form_line_polyline;
}

fn add_tails_to_tagged_form_line_polyline(
    mut tagged_form_line_polyline: Vec<TaggedPoint>,
    additional_tail_length: f64,
) -> Vec<TaggedPoint> {
    let mut start_edges_indexes: Vec<usize> = vec![];
    let mut end_edges_indexes: Vec<usize> = vec![];

    for index in 0..tagged_form_line_polyline.len() {
        if !tagged_form_line_polyline[index].should_be_kept
            || index == 0
            || index == tagged_form_line_polyline.len() - 1
        {
            continue;
        }

        let previous_tagged_point = &tagged_form_line_polyline[index - 1];
        let next_tagged_point = &tagged_form_line_polyline[index + 1];

        if !previous_tagged_point.should_be_kept && next_tagged_point.should_be_kept {
            start_edges_indexes.push(index);
        }

        if previous_tagged_point.should_be_kept && !next_tagged_point.should_be_kept {
            end_edges_indexes.push(index);
        }
    }

    for start_index in start_edges_indexes {
        let mut index: usize = start_index - 1;
        let mut tail: Vec<Point> = vec![*tagged_form_line_polyline[start_index].point];
        let mut tail_indexes: Vec<usize> = vec![];

        loop {
            let tail_length = polyline_length(&tail);
            if index == 0 || tail_length > additional_tail_length {
                break;
            }
            tail.push(*tagged_form_line_polyline[index].point);
            tail_indexes.push(index);
            index -= 1;
        }

        for tail_index in tail_indexes {
            tagged_form_line_polyline[tail_index].should_be_kept = true;
        }
    }

    for end_index in end_edges_indexes {
        let mut index = end_index + 1;
        let mut tail: Vec<Point> = vec![*tagged_form_line_polyline[end_index].point];
        let mut tail_indexes: Vec<usize> = vec![];

        loop {
            let tail_length = polyline_length(&tail);
            if index == tagged_form_line_polyline.len() - 1 || tail_length > additional_tail_length
            {
                break;
            }
            tail.push(*tagged_form_line_polyline[index].point);
            tail_indexes.push(index);
            index += 1;
        }

        for tail_index in tail_indexes {
            tagged_form_line_polyline[tail_index].should_be_kept = true;
        }
    }

    return tagged_form_line_polyline;
}

fn distance_point_to_polyline(point: &Point, polyline: &Vec<Point>) -> f64 {
    if polyline.len() == 0 {
        panic!("Trying to compute distance to an empty polyline.")
    };

    let mut distance = magnitude(point.x - polyline[0].x, point.y - polyline[0].y);

    if polyline.len() == 1 {
        return distance;
    }

    for i in 1..polyline.len() {
        let d = distance_point_to_segment(point, polyline[i - 1], polyline[i]);

        if d < distance {
            distance = d;
        }
    }

    return distance;
}

fn distance_point_to_segment(point: &Point, extremity1: Point, extremity2: Point) -> f64 {
    let r = dot_product(
        extremity2.x - extremity1.x,
        extremity2.y - extremity1.y,
        point.x - extremity1.x,
        point.y - extremity1.y,
    ) / magnitude(extremity2.x - extremity1.x, extremity2.y - extremity1.y).powi(2);

    if r < 0.0 {
        return magnitude(point.x - extremity1.x, point.y - extremity1.y);
    } else if r > 1.0 {
        return magnitude(extremity2.x - point.x, extremity2.y - point.y);
    }

    return (magnitude(point.x - extremity1.x, point.y - extremity1.y).powi(2)
        - (r * magnitude(extremity2.x - extremity1.x, extremity2.y - extremity1.y)).powi(2))
    .sqrt();
}

fn polyline_length(polyline: &Vec<Point>) -> f64 {
    if polyline.len() < 2 {
        return 0.0;
    }

    let mut length = 0.0;

    for index in 0..(polyline.len() - 1) {
        length += magnitude(
            polyline[index + 1].x - polyline[index].x,
            polyline[index + 1].y - polyline[index].y,
        );
    }

    return length;
}

fn magnitude(x: f64, y: f64) -> f64 {
    return (x.powi(2) + y.powi(2)).sqrt();
}

fn dot_product(x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    return x1 * x2 + y1 * y2;
}

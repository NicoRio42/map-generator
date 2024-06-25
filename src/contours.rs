use std::path::PathBuf;

use crate::{
    canvas::Canvas,
    config::Config,
    constants::{
        BROWN, CONTOUR_THICKNESS_MILLIMETTER, FORM_CONTOUR_DASH_INTERVAL_LENGTH,
        FORM_CONTOUR_DASH_LENGTH, FORM_CONTOUR_THICKNESS_MILLIMETTER, INCH,
        MASTER_CONTOUR_THICKNESS_MILLIMETTER,
    },
};
use shapefile::dbase;

pub fn render_contours_to_png(
    image_width: u32,
    image_height: u32,
    config: &Config,
    min_x: u64,
    min_y: u64,
    out_dir: &PathBuf,
) {
    println!("Rendering contours");

    let scale_factor = config.dpi_resolution / INCH;
    let contours_path = out_dir.join("contours.shp");

    let contours =
        shapefile::read_as::<_, shapefile::Polyline, shapefile::dbase::Record>(contours_path)
            .expect("Could not open contours shapefile");

    let mut contours_layer_img = Canvas::new(image_width as i32, image_height as i32);

    for (polyline, record) in contours {
        let elevation = match record.get("elev") {
            Some(dbase::FieldValue::Numeric(Some(x))) => x,
            Some(_) => panic!("Expected 'elev' to be a numeric in polygon-dataset"),
            None => panic!("Field 'elev' is not within polygon-dataset"),
        };

        let is_normal_contour = *elevation as i32 % 5 == 0;
        let is_master_contour = *elevation as i32 % 25 == 0;

        for part in polyline.parts() {
            if part.len() < 2 {
                continue;
            }

            let mut points: Vec<(f32, f32)> = vec![];

            for point in part {
                points.push((
                    (point.x as i64 - min_x as i64) as f32 * scale_factor,
                    (image_height as f32 - ((point.y as i64 - min_y as i64) as f32 * scale_factor)),
                ))
            }

            contours_layer_img.set_stroke_cap_round();
            contours_layer_img.set_color(BROWN);

            if is_master_contour {
                contours_layer_img.set_line_width(
                    MASTER_CONTOUR_THICKNESS_MILLIMETTER * config.dpi_resolution * 10.0 / INCH,
                );
            } else if is_normal_contour {
                contours_layer_img.set_line_width(
                    CONTOUR_THICKNESS_MILLIMETTER * config.dpi_resolution * 10.0 / INCH,
                );
            } else {
                contours_layer_img.set_line_width(
                    FORM_CONTOUR_THICKNESS_MILLIMETTER * config.dpi_resolution * 10.0 / INCH,
                );

                contours_layer_img.set_dash(
                    FORM_CONTOUR_DASH_LENGTH * config.dpi_resolution * 10.0 / INCH,
                    FORM_CONTOUR_DASH_INTERVAL_LENGTH * config.dpi_resolution * 10.0 / INCH,
                )
            }

            contours_layer_img.draw_polyline(&points);
            contours_layer_img.unset_dash();
        }
    }

    let contours_output_path = out_dir.join("contours.png");
    let contours_output_path_str = contours_output_path.to_str().unwrap();

    contours_layer_img.save_as(&contours_output_path_str);
}

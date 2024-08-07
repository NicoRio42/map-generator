use crate::{
    canvas::Canvas,
    config::Config,
    constants::{
        BLUE, FOOTPATH_DASH_INTERVAL_LENGTH, FOOTPATH_DASH_LENGTH, FOOTPATH_WIDTH, INCH,
        INCROSSABLE_BODY_OF_WATER_OUTLINE_WIDTH,
    },
    tile::Tile,
};
use shapefile::{
    dbase::{FieldValue, Record},
    read_as, Polygon, Polyline,
};
use std::{
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
};

pub fn render_vector_shapes(tile: &Tile, image_width: u32, image_height: u32, config: &Config) {
    println!("Transforming osm file to shapefiles");

    let scale_factor = config.dpi_resolution / INCH;
    let shapes_outlput_path = tile.dir_path.join("shapes");
    let osm_path = Path::new("in").join(format!("{:0>7}_{:0>7}.osm", tile.min_x, tile.max_y));

    let ogr2ogr_output = Command::new("ogr2ogr")
        .args([
            "--config",
            "OSM_USE_CUSTOM_INDEXING",
            "NO",
            "-skipfailures",
            "-f",
            "ESRI Shapefile",
            &shapes_outlput_path.to_str().unwrap(),
            &osm_path.to_str().unwrap(),
            "-t_srs",
            "EPSG:2154",
        ])
        .arg("--quiet")
        .output()
        .expect("failed to execute ogr2ogr command");

    if ExitStatus::success(&ogr2ogr_output.status) {
        println!("{}", String::from_utf8(ogr2ogr_output.stdout).unwrap());
    } else {
        println!("{}", String::from_utf8(ogr2ogr_output.stderr).unwrap());
    }

    println!("Rendering vectors");

    let mut vectors_layer_img = Canvas::new(image_width as i32, image_height as i32);

    let multipolygons_path = shapes_outlput_path.join("multipolygons.shp");
    let multipolygons = read_as::<_, Polygon, Record>(multipolygons_path)
        .expect("Could not open multipolygons shapefile");

    for (polygon, record) in multipolygons {
        let natural = match record.get("natural") {
            Some(FieldValue::Character(Some(x))) => x,
            Some(_) => "",
            None => panic!("Field 'natural' is not within polygon-dataset"),
        };

        if natural != "water" {
            continue;
        }

        for ring in polygon.rings().iter() {
            let mut points: Vec<(f32, f32)> = vec![];

            for point in ring.points().iter() {
                points.push((
                    (point.x as i64 - tile.min_x) as f32 * scale_factor,
                    (image_height as f32 - ((point.y as i64 - tile.min_y) as f32 * scale_factor)),
                ))
            }

            vectors_layer_img.set_color(BLUE);
            vectors_layer_img.draw_filled_polygon(&points);

            vectors_layer_img.set_line_width(
                INCROSSABLE_BODY_OF_WATER_OUTLINE_WIDTH * config.dpi_resolution * 10.0 / INCH,
            );

            vectors_layer_img.set_color((0, 0, 0));
            vectors_layer_img.draw_polyline(&points);
        }
    }

    let lines_path = shapes_outlput_path.join("lines.shp");
    let lines = read_as::<_, Polyline, Record>(lines_path).expect("Could not open lines shapefile");

    for (line, record) in lines {
        let highway = match record.get("highway") {
            Some(FieldValue::Character(Some(x))) => x,
            Some(_) => "",
            None => panic!("Field 'highway' is not within polygon-dataset"),
        };

        if highway != "path" {
            continue;
        }

        for part in line.parts() {
            let mut points: Vec<(f32, f32)> = vec![];

            for point in part {
                points.push((
                    (point.x as i64 - tile.min_x) as f32 * scale_factor,
                    (image_height as f32 - ((point.y as i64 - tile.min_y) as f32 * scale_factor)),
                ))
            }

            vectors_layer_img.set_color((0, 0, 0));

            vectors_layer_img.set_line_width(FOOTPATH_WIDTH * config.dpi_resolution * 10.0 / INCH);

            vectors_layer_img.set_dash(
                FOOTPATH_DASH_LENGTH * config.dpi_resolution * 10.0 / INCH,
                FOOTPATH_DASH_INTERVAL_LENGTH * config.dpi_resolution * 10.0 / INCH,
            );

            vectors_layer_img.draw_polyline(&points);
        }
    }

    let vectors_output_path = tile.dir_path.join("vectors.png");
    vectors_layer_img.save_as(&vectors_output_path.to_str().unwrap());
}

#![no_main]
#![no_std]

use cortex_m_rt::entry;
use embedded_graphics::{
    Drawable,
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{Line, PrimitiveStyle},
};
use embedded_hal_bus::spi::ExclusiveDevice;
use microbit::hal::{
    Spim,
    gpio::Level,
    saadc,
    spim::{self, Frequency},
    timer::Timer,
};
use mipidsi::{
    Builder,
    models::GC9A01,
    options::{ColorInversion, Orientation, Rotation},
};
use nalgebra::{Rotation3, Vector3};
use panic_rtt_target as _;
use rtt_target::rtt_init_print;

const TAU: f32 = core::f32::consts::TAU;
const POT_PIN_MAX_READ: i16 = 16_000;
const OBJ_EDGE_COUNT: usize = 8;
const OBJ_VERT_COUNT: usize = 5;
const STROKE_WIDTH: u32 = 3;
const RADIANS_TO_ROTATE_PER_FRAME: f32 = 0.3;

#[entry]
fn main() -> ! {
    rtt_init_print!();

    let board = microbit::Board::take().unwrap();
    let mut timer0 = Timer::new(board.TIMER0);

    // Setup SPI
    let sck = board.pins.p0_17.into_push_pull_output(Level::Low).degrade();
    let coti = board.pins.p0_13.into_push_pull_output(Level::Low).degrade();

    let dc = board.edge.e08.into_push_pull_output(Level::Low);
    let cs = board.edge.e01.into_push_pull_output(Level::Low);
    let rst = board.edge.e09.into_push_pull_output(Level::High);

    let spi_bus = Spim::new(
        board.SPIM3,
        microbit::hal::spim::Pins {
            sck: Some(sck),
            mosi: Some(coti),
            miso: None,
        },
        Frequency::M32,
        spim::MODE_0,
        0xFF, // ORC overflow character
    );
    let spi = display_interface_spi::SPIInterface::new(
        ExclusiveDevice::new_no_delay(spi_bus, cs).unwrap(),
        dc,
    );

    // Setup GC9A01 display using mipidsi
    let mut display = Builder::new(GC9A01, spi)
        .orientation(Orientation::new().rotate(Rotation::Deg0))
        .invert_colors(ColorInversion::Inverted)
        .reset_pin(rst)
        .init(&mut timer0)
        .unwrap();

    // Call `embedded_graphics` `clear()` trait method
    <_ as embedded_graphics::draw_target::DrawTarget>::clear(
        &mut display,
        Rgb565::WHITE,
    )
    .unwrap();

    // Set up potentiometer.
    let mut pot_pin = board.edge.e02.into_floating_input();
    let saadc_config = saadc::SaadcConfig::default();
    let mut saadc = saadc::Saadc::new(board.ADC, saadc_config);

    // A set of colors that will be used for drawing the object.
    let edge_colors: [Rgb565; 8] = [
        Rgb565::RED,
        Rgb565::GREEN,
        Rgb565::BLUE,
        Rgb565::YELLOW,
        Rgb565::CSS_VIOLET,
        Rgb565::CSS_PINK,
        Rgb565::CSS_BROWN,
        Rgb565::CSS_DARK_GRAY,
    ];

    let object = Object3D::new(
        [
            Vector3::new(0.0f32, 10.0f32, 0.0f32),
            Vector3::new(10.0f32, -10.0f32, 10.0f32),
            Vector3::new(-10.0f32, -10.0f32, 10.0f32),
            Vector3::new(-10.0f32, -10.0f32, -10.0f32),
            Vector3::new(10.0f32, -10.0f32, -10.0f32),
        ],
        [
            (0, 1),
            (0, 2),
            (0, 3),
            (0, 4),
            (1, 2),
            (2, 3),
            (3, 4),
            (4, 1),
        ],
    );

    let camera_pos = Vector3::<f32>::new(0.0, 0.0, 40.0);
    let display_surface = Vector3::<f32>::new(0.0, 0.0, 100.0);
    let mut object_rotation: f32 = 0.0;

    loop {
        let object_rotated = rotate_object(&object, object_rotation);
        object_rotation =
            (object_rotation + RADIANS_TO_ROTATE_PER_FRAME) % TAU;

        let saadc_result = saadc.read_channel(&mut pot_pin).unwrap();
        let new_angle = scale_saadc_result(saadc_result);
        let camera_rotation = Vector3::<f32>::new(0.0, new_angle, 0.0);
        let mut points: [Point; OBJ_VERT_COUNT] =
            object_rotated.vertices.map(|v| {
                convert_vertex_to_2d_point(
                    &v,
                    &camera_rotation,
                    &camera_pos,
                    &display_surface,
                )
            });

        convert_points_to_display_coords(&mut points);

        display.clear(Rgb565::BLACK).unwrap();

        for (i, edge) in object.edges.iter().enumerate() {
            Line::new(
                Point::new(points[edge.0].x, points[edge.0].y),
                Point::new(points[edge.1].x, points[edge.1].y),
            )
            .into_styled(PrimitiveStyle::with_stroke(
                edge_colors[i % edge_colors.len()],
                STROKE_WIDTH,
            ))
            .draw(&mut display)
            .unwrap();
        }
    }
}

/// Converts to display coords which range from 0 to 240 on each axis.
fn convert_points_to_display_coords(points: &mut [Point]) {
    for p in points {
        *p = Point::new(p.x + 120, p.y + 120);
    }
}

/// Projects a 3D vertex to a 2D point.
/// https://en.wikipedia.org/wiki/3D_projection#Mathematical_formula
fn convert_vertex_to_2d_point(
    vec: &Vector3<f32>,
    cam_rot: &Vector3<f32>,
    cam_pos: &Vector3<f32>,
    surf: &Vector3<f32>,
) -> Point {
    let theta_x = cam_rot.x.clamp(0.0, TAU);
    let theta_y = cam_rot.y.clamp(0.0, TAU);
    let theta_z = cam_rot.z.clamp(0.0, TAU);

    let rot_x = Rotation3::<f32>::from_euler_angles(theta_x, 0.0, 0.0);
    let rot_y = Rotation3::<f32>::from_euler_angles(0.0, theta_y, 0.0);
    let rot_z = Rotation3::<f32>::from_euler_angles(0.0, 0.0, theta_z);
    let diff = Vector3::<f32>::new(
        vec.x - cam_pos.x,
        vec.y - cam_pos.y,
        vec.z - cam_pos.z,
    );
    let v = rot_x * (rot_y * (rot_z * diff));

    let x = (((surf.z / v.z) * v.x) + surf.x).clamp(-120f32, 120f32);
    let y = (((surf.z / v.z) * v.y) + surf.x).clamp(-120f32, 120f32);

    Point {
        x: x as i32,
        y: y as i32,
    }
}

/// Rotates n radians counterclockwise on the y-axis.
/// https://en.wikipedia.org/wiki/Rotation_matrix#Basic_3D_rotations
fn rotate_object(obj: &Object3D, n: f32) -> Object3D {
    let rot_mat = Rotation3::<f32>::from_euler_angles(0.0, n, 0.0);
    let mut rotated_obj = obj.clone();
    for v in rotated_obj.vertices.iter_mut() {
        *v = rot_mat * *v;
    }
    rotated_obj
}

/// Takes an i16 number expected to be between 0 and 16384 (2^14) and scales
/// it to a radians value between 0.0 and TAU.
fn scale_saadc_result(n: i16) -> f32 {
    let n = n.clamp(0, POT_PIN_MAX_READ);
    ((n as f32 / POT_PIN_MAX_READ as f32) * TAU).clamp(0.0, TAU)
}

#[derive(Clone)]
struct Object3D {
    vertices: [Vector3<f32>; OBJ_VERT_COUNT],
    edges: [(usize, usize); OBJ_EDGE_COUNT],
}

impl Object3D {
    fn new(
        vertices: [Vector3<f32>; OBJ_VERT_COUNT],
        edges: [(usize, usize); OBJ_EDGE_COUNT],
    ) -> Self {
        Self { vertices, edges }
    }
}
